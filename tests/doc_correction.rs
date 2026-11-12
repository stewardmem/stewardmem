// IMPROVE16 S2 — the doc-correction lane, driven end to end.
//
// ## Why this file exists
//
// `correct: true` on `POST /ingest/document` is the lane's ONE door, and a
// governance lane is exactly the kind of seam whose defects are invisible
// to unit tests of the client: they only exist where the doors compose
// (the verbatim pipeline, the provider call, the cage, the two rows, the
// stamps, the HITL posture). These tests drive the REAL route over the
// REAL migrated schema with the REAL doors and a scripted loopback
// corrector — recorded responses, zero live calls.
//
// ## Red-first
//
// Every pin here was born against the PRE-S2 tree (S1 committed, no route
// lane) and red there BEHAVIORALLY — the route ignored `correct` and
// returned the verbatim-only shape, so each pin's expected contract was
// observed absent by name (the transcript is the round's evidence file).
// The non-vacuity control: pin 2 (`the_default_pipeline_has_no_correction_surface`)
// is GREEN both before and after — a scan with no planted-miss control
// proves nothing.

use std::io::{Read, Write};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const OP_TOKEN: &str = "improve16-doc-correction-op-token";

/// A minimal one-page text-layer PDF (the document_ingestion.rs vehicle —
/// real bytes on the wire, not mocks).
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

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The scripted loopback corrector: one raw response per connection,
/// recording every request body. The S1 fixture posture, route-sized.
struct ScriptServer {
    addr: std::net::SocketAddr,
    received: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
}

impl ScriptServer {
    fn spawn(script: Vec<String>) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
        let addr = listener.local_addr().unwrap();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);
        std::thread::spawn(move || {
            for body in script {
                let Ok((mut sock, _)) = listener.accept() else {
                    return;
                };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                loop {
                    let n = sock.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let raw = String::from_utf8_lossy(&buf).to_string();
                if let Some(Ok(v)) = raw.split("\r\n\r\n").nth(1).map(serde_json::from_str) {
                    received_clone.lock().unwrap().push(v);
                }
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes());
                let _ = sock.shutdown(std::net::Shutdown::Both);
            }
        });
        Self { addr, received }
    }

    fn completion(content: &str) -> String {
        serde_json::json!({
            "id": "cmpl-fixture",
            "model": "fixture-model@v9",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        })
        .to_string()
    }
}

/// The corrected one-page body the WELL-BEHAVED fixture returns: the
/// structure kept byte-identical, one word fixed.
fn corrected_page_body() -> String {
    "<!-- doc: whatever-the-corrector-says | source: document | pages: 1 | tier: text-layer\n     extracted: wrong | extractor: wrong | layout: unverified -->\n## Page 1\nThe quick brown fox jumps over the lazy dog.\n<!-- anchor: p1 -->\n"
        .to_string()
}

/// The structure-breaking fixture: the page block lost its anchor (the
/// mutation the cage must refuse BY NAME).
fn anchorless_page_body() -> String {
    "## Page 1\nThe quick brown fox jumps over the lazy dog.\n".to_string()
}

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

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

fn document_body(pdf: &[u8], title: &str, correct: bool) -> String {
    serde_json::json!({
        "data": b64(pdf),
        "title": title,
        "correct": correct,
    })
    .to_string()
}

fn code_of(resp: &serde_json::Value) -> String {
    resp["error"]["code"]
        .as_str()
        .or_else(|| resp["code"].as_str())
        .unwrap_or("<no code>")
        .to_string()
}

fn document_count(server: &Server) -> i64 {
    server
        .state
        .pool
        .get()
        .expect("conn")
        .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
        .expect("count")
}

/// The env discipline: the four profile vars are process-global and the
/// tests below run in parallel — every env-touching test holds the lock.
static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn clear_corrector_env() {
    for name in [
        "BRAIN_DOC_CORRECTOR_BASE_URL",
        "BRAIN_DOC_CORRECTOR_MODEL",
        "BRAIN_DOC_CORRECTOR_SECRET_FILE",
        "BRAIN_DOC_CORRECTOR_SECRET_ROOT",
    ] {
        // SAFETY: under ENV_LOCK; no other test reads these concurrently.
        unsafe {
            std::env::remove_var(name);
        }
    }
}

fn set_corrector_env(base_url: &str, root: &std::path::Path) {
    // SAFETY: as above.
    unsafe {
        std::env::set_var("BRAIN_DOC_CORRECTOR_BASE_URL", base_url);
        std::env::set_var("BRAIN_DOC_CORRECTOR_MODEL", "fixture-model");
        std::env::set_var("BRAIN_DOC_CORRECTOR_SECRET_FILE", "corrector.key");
        std::env::set_var(
            "BRAIN_DOC_CORRECTOR_SECRET_ROOT",
            root.display().to_string(),
        );
    }
}

fn secret_root(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let root = dir.path().join("corrector-root");
    std::fs::create_dir_all(&root).expect("mkdir root");
    let secret = root.join("corrector.key");
    std::fs::write(&secret, "fixture-secret").expect("write secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600))
            .expect("chmod secret");
    }
    root
}

// ── CA-1: the lane is OFF unless configured ──────────────────────────────────

/// Unconfigured + `correct: true` → the NAMED refusal, and NOTHING lands:
/// no corrected row, no verbatim row (the request was correction-shaped;
/// a silent verbatim-only insert would hide the operator's intent).
#[tokio::test]
async fn unconfigured_correct_refuses_named_and_nothing_lands() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA1", true)).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "the unconfigured lane refuses 503, got {status} {resp}"
    );
    assert_eq!(code_of(&resp), "corrector_unconfigured");
    assert_eq!(document_count(&server), 0, "nothing lands on the refusal");
}

/// The non-vacuity control: WITHOUT the flag the pipeline is exactly the
/// pre-lane shape — success, no correction surface in the response, one
/// verbatim row. GREEN before AND after the lane lands.
#[tokio::test]
async fn the_default_pipeline_has_no_correction_surface() {
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA1b", false)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the default ingest succeeds: {resp}"
    );
    assert_eq!(resp["status"], "created");
    assert!(
        resp.get("corrected").is_none(),
        "the default response carries no correction surface"
    );
    assert_eq!(document_count(&server), 1);
}

// ── CA-5: the provider posture at the route ──────────────────────────────────

/// A PARTIAL profile (three of four) is a named configuration refusal at
/// the route, never a half-built client.
#[tokio::test]
async fn a_partial_profile_refuses_named_at_the_route() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    // SAFETY: under ENV_LOCK.
    unsafe {
        std::env::set_var("BRAIN_DOC_CORRECTOR_BASE_URL", "http://127.0.0.1:1/v1");
        std::env::set_var("BRAIN_DOC_CORRECTOR_MODEL", "fixture-model");
        std::env::set_var("BRAIN_DOC_CORRECTOR_SECRET_FILE", "corrector.key");
        // SECRET_ROOT missing — the incomplete half.
    }
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA5", true)).await;
    clear_corrector_env();
    assert_eq!(status, StatusCode::BAD_REQUEST, "got {status} {resp}");
    assert_eq!(code_of(&resp), "corrector_config_invalid");
}

/// The audit row per call: a corrected ingest writes kind `doc-correction`
/// targeting the verbatim markdown's sha (the corrector's INPUT).
#[tokio::test]
async fn a_corrected_ingest_writes_the_audit_row_per_call() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = secret_root(&dir);
    let verbatim_lines = &["The quick brown fox."];
    let corrected = format!(
        "## Page 1\n{}\n<!-- anchor: p1 -->\n",
        "The quick brown fox jumps."
    );
    let server_fixture = ScriptServer::spawn(vec![ScriptServer::completion(&corrected)]);
    set_corrector_env(&format!("http://{}/v1", server_fixture.addr), &root);
    let server = server();
    let pdf = tiny_pdf(verbatim_lines);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA5b", true)).await;
    clear_corrector_env();
    assert_eq!(
        status,
        StatusCode::OK,
        "the corrected ingest succeeds: {resp}"
    );
    let corrected_resp = resp
        .get("corrected")
        .expect("the response names the corrected artifact");
    let input_sha = corrected_resp["sha256"]
        .as_str()
        .expect("the input sha rides");
    let (kinds, targets): (String, String) = server
        .state
        .pool
        .get()
        .expect("conn")
        .query_row(
            "SELECT kind, target_hash FROM audit_events WHERE kind = 'doc-correction'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the doc-correction audit row exists");
    assert_eq!(kinds, "doc-correction");
    assert_eq!(
        targets,
        brain_server::audit::hash(input_sha),
        "the call is audited against the VERBATIM markdown sha"
    );
}

// ── CA-2 + CA-4: two artifacts, one lineage, the stamps end-to-end ───────────

/// The corrected ingest lands TWO rows: the verbatim row byte-unchanged
/// (CA-2) and the corrected row stamped end-to-end (CA-4) — the header's
/// `corrector:` == the row's extractor_version == the served stamp, the
/// corrected tier, and the corrected row's sha256 IS the verbatim
/// markdown's sha (the lineage anchor: the row records what was corrected,
/// its markdown_sha256 records what was produced).
#[tokio::test]
async fn corrected_ingest_lands_two_artifacts_with_the_stamps_end_to_end() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = secret_root(&dir);
    let corrected = format!(
        "<!-- doc: x | source: document | pages: 1 | tier: text-layer -->\n{}",
        corrected_page_body()
    );
    let server_fixture = ScriptServer::spawn(vec![ScriptServer::completion(&corrected)]);
    set_corrector_env(&format!("http://{}/v1", server_fixture.addr), &root);
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA2", true)).await;
    clear_corrector_env();
    assert_eq!(status, StatusCode::OK, "{resp}");
    let corrected_resp = resp
        .get("corrected")
        .expect("the response names the corrected artifact");
    let verbatim_sha = resp["sha256"].as_str().expect("the verbatim sha");
    let input_sha = corrected_resp["sha256"]
        .as_str()
        .expect("the input sha rides");
    let stamp = corrected_resp["corrector"]
        .as_str()
        .expect("the stamp rides");
    // The corrected row's key is NOT the verbatim row's key (the PDF sha):
    // two rows, two keys — the lineage lives INSIDE the corrected row
    // (sha256 = sha(M_v)), asserted at the row level below.
    assert_ne!(input_sha, verbatim_sha, "two artifacts carry two keys");
    assert!(
        stamp.starts_with("fixture-model@"),
        "the stamp is <model>@<served>: {stamp}"
    );
    // Two rows, distinct shapes.
    assert_eq!(document_count(&server), 2, "two artifacts");
    // CA-2: the verbatim row's markdown_sha unchanged after the corrected insert.
    let conn = server.state.pool.get().expect("conn");
    let verbatim: (String, String, String, String) = conn
        .query_row(
            "SELECT tier, extractor, extractor_version, markdown_sha256 FROM documents WHERE sha256 = ?1",
            [verbatim_sha],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("the verbatim row exists");
    assert_eq!(verbatim.0, "text-layer", "the verbatim tier is untouched");
    assert_eq!(
        verbatim.1, "pdf-extract",
        "the verbatim extractor is untouched"
    );
    let corrected_row: (String, String, String, String, String) = conn
        .query_row(
            "SELECT tier, extractor, extractor_version, sha256, markdown_sha256 FROM documents WHERE extractor = 'doc-corrector'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .expect("the corrected row exists");
    assert_eq!(
        corrected_row.0, "text-layer-corrected",
        "the corrected tier"
    );
    assert_eq!(corrected_row.1, "doc-corrector", "the corrected extractor");
    assert_eq!(
        corrected_row.2, stamp,
        "the row stamp == the response stamp == the header stamp"
    );
    eprintln!("DEBUG verbatim row: {:?}", verbatim);
    eprintln!("DEBUG corrected row: {:?}", corrected_row);
    eprintln!(
        "DEBUG resp sha256: {:?} corrected_resp: {:?}",
        resp["sha256"], corrected_resp
    );
    assert_eq!(
        corrected_row.3, verbatim.3,
        "the corrected row's sha256 IS sha(M_v) — the lineage anchor: the row records what was corrected, its markdown_sha256 what was produced"
    );
    drop(conn);
}

// ── CA-3: the structure is the corrector's cage ──────────────────────────────

/// A correction that drops the page's anchor refuses NAMED
/// (`correction_contract_violation`), the verbatim row is whole, and NO
/// corrected row lands.
#[tokio::test]
async fn a_structure_breaking_correction_refuses_named_and_leaves_the_verbatim_whole() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = secret_root(&dir);
    let server_fixture =
        ScriptServer::spawn(vec![ScriptServer::completion(&anchorless_page_body())]);
    set_corrector_env(&format!("http://{}/v1", server_fixture.addr), &root);
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA3", true)).await;
    clear_corrector_env();
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "got {status} {resp}"
    );
    assert_eq!(code_of(&resp), "correction_contract_violation");
    // The verbatim row survived (two artifacts: a refused correction is
    // not a destroyed verbatim).
    assert!(
        document_count(&server) >= 1,
        "the verbatim row survives the refused correction"
    );
    let conn = server.state.pool.get().expect("conn");
    let corrected_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM documents WHERE extractor = 'doc-corrector'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(
        corrected_rows, 0,
        "a refused correction lands NO corrected row"
    );
}

// ── CA-6: the review posture — proposals, nothing auto-promotes ──────────────

/// Under the review posture a corrected ingest files BOTH variants' chunks
/// as proposals and inserts NOTHING.
#[tokio::test]
async fn review_posture_proposes_both_variants_and_promotes_nothing() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = secret_root(&dir);
    let corrected = format!(
        "<!-- doc: x | source: document | pages: 1 | tier: text-layer -->\n{}",
        corrected_page_body()
    );
    let server_fixture = ScriptServer::spawn(vec![ScriptServer::completion(&corrected)]);
    set_corrector_env(&format!("http://{}/v1", server_fixture.addr), &root);
    let prev = std::env::var("BRAIN_WRITE_POSTURE").ok();
    // SAFETY: under ENV_LOCK; restored below.
    unsafe {
        std::env::set_var("BRAIN_WRITE_POSTURE", "review");
    }
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status, resp) = post_document(&server.state, document_body(&pdf, "CA6", true)).await;
    clear_corrector_env();
    match prev {
        Some(v) => unsafe { std::env::set_var("BRAIN_WRITE_POSTURE", v) },
        None => unsafe { std::env::remove_var("BRAIN_WRITE_POSTURE") },
    }
    assert_eq!(status, StatusCode::ACCEPTED, "got {status} {resp}");
    assert_eq!(resp["status"], "proposal_pending");
    let verbatim_ids = resp["proposal_ids"].as_array().expect("verbatim proposals");
    let corrected_ids = resp["corrected_proposal_ids"]
        .as_array()
        .expect("the corrected variant files its OWN proposals");
    assert!(
        !verbatim_ids.is_empty() && !corrected_ids.is_empty(),
        "both variants propose"
    );
    assert_eq!(document_count(&server), 0, "nothing auto-promotes");
    let knowledge: i64 = server
        .state
        .pool
        .get()
        .expect("conn")
        .query_row(
            "SELECT COUNT(*) FROM knowledge WHERE source = 'document'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(knowledge, 0);
}

// ── the corrected duplicate ──────────────────────────────────────────────────

/// A second `correct: true` for a document that ALREADY has a corrected
/// row is a READ-class re-serve: 200 duplicate, and the additive
/// `corrected` block NAMES the existing artifact (status exists, the SAME
/// id) — a re-request never silently re-corrects, never overwrites, and
/// the corrector is never called twice for the same bytes (the
/// two-artifact law's dedupe posture; §1.5: re-correction is an explicit
/// data act). RESHAPED FROM THE RED PIN (which expected a 409): the tree
/// taught that the verbatim duplicate path short-circuits any re-request,
/// so the exists-naming lives in the duplicate response, not a conflict.
#[tokio::test]
async fn a_second_corrected_request_names_the_existing_artifact() {
    let _env = ENV_LOCK.lock().await;
    clear_corrector_env();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let root = secret_root(&dir);
    let corrected = format!(
        "<!-- doc: x | source: document | pages: 1 | tier: text-layer -->\n{}",
        corrected_page_body()
    );
    let server_fixture = ScriptServer::spawn(vec![
        ScriptServer::completion(&corrected),
        ScriptServer::completion(&corrected),
    ]);
    set_corrector_env(&format!("http://{}/v1", server_fixture.addr), &root);
    let server = server();
    let pdf = tiny_pdf(&["The quick brown fox."]);
    let (status1, resp1) = post_document(&server.state, document_body(&pdf, "DUP", true)).await;
    assert_eq!(status1, StatusCode::OK, "{resp1}");
    let first_id = resp1["corrected"]["document_id"]
        .as_i64()
        .expect("the first corrected id");
    let calls_after_first = server_fixture.received.lock().unwrap().len();
    let (status2, resp2) = post_document(&server.state, document_body(&pdf, "DUP", true)).await;
    clear_corrector_env();
    assert_eq!(
        status2,
        StatusCode::OK,
        "the re-serve is a duplicate, got {status2} {resp2}"
    );
    assert_eq!(resp2["status"], "duplicate");
    let corrected_block = resp2
        .get("corrected")
        .expect("the corrected artifact is named");
    assert_eq!(corrected_block["status"], "exists");
    assert_eq!(
        corrected_block["document_id"].as_i64(),
        Some(first_id),
        "the SAME corrected row is named, never a second one"
    );
    let calls_after_second = server_fixture.received.lock().unwrap().len();
    assert_eq!(
        calls_after_first, calls_after_second,
        "the corrector is never called twice for the same bytes"
    );
}
