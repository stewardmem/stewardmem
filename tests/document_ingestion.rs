// IMPROVE04 S2 — the document-ingestion route, driven end to end.
//
// ## Why this file exists
//
// `POST /ingest/document` is a NEW ingest surface, and an ingest surface is
// exactly the kind of seam whose defects are invisible to unit tests of the
// extractor itself: they only exist where the doors compose. These tests
// drive the REAL route over the REAL migrated schema with the REAL doors
// (AuthZ, capacity, strip, screen, dedupe, vec0) and pin the S2 acceptance:
// the annotated markdown contract round-trips, the document row lands with
// its provenance fields, the scanned fixture refuses NAMED, the over-cap
// request refuses NAMED, and the same bytes re-serve byte-identically.
// The DI pins live in `tests/document_ingestion_pins.rs` (S3).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/documents");

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture in the corpus")
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A minimal one-page text-layer PDF (the render_pdf shape: hand-written
/// %PDF-1.4, computed xref, Helvetica base-14). The vehicle for payloads the
/// fixture corpus deliberately does not carry (injection strings, think
/// traces) — real bytes on the wire, not mocks.
fn tiny_pdf(lines: &[&str]) -> Vec<u8> {
    let mut stream = String::from("BT /F1 10 Tf 12 TL 54 720 Td\n");
    for line in lines {
        let escaped = line
            .replace('\\', r"\\")
            .replace('(', r"\(")
            .replace(')', r"\)");
        stream.push_str(&format!("({escaped}) Tj T*\n"));
    }
    stream.push_str("ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

const OP_TOKEN: &str = "improve04-document-op-token";

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

/// An opaque-mode server on a real file-backed, real migrated database —
/// the same harness shape as `tests/ingest_title_pins.rs`.
fn server() -> Server {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    let mgr = SqliteConnectionManager::file(&db_path);
    let pool: brain_server::Pool = r2d2::Pool::builder().max_size(4).build(mgr).expect("pool");
    brain_server::migration::run_migration(
        &mut pool.get().expect("conn"),
        brain_server::config::DB_MMAP_SIZE_MIB,
    )
    .expect("migration");
    let model: Arc<dyn brain_server::embed::Embedder> = Arc::new(
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model"),
    );
    let tok_file = dir.path().join("tokens");
    std::fs::write(&tok_file, format!("{OP_TOKEN}\n")).expect("token file");
    let token_store = brain_server::auth::TokenStore::from_file(Some(tok_file));
    token_store.reload_parts_from(vec![OP_TOKEN.to_string()], None);
    let state = Arc::new(brain_server::AppState {
        token_store,
        jwt_middleware_state: Arc::new(JwtMiddlewareState::opaque_for_tests(
            pool.clone(),
            db_path.clone(),
        )),
        cors: brain_server::server::bootstrap::cors_layer_from_config(),
        chat_answer_cache: std::sync::Arc::new(
            brain_server::service::chat_answer_cache::ChatAnswerCache::new(),
        ),
        durability: Default::default(),
        loom: Default::default(),
        model,
        registry: brain_server::domain_registry::DomainRegistry::new(pool.clone(), &db_path, false),
        pool,
        db_path,
        connection_tracker: Arc::new(brain_server::http_limit::ConnectionTracker::new()),
        rate_limiter: Arc::new(brain_server::http_limit::RateLimiter::new()),
        snapshot: brain_server::integrity::SnapshotState::default(),
        audit_chain_cache: Arc::new(std::sync::Mutex::new(None)),
        auth_mode: brain_server::auth::AuthMode::Opaque,
        key_store: brain_server::auth::jwks::KeyStore::default(),
        revocation_cache: Arc::new(brain_server::auth::revocation::RevocationCache::new()),
        jwt_issuer: String::new(),
        jwt_audience: String::new(),
        oidc_config: brain_server::handlers::well_known::OidcConfig::unconfigured(),
        ump_events: tokio::sync::broadcast::channel(brain_server::config::UMP_EVENT_BUFFER).0,
        alert_events: tokio::sync::broadcast::channel(brain_server::config::ALERT_EVENT_BUFFER).0,
        alert_seq: std::sync::atomic::AtomicU64::new(0),
        chain_watch: brain_server::alert::ChainWatchState::default(),
        concurrency: &brain_server::concurrency::CONCURRENCY,
    });
    Server { _dir: dir, state }
}

async fn post_document(
    state: &Arc<brain_server::AppState>,
    body: String,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/ingest/document")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("request");
    let res = app(state.clone()).oneshot(req).await.expect("infallible");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

fn document_body(name: &str, title: &str) -> String {
    serde_json::json!({ "data": b64(&fixture(name)), "title": title }).to_string()
}

fn code_of(resp: &serde_json::Value) -> String {
    resp["error"]["code"]
        .as_str()
        .or_else(|| resp["code"].as_str())
        .unwrap_or("<no code>")
        .to_string()
}

// ── the contract round-trip ──────────────────────────────────────────────────

#[tokio::test]
async fn the_route_returns_the_annotated_markdown() {
    let s = server();
    let (status, resp) = post_document(
        &s.state,
        document_body("single-column.pdf", "Single Column Fixture"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {resp}");
    assert_eq!(resp["status"], "created", "body: {resp}");
    let md = resp["markdown"].as_str().expect("markdown in the response");

    // the machine-checkable header
    assert!(
        md.starts_with("<!-- doc: "),
        "header first, got: {}",
        &md[..md.len().min(80)]
    );
    assert!(md.contains("| tier: text-layer\n"), "tier in header");
    assert!(
        md.contains("| pages: 3 |"),
        "pages: 3 for the 3-page fixture"
    );
    assert!(md.contains("| layout: unverified"), "Tier 0 layout posture");
    assert!(
        md.contains("| extractor: pdf-extract/0.12.1 |"),
        "extractor name+version"
    );
    // every page block carries its anchor
    for n in 1..=3 {
        assert!(md.contains(&format!("## Page {n}")), "page heading {n}");
        assert!(md.contains(&format!("<!-- anchor: p{n} -->")), "anchor {n}");
    }
    // the page text survived
    assert!(md.contains("Local-First Memory for Agents"), "page 1 text");
    assert!(md.contains("Determinism as a Product Law"), "page 3 text");

    // the structural self-check ran (the route never ships a broken contract)
    brain_server::ingest_document::verify_anchor_contract(md, 3)
        .expect("the route's own output satisfies the anchor contract");
}

#[tokio::test]
async fn the_document_row_lands_with_the_contract_fields() {
    let s = server();
    let (status, resp) =
        post_document(&s.state, document_body("single-column.pdf", "Row Fixture")).await;
    assert_eq!(status, StatusCode::OK, "body: {resp}");
    let md = resp["markdown"].as_str().unwrap();
    let sha = resp["sha256"].as_str().unwrap();

    let conn = s.state.pool.get().unwrap();
    let row: (String, String, String, String, i64, i64, String) = conn
        .query_row(
            "SELECT sha256, tier, extractor, extractor_version, pages, layout_unverified, source
             FROM documents WHERE sha256 = ?1",
            [sha],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .expect("the document row exists");
    assert_eq!(row.0, sha, "row keyed by the source-bytes sha");
    assert_eq!(row.1, "text-layer", "tier");
    assert_eq!(row.2, "pdf-extract", "extractor name");
    assert_eq!(row.3, "0.12.1", "extractor version");
    assert_eq!(row.4, 3, "pages");
    assert_eq!(row.5, 1, "Tier 0 NEVER claims layout verified");
    assert_eq!(row.6, "document", "the default trust label");

    // the chunks carry the new ingest kind + the document grouping key
    let chunk_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM knowledge WHERE source = 'document' AND document_id = ?1",
            [sha],
            |r| r.get(0),
        )
        .unwrap();
    assert!(chunk_count > 0, "chunks stored under source=document");

    // the header agrees with the row (DI-3's honest path)
    let hdr = brain_server::ingest_document::parse_document_header(md).expect("header parses");
    assert_eq!(hdr.doc, sha);
    assert_eq!(hdr.pages, 3);
    assert_eq!(hdr.layout, "unverified");
}

#[tokio::test]
async fn same_pdf_bytes_rerun_is_byte_identical_and_duplicate() {
    let s = server();
    let body = document_body("two-column.pdf", "Two Column Fixture");
    let (s1, r1) = post_document(&s.state, body.clone()).await;
    assert_eq!(s1, StatusCode::OK, "first ingest: {r1}");
    let (s2, r2) = post_document(&s.state, body).await;
    assert_eq!(s2, StatusCode::OK, "second ingest: {r2}");
    assert_eq!(
        r2["status"], "duplicate",
        "the sha-keyed row short-circuits: {r2}"
    );

    // THE BYTE-DETERMINISM ACCEPTANCE: same PDF bytes → byte-identical
    // markdown (the duplicate path re-serves under the ROW's extracted
    // stamp — a re-serve is not a re-extraction).
    assert_eq!(
        r1["markdown"].as_str().unwrap(),
        r2["markdown"].as_str().unwrap(),
        "run twice, diff empty"
    );

    // no new chunks: the stored rows already hold this document
    let conn = s.state.pool.get().unwrap();
    let sha = r1["sha256"].as_str().unwrap();
    let count = |conn: &rusqlite::Connection| -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM knowledge WHERE document_id = ?1",
            [sha],
            |r| r.get(0),
        )
        .unwrap()
    };
    let after_first = count(&conn);
    assert!(after_first > 0, "the first ingest stored chunks");
    let after_second = count(&conn);
    assert_eq!(after_first, after_second, "the duplicate inserts nothing");
    assert_eq!(r2["entries_added"].as_i64().unwrap_or(0), 0);
}

// ── the named refusals ───────────────────────────────────────────────────────

#[tokio::test]
async fn scanned_fixture_refuses_no_text_layer() {
    let s = server();
    let (status, resp) =
        post_document(&s.state, document_body("scanned.pdf", "Scan Fixture")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {resp}");
    assert_eq!(
        code_of(&resp),
        "no_text_layer",
        "the refusal is NAMED: {resp}"
    );
    // nothing stored
    let conn = s.state.pool.get().unwrap();
    let docs: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
        .unwrap();
    assert_eq!(docs, 0, "a refused document stores no row");
}

#[tokio::test]
async fn garbage_refuses_invalid_document() {
    let s = server();
    // not base64
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": "!!!not base64!!!", "title": "t" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
    assert_eq!(code_of(&resp), "invalid_document", "{resp}");
    // valid base64, not a PDF
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": b64(b"this is not a pdf"), "title": "t" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
    assert_eq!(code_of(&resp), "invalid_document", "{resp}");
}

#[tokio::test]
async fn over_cap_refuses_document_too_large_never_truncates() {
    let s = server();
    // a payload over the shared 1 MiB cap (MAX_REQUEST_SIZE).
    let big = vec![b'x'; brain_server::config::MAX_REQUEST_SIZE + 1];
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": b64(&big), "title": "t" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "body: {resp}");
    assert_eq!(
        code_of(&resp),
        "document_too_large",
        "NAMED, never silent: {resp}"
    );
}

#[tokio::test]
async fn a_pdf_that_panics_the_extractor_refuses_named_never_kills_the_server() {
    // THE ISOLATION PIN (measured 2026-11-07): hostile-font-panic.pdf trips
    // pdf-extract's `expect("missing unicode map and encoding")` — a
    // legitimate arXiv paper (2610.28425) trips the same expect. Under the
    // release profile's panic = "abort", an in-process parse would kill the
    // SERVER. The parse runs in the extract-document child; its abort kills
    // only the child, and this route answers the named refusal. The server
    // answering the NEXT request is the whole point.
    let s = server();
    let (status, resp) = post_document(
        &s.state,
        document_body("hostile-font-panic.pdf", "Hostile Fixture"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
    assert_eq!(code_of(&resp), "invalid_document", "NAMED: {resp}");
    // the server is alive: the very next request answers normally.
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": "!!!not base64!!!", "title": "t" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "server alive: {resp}");
    assert_eq!(code_of(&resp), "invalid_document");
}

#[tokio::test]
async fn the_tier2_variant_refuses_named_in_every_environment() {
    // THE TIER-2 ROUTE LAW (tightened in IMPROVE06 — the 501 boundary is
    // gone): with nothing configured, tier=docling answers EXACTLY
    // converter_unavailable (503) — the improve05-era tolerance (either this
    // or the old boundary refusal) is retired with the boundary itself. The
    // configured path's real pipeline and the named fallbacks are the DC
    // pins' (tests/docling_pins.rs); the pure probe arms stay pinned in
    // ingest_document's tests.
    let s = server();
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "t",
            "tier": "docling"
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "the unconfigured Tier-2 posture is 503: {status} {resp}"
    );
    assert_eq!(
        code_of(&resp),
        "converter_unavailable",
        "the refusal names the posture: {resp}"
    );
    // Tier 0 is unaffected by any of it — the default still extracts.
    let (status, resp) =
        post_document(&s.state, document_body("table.pdf", "Tier0 Still Default")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "Tier 0 needs no configuration: {resp}"
    );
}

#[tokio::test]
async fn unsupported_tier_refuses_named() {
    let s = server();
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": b64(&fixture("table.pdf")), "title": "t", "tier": "warp" })
            .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
    assert_eq!(code_of(&resp), "unsupported_tier", "{resp}");
}

// ── the doors (S2's subset of DI-5) ─────────────────────────────────────────

#[tokio::test]
async fn a_think_trace_inside_the_pdf_does_not_survive_ingest() {
    let s = server();
    let pdf = tiny_pdf(&[
        "Public reasoning about the case.",
        "<think>the model's private deliberation draft</think>",
        "The conclusion follows from the stored facts.",
    ]);
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": b64(&pdf), "title": "Trace Fixture" }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {resp}");
    let md = resp["markdown"].as_str().unwrap();
    assert!(
        !md.contains("private deliberation draft"),
        "the trace strip (the /add pre-insert door) applies to the extracted text: {md}"
    );
    assert!(
        md.contains("The conclusion follows"),
        "the clean text survives"
    );
}

#[tokio::test]
async fn title_and_source_bounds_apply_to_the_document_door() {
    let s = server();
    // empty title (the markdown door's law)
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({ "data": b64(&fixture("table.pdf")), "title": "  " }).to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
    // over-long trust label (the MAX_SOURCE law)
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "t",
            "source": "x".repeat(brain_server::handlers::MAX_SOURCE + 1)
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {resp}");
}
