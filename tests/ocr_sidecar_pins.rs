// IMPROVE05 S3 — the OV pins, red-first: each written against the pre-pin
// tree, each reds BY NAME, then green.
//
// ## The red ledger (measured 2026-11-08, against the S2 tree)
//
// - OV-4 red: `expected 503 ocr_unavailable, got 400 unsupported_tier` —
//   the cleanest red: the refusal did not exist.
// - OV-1 red: `expected 200 with a per-page tier map, got 400
//   unsupported_tier` — the route had no ocr arm, so the hybrid's stub run
//   never happened (the stub's record file was never written).
// - OV-2 red: same run — the row read failed `no such column: page_tiers`
//   and no pages-tier header field existed to compare against.
// - OV-3 red: same run — no low-confidence marker, no row count.
// - OV-5 behavioral red: same run — the pipeline never ran, so no verbatim
//   text could be asserted; structural arm green-on-arrival (the S2
//   sidecar carries no correction path) with the mutation red-proof
//   recorded in IMPROVE05_EVIDENCE_2026-11-08.md.
// - OV-6 green-on-arrival (the S2 source is sequential by construction)
//   with the mutation red-proof recorded in the evidence.
// - The pure-probe pins (OcrProbe's three postures) and the engine-mismatch
//   re-serve arm green-on-arrival with the implementation — their red
//   proofs are BY MUTATION, same as improve04's DI-2/DI-3.
//
// The stub sidecar: a POSIX-sh script the test writes into a temp dir —
// it reads the request line, drains the PDF bytes, records the request
// when OCR_STUB_RECORD names a file, and prints ONE canned reply line with
// the request's id echoed. The kernel cannot tell it from the real
// sidecar; that is the point: the kernel-side routing law is testable
// without macOS, deterministically.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/documents");
const OP_TOKEN: &str = "improve05-ocr-op-token";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture in the corpus")
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

/// The same harness shape as `tests/document_ingestion.rs`: an opaque-mode
/// server on a real file-backed, real migrated database.
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

fn code_of(resp: &serde_json::Value) -> String {
    resp["error"]["code"]
        .as_str()
        .or_else(|| resp["code"].as_str())
        .unwrap_or("<no code>")
        .to_string()
}

/// A wire-level header-field reader: the value after `key:` in the header
/// comment (the union across the two physical lines, the contract's parse
/// shape). Deliberately LOCAL — the pins compile against the pre-pin tree
/// and read the WIRE, not the kernel's parser (which gains the new fields
/// in the same commit that satisfies the pins).
fn header_field(markdown: &str, key: &str) -> Option<String> {
    let start = markdown.find("<!-- doc: ")?;
    let end = markdown[start..].find("-->")?;
    for line in markdown[start..start + end].lines() {
        for part in line.split('|') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once(':')
                && k.trim() == key
            {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

// ── the BRAIN_VISION_BRIDGE env seam ────────────────────────────────────────
// The probe is read PER REQUEST (the chat-tier idiom, tests/common), and
// cargo runs a crate's tests concurrently — an ocr-tier test that pointed
// the bridge at a stub while another asserted the absence posture would be
// a scheduling flake, not a pin. Every ocr-tier test holds this lock for
// its whole body; the guard restores BOTH keys on drop.

fn vision_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Holds the lock + restores both env keys on drop. (The chat_tier guard
/// shape, doubled for the stub's record path.)
struct VisionEnv {
    _guard: MutexGuard<'static, ()>,
    saved_bridge: Option<String>,
    saved_record: Option<String>,
    /// The stub's home: held alive for the run's whole body (dropping it
    /// deletes the stub before the route probes it — measured).
    _dir: Option<tempfile::TempDir>,
}

impl Drop for VisionEnv {
    fn drop(&mut self) {
        match &self.saved_bridge {
            Some(v) => unsafe { std::env::set_var("BRAIN_VISION_BRIDGE", v) },
            None => unsafe { std::env::remove_var("BRAIN_VISION_BRIDGE") },
        }
        match &self.saved_record {
            Some(v) => unsafe { std::env::set_var("OCR_STUB_RECORD", v) },
            None => unsafe { std::env::remove_var("OCR_STUB_RECORD") },
        }
    }
}

/// Point BRAIN_VISION_BRIDGE at a recording stub; return where the request
/// line will land. The lock is held for the run's whole body.
fn stub_env(reply: &str) -> (VisionEnv, PathBuf) {
    let guard = vision_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = tempfile::TempDir::new().expect("stub dir");
    let stub = write_stub(dir.path(), reply);
    let record = dir.path().join("request-record.json");
    let env = VisionEnv {
        saved_bridge: std::env::var("BRAIN_VISION_BRIDGE").ok(),
        saved_record: std::env::var("OCR_STUB_RECORD").ok(),
        _guard: guard,
        _dir: Some(dir),
    };
    unsafe { std::env::set_var("OCR_STUB_RECORD", &record) };
    unsafe { std::env::set_var("BRAIN_VISION_BRIDGE", stub.to_str().expect("utf8 stub")) };
    (env, record)
}

/// Acquire the lock with the bridge pointing at a path that does not
/// exist — the DETERMINISTIC absence posture (the env override wins over
/// the sibling/dev-tree resolution, so this forces the PathMissing arm on
/// every tree, including a macOS tree that has built the real sidecar).
fn missing_env() -> VisionEnv {
    let guard = vision_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let env = VisionEnv {
        saved_bridge: std::env::var("BRAIN_VISION_BRIDGE").ok(),
        saved_record: std::env::var("OCR_STUB_RECORD").ok(),
        _guard: guard,
        _dir: None,
    };
    unsafe { std::env::set_var("BRAIN_VISION_BRIDGE", "/nonexistent/shell-vision") };
    unsafe { std::env::remove_var("OCR_STUB_RECORD") };
    env
}

/// Acquire the lock with the bridge ABSENT (no stub) — the pure absence
/// posture for pins that must not race the stub pins.
fn absent_env() -> VisionEnv {
    let guard = vision_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let env = VisionEnv {
        saved_bridge: std::env::var("BRAIN_VISION_BRIDGE").ok(),
        saved_record: std::env::var("OCR_STUB_RECORD").ok(),
        _guard: guard,
        _dir: None,
    };
    unsafe { std::env::remove_var("BRAIN_VISION_BRIDGE") };
    unsafe { std::env::remove_var("OCR_STUB_RECORD") };
    env
}

fn recorded_request(record: &std::path::Path) -> serde_json::Value {
    let raw = std::fs::read_to_string(record).expect("the stub recorded a request");
    serde_json::from_str(&raw).expect("the recorded request line is JSON")
}

// ── the stub sidecar ─────────────────────────────────────────────────────────

/// Write a one-shot stub sidecar: reads the request line, drains the PDF
/// bytes, records the request line when OCR_STUB_RECORD names a file, and
/// answers with `reply` (ONE JSON line; the `__ID__` token becomes the
/// request's echoed id).
fn write_stub(dir: &std::path::Path, reply: &str) -> PathBuf {
    assert!(!reply.contains('\''), "the stub reply must be sh-safe");
    let path = dir.join("shell-vision-stub");
    let script = "#!/bin/sh\n\
         # the OV-pin stub sidecar (IMPROVE05 S3): the kernel-side routing law\n\
         # is testable without macOS, deterministically.\n\
         IFS= read -r request_line\n\
         cat > /dev/null\n\
         if [ -n \"${OCR_STUB_RECORD:-}\" ]; then\n\
         \x20 printf '%s' \"$request_line\" > \"$OCR_STUB_RECORD\"\n\
         fi\n\
         req_id=$(printf '%s' \"$request_line\" | sed -n 's/.*\"id\":\"\\([^\"]*\\)\".*/\\1/p')\n\
         reply='__REPLY__'\n\
         printf '%s\\n' \"$reply\" | sed \"s/__ID__/$req_id/\"\n"
        .replace("__REPLY__", reply);
    let _ = std::fs::write("/tmp/stubtest/actual-stub", &script);
    std::fs::write(&path, script).expect("stub written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("stub executable");
    }
    path
}

/// Build a canned ok-reply with the `__ID__` token where the echoed id goes.
fn canned_reply(pages: serde_json::Value) -> String {
    serde_json::json!({
        "engine": "apple-vision/recognize-text@STUBBUILD",
        "id": "__ID__",
        "ok": true,
        "pages": pages,
    })
    .to_string()
}

/// The canned reply for the hybrid's image-only pages (2 and 4): page 2 at
/// 0.9 confidence (above the floor), page 4 at 0.2 (below).
fn hybrid_stub_reply() -> String {
    canned_reply(serde_json::json!([
        {"confidence": 0.9, "index": 2, "text": "scanned insert one"},
        {"confidence": 0.2, "index": 4, "text": "scanned insert two"}
    ]))
}

// ── OV-4 — the absence posture (the Linux-green path) ────────────────────────

/// The bridge is ABSENT (the operator's env names a path that does not
/// exist — the deterministic stand-in for "no sidecar installed", the CI
/// posture): the ocr tier refuses NAMED 503 `ocr_unavailable`, naming the
/// fix. Tier 0 and the docling arm are unaffected — the three probes are
/// independent.
#[tokio::test]
async fn ocr_tier_refuses_ocr_unavailable_when_the_bridge_is_absent() {
    let s = server();
    let _env = missing_env();
    let body = serde_json::json!({
        "data": b64(&fixture("hybrid.pdf")),
        "title": "OV-4",
        "tier": "ocr",
    })
    .to_string();
    let (status, resp) = post_document(&s.state, body).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "expected 503 ocr_unavailable, got {status} {resp}"
    );
    assert_eq!(
        code_of(&resp),
        "ocr_unavailable",
        "OV-4's exact name — NOT the docling arm's converter_unavailable"
    );
}

/// The three probes are independent: a configured OCR bridge does not fix
/// the docling arm, an absent bridge does not touch Tier 0, and the ocr
/// refusal is the ocr seam's own name — the arms share nothing.
#[tokio::test]
async fn the_three_probes_are_independent() {
    let s = server();

    // Tier 0 with the bridge absent: extracts (the zero-configuration default).
    {
        let _env = absent_env();
        let (status, resp) = post_document(
            &s.state,
            serde_json::json!({
                "data": b64(&fixture("single-column.pdf")),
                "title": "OV-4 independence: tier 0",
            })
            .to_string(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "Tier 0 is never touched: {resp}");
        assert_eq!(resp["tier"], "text-layer");
    }

    // The docling arm with the OCR bridge CONFIGURED: still the converter's
    // refusal — a configured vision bridge satisfies nothing else.
    {
        let (_env, _record) = stub_env(&hybrid_stub_reply());
        let body = serde_json::json!({
            "data": b64(&fixture("single-column.pdf")),
            "title": "OV-4 independence: docling",
            "tier": "docling",
        })
        .to_string();
        let (status, resp) = post_document(&s.state, body).await;
        assert_eq!(
            code_of(&resp),
            "converter_unavailable",
            "the docling arm probes its own seam (BRAIN_DOCLING_PATH), got {resp}"
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // The ocr arm with the bridge absent: the ocr seam's OWN refusal name —
    // never the docling arm's.
    {
        let _env = missing_env();
        let body = serde_json::json!({
            "data": b64(&fixture("hybrid.pdf")),
            "title": "OV-4 independence: ocr",
            "tier": "ocr",
        })
        .to_string();
        let (status, resp) = post_document(&s.state, body).await;
        assert_eq!(
            code_of(&resp),
            "ocr_unavailable",
            "the ocr arm probes its own seam, got {resp}"
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
}

// ── OV-1 + OV-2 + OV-3 — the hybrid routing, the map, the floor ─────────────

/// OV-1: the stub's RECORDED request names EXACTLY the empty pages [2, 4] —
/// the text-layer pages (1, 3) are never sent. OV-2: the response header's
/// per-page map, the row's map, and the recorded request agree three ways.
/// OV-3: the below-floor page carries `low-confidence` in its block; the
/// above-floor page does not; the row counts exactly one.
#[tokio::test]
async fn the_hybrid_routes_by_per_page_emptiness_and_the_map_agrees() {
    let s = server();
    let (_env, record) = stub_env(&hybrid_stub_reply());
    let body = serde_json::json!({
        "data": b64(&fixture("hybrid.pdf")),
        "title": "OV-1/OV-2/OV-3",
        "tier": "ocr",
    })
    .to_string();
    let (status, resp) = post_document(&s.state, body).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "expected the ocr pipeline, got {status} {resp}"
    );
    let markdown = resp["markdown"].as_str().expect("markdown in the response");
    let sha = resp["sha256"].as_str().expect("sha");

    // OV-1: the request named only the empty pages.
    let request = recorded_request(&record);
    assert_eq!(
        request["pages"],
        serde_json::json!([2, 4]),
        "the sidecar request names EXACTLY the image-only pages, got: {request}"
    );
    assert_eq!(request["op"], "ocr");
    assert_eq!(request["dpi"], 300, "the kernel sends its named constant");

    // OV-2 (header leg): the map marks the split, wire-level.
    let map = header_field(markdown, "pages-tier")
        .expect("the ocr document's header carries the per-page tier map");
    assert_eq!(
        map, "text-layer,ocr,text-layer,ocr",
        "the map proves the split, got: {map}"
    );
    assert_eq!(
        header_field(markdown, "ocr-engine").as_deref(),
        Some("apple-vision/recognize-text@STUBBUILD"),
        "the header names the engine that ran"
    );
    assert_eq!(
        header_field(markdown, "tier").as_deref(),
        Some("mixed"),
        "the derived tier reflects the mix"
    );

    // OV-2 (row leg): the row's map agrees with the header's.
    let conn = s.state.pool.get().unwrap();
    let (row_tier, row_map, row_low, row_engine): (String, String, i64, String) = conn
        .query_row(
            "SELECT tier, page_tiers, low_confidence_pages, ocr_engine
             FROM documents WHERE sha256 = ?1",
            [sha],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("the ocr document row exists");
    assert_eq!(row_tier, "mixed");
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&row_map).expect("row map is a JSON array"),
        ["text-layer", "ocr", "text-layer", "ocr"],
        "OV-2: the row's map is the header's map"
    );
    assert_eq!(
        row_engine, "apple-vision/recognize-text@STUBBUILD",
        "the row pins the engine"
    );

    // OV-3: the below-floor page carries the marker; the above-floor page
    // does not; the row counts exactly one.
    let page2 = markdown
        .split("## Page 2\n")
        .nth(1)
        .expect("page 2 block")
        .split("## Page 3")
        .next()
        .expect("page 2 block ends");
    let page4 = markdown
        .split("## Page 4\n")
        .nth(1)
        .expect("page 4 block")
        .to_string();
    assert!(
        !page2.contains("low-confidence"),
        "page 2 (0.9 >= the floor) is clean"
    );
    assert!(
        page4.contains("low-confidence"),
        "page 4 (0.2 < the floor) is flagged"
    );
    assert!(
        page2.contains("scanned insert one"),
        "the OCR text is in the block"
    );
    assert_eq!(
        row_low, 1,
        "the row's low-confidence count is exactly the flagged page"
    );
    assert!(
        page2.contains("confidence 0.9") && page4.contains("confidence 0.2"),
        "the confidence values ride the markers"
    );
    assert!(
        markdown.contains("confidence 0.9"),
        "the above-floor confidence is recorded, not hidden"
    );
}

// ── OV-5 — verbatim entry, no correction path ────────────────────────────────

/// The sidecar's text enters the assembled markdown VERBATIM — oddities and
/// all. No smoothing, no spell-fix, no rewrite exists between the reply and
/// the stored markdown (arXiv:2611.05539's class, refused by construction).
#[tokio::test]
async fn ocr_text_enters_verbatim_never_corrected() {
    let s = server();
    let odd = "The quik brown fox  jumps over the lazy dogg";
    let reply = canned_reply(serde_json::json!([
        {"confidence": 0.7, "index": 2, "text": odd}
    ]));
    let (_env, _record) = stub_env(&reply);
    let body = serde_json::json!({
        "data": b64(&fixture("hybrid.pdf")),
        "title": "OV-5 verbatim",
        "tier": "ocr",
    })
    .to_string();
    let (status, resp) = post_document(&s.state, body).await;
    assert_eq!(status, StatusCode::OK, "the pipeline ran: {resp}");
    let markdown = resp["markdown"].as_str().expect("markdown");
    assert!(
        markdown.contains(odd),
        "the oddities survive byte-for-byte — no correction path touched them"
    );
    assert!(
        !markdown.contains("quick brown fox"),
        "the corrected spelling does not exist in the output"
    );

    // the structural half: the sidecar's import list is CLOSED — no model
    // provider, no network surface, no correction machinery can hide behind
    // an import the pin does not know.
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shell-macos/Sources/shell-vision/main.swift"
    ))
    .expect("the sidecar source is in this tree");
    let imports: Vec<String> = source
        .lines()
        .filter_map(|l| l.trim().strip_prefix("import "))
        .map(|l| l.trim().to_string())
        .collect();
    assert_eq!(
        imports,
        vec!["CoreGraphics", "CoreText", "Foundation", "Vision"],
        "the sidecar's imports are exactly the rasterization+recognition set — \
         CoreText joined for the capability probe's synthetic stimulus (the \
         2026-11-14 ocr_assets_unavailable verdict renders text to measure \
         against); a network or model import is a found bug: {imports:?}"
    );
}

// ── OV-6 — sequential recognition (structural) ──────────────────────────────

/// The sidecar performs ONE Vision request at a time: the concurrency
/// constructs that could parallelize the loop do not exist in its source.
/// (The measured hazard is the plan's §0.2.2; the mutation red-proof — a
/// TaskGroup briefly added, the pin red, reverted — is in the evidence.)
#[test]
fn recognition_is_sequential_in_the_sidecar_source() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shell-macos/Sources/shell-vision/main.swift"
    ))
    .expect("the sidecar source is in this tree");
    for construct in [
        "TaskGroup",
        "withTaskGroup",
        "concurrentPerform",
        "DispatchQueue.global",
        "dispatchApply",
    ] {
        assert!(
            !source.contains(construct),
            "the sidecar source carries {construct} — parallel recognition is \
             the measured hang hazard (the plan's §0.2.2); sequential is the law"
        );
    }
    // The recognition request exists (the pin must not pass vacuously) —
    // and BOTH call sites ride the one construction: the page loop AND the
    // capability probe (ocr_assets_unavailable's measured stimulus), each
    // awaiting its perform sequentially.
    assert!(
        source.contains("makeRecognizeTextRequest()"),
        "the one request construction exists (the pin must not pass vacuously)"
    );
    assert_eq!(
        source.matches(".perform(on:").count(),
        2,
        "exactly two recognition sites: the sequential page loop and the \
         capability probe — a third is a found bug"
    );
}

// ── the macOS-gated presence path ───────────────────────────────────────────

/// The dev-tree sidecar probe (the pin's own stand-in for the config
/// resolution): the SwiftPM build output, debug or release. Deliberately
/// LOCAL — the pins compile against the pre-pin tree; the config fn that
/// the route really consults resolves the same paths (the resolve_child_exe
/// idiom).
fn dev_sidecar() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    [
        manifest
            .join("shell-macos/.build")
            .join(cfg)
            .join("shell-vision"),
        manifest
            .join("shell-macos/.build/arm64-apple-macosx")
            .join(cfg)
            .join("shell-vision"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// The FULL path through the REAL sidecar: on a macOS tree that has built
/// the Swift package, the route runs the REAL Vision API end to end. On
/// Linux (no sidecar can exist) the resolution lands on None and the
/// absence-path pins above are the tested green path — the recorded dual,
/// not a silent skip: the evidence records which half ran where.
#[tokio::test]
async fn real_sidecar_full_path_when_present() {
    let s = server();
    // No env override: the config's own resolution is the presence path.
    let _env = absent_env();
    let body = serde_json::json!({
        "data": b64(&fixture("ocr-real/clean-scan.pdf")),
        "title": "OV presence: the real scan",
        "tier": "ocr",
    })
    .to_string();
    let (status, resp) = post_document(&s.state, body).await;
    if dev_sidecar().is_none() {
        // The Linux half of the dual: no sidecar anywhere on this tree →
        // the named refusal IS the green outcome here.
        assert_eq!(
            code_of(&resp),
            "ocr_unavailable",
            "no sidecar anywhere on this tree → the named refusal: {resp}"
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        return;
    }
    assert_eq!(status, StatusCode::OK, "the REAL pipeline: {resp}");
    assert_eq!(resp["tier"], "ocr", "every page of the scan is OCR");
    let markdown = resp["markdown"].as_str().expect("markdown");
    assert!(
        markdown.contains("On-Device Transcription"),
        "the REAL Vision output entered the assembled markdown"
    );
    assert_eq!(
        header_field(markdown, "pages-tier").as_deref(),
        Some("ocr"),
        "the map marks the whole scan ocr"
    );
    let engine = header_field(markdown, "ocr-engine").expect("the engine stamp");
    assert!(
        engine.starts_with("apple-vision/recognize-text@"),
        "the REAL engine string, got: {engine}"
    );
}

// ── the duplicate path for an ocr row ───────────────────────────────────────

/// The duplicate re-serve re-derives through the SAME pipeline (the child +
/// the stub again) and verifies the header against the row — including the
/// pinned engine. An engine that MOVED under the stored row (the OS-upgrade
/// case the stub simulates) refuses NAMED, the same law as an extractor
/// version bump.
#[tokio::test]
async fn the_duplicate_path_re_derives_an_ocr_row_and_pins_the_engine() {
    let s = server();
    let body = serde_json::json!({
        "data": b64(&fixture("hybrid.pdf")),
        "title": "OV duplicate",
        "tier": "ocr",
    })
    .to_string();

    // First ingest: the row lands with the stub's engine stamp.
    {
        let (_env, _record) = stub_env(&hybrid_stub_reply());
        let (status, resp) = post_document(&s.state, body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{resp}");
        assert_eq!(resp["status"], "created");
        assert_eq!(resp["tier"], "mixed");
    }

    // Re-serve under the SAME engine: duplicate, the map re-derived.
    {
        let (_env, _record) = stub_env(&hybrid_stub_reply());
        let (status, resp) = post_document(&s.state, body.clone()).await;
        assert_eq!(status, StatusCode::OK, "{resp}");
        assert_eq!(resp["status"], "duplicate", "the same bytes are a re-serve");
        assert_eq!(resp["tier"], "mixed");
        let markdown = resp["markdown"].as_str().expect("re-derived markdown");
        assert_eq!(
            header_field(markdown, "ocr-engine").as_deref(),
            Some("apple-vision/recognize-text@STUBBUILD"),
            "the re-derivation ran under the row's engine"
        );
    }

    // Re-serve under a MOVED engine (the simulated OS upgrade): the pinned
    // provenance refuses NAMED — the stored row's engine did not run.
    {
        let moved = hybrid_stub_reply().replace("STUBBUILD", "MOVEDBUILD");
        let (_env, _record) = stub_env(&moved);
        let (status, resp) = post_document(&s.state, body).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "the engine moved under the row: {resp}"
        );
        assert_eq!(
            code_of(&resp),
            "document_row_mismatch",
            "the DI-3 law extends to the engine pin"
        );
    }
}

// ── the DI-3 extension, pinned directly ─────────────────────────────────────

/// The row check fails a tampered map / a moved engine BY NAME (OV-2's
/// checker leg, the improve04 DI-3 pattern extended to the new fields).
/// Mutation red-proof: with the map check removed from
/// `verify_document_header` (measured 2026-11-08), the tampered markdown
/// verifies and this pin reds `expected Err, got Ok`; reverted, green.
#[test]
fn di3_row_check_fails_a_tampered_map_or_engine_by_name() {
    use brain_server::ingest_document as ig;
    let pages = ig::ExtractedDocument {
        pages: vec!["typed text".into(), String::new(), "more typed".into()],
    };
    let assembly = ig::OcrAssembly {
        engine: "apple-vision/recognize-text@BUILD".into(),
        pages: vec![ig::OcrPageMark {
            index: 2,
            confidence: Some(0.8),
        }],
    };
    let md = ig::emit_annotated_markdown(
        &pages,
        "cafe05",
        "upload",
        ig::TIER_MIXED,
        "2026-11-08T00:00:00+00:00",
        ig::LAYOUT_UNVERIFIED,
        &ig::DocumentProvenance::Ocr(assembly),
    );
    let row = ig::DocumentRow {
        id: 1,
        sha256: "cafe05".into(),
        title: "t".into(),
        tier: ig::TIER_MIXED.into(),
        extractor: ig::EXTRACTOR_NAME.into(),
        extractor_version: ig::EXTRACTOR_VERSION.into(),
        pages: 3,
        layout_unverified: true,
        source: "upload".into(),
        domain: "global".into(),
        owner: None,
        extracted_utc: "2026-11-08T00:00:00+00:00".into(),
        markdown_sha256: String::new(),
        page_tiers: Some(r#"["text-layer","ocr","text-layer"]"#.into()),
        low_confidence_pages: 0,
        ocr_engine: Some("apple-vision/recognize-text@BUILD".into()),
    };
    ig::verify_document_header(&md, &row).expect("an honest ocr header verifies");

    // the map, tampered one page at a time, fails naming the field
    for tampered_map in [
        r#"["ocr","ocr","text-layer"]"#,
        r#"["text-layer","text-layer","text-layer"]"#,
        r#"["text-layer","ocr"]"#,
    ] {
        let tampered = md.replacen(
            "pages-tier: text-layer,ocr,text-layer",
            &format!(
                "pages-tier: {}",
                tampered_map.trim_matches(|c| c == '[' || c == ']')
            ),
            1,
        );
        let err = ig::verify_document_header(&tampered, &row)
            .expect_err(&format!("tampered map {tampered_map} must fail"));
        assert!(
            err.starts_with("pages-tier"),
            "the failure names the field: {err}"
        );
    }
    // a header that DROPS the map (the Tier-0 shape) fails an ocr row
    let dropped = md.split(" | pages-tier: ").next().unwrap().to_string()
        + " -->\n"
        + md.split("-->\n").nth(1).unwrap();
    let err = ig::verify_document_header(&dropped, &row).expect_err("dropped map");
    assert!(err.starts_with("pages-tier"), "{err}");

    // the engine, moved, fails naming the field
    let moved = md.replacen("@BUILD", "@MOVED", 1);
    let err = ig::verify_document_header(&moved, &row).expect_err("moved engine");
    assert!(err.starts_with("ocr-engine"), "{err}");
}
