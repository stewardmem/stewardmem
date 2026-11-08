// IMPROVE06 S2 — the DC pins, red-first: each written against the pre-pin
// tree, each reds BY NAME, then green.
//
// ## The red ledger (measured 2026-11-08, against the S1 tree at ded0ae95)
//
// Stashing the route/emitter changes and running this file against the S1
// tree: **12 red, 4 green** (16 tests). The reds, by name:
//
// - DC-1 behavioral (both arms) red: `expected 503 converter_remote_refused,
//   got 501 converter_not_implemented` — the exit-5 posture did not exist.
// - DC-1 structural red — a genuine catch: the S1 wrapper's own COMMENT
//   contained the literal `convert-remote`, tripping the byte-literal pin.
//   The plan says the string appears NOWHERE in the source; the comment
//   violated the letter. The source was reworded (the pin stayed literal).
// - DC-2 behavioral red: the 501 boundary never spawned the stub, so no odd
//   byte could survive.
// - DC-5 red (all three arms): the marker cross-check did not exist.
// - DC-6 behavioral red (both arms): no row was ever written, so no
//   extractor stamp from meta.json existed, and no re-derive could refuse.
// - exit-4/exit-3 red: the mapping arms did not exist.
// - DC-3 independence red (the docling side): the ocr arm ran the dev-tree
//   hybrid instead of refusing — the test now forces the bridge absent
//   (the measured flake, the missing_env shape).
//
// Green-on-arrival (with the mutation red-proofs in the evidence):
// - DC-3's absence arm — the improve04 seam already named
//   converter_unavailable for the unconfigured posture.
// - DC-4 + DC-6 structural — the S1 sources/docs were written to the law;
//   their red proofs are BY MUTATION.
// - the golden round-trip pin — tool-gated, green on the recording machine.
//
// The stub tool: a POSIX-sh script the test writes into a temp dir — it
// speaks docs/DOCLING_TOOL_PROTOCOL.md minimally and deterministically on
// any OS: the remote refusal (exit 5), the two output files, the exit-code
// vocabulary. The kernel cannot tell it from the real converters; that is
// the point (the improve05 recording-stub pattern).

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
const OP_TOKEN: &str = "improve06-docling-op-token";

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

/// The same harness shape as `tests/ocr_sidecar_pins.rs`: an opaque-mode
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

/// A wire-level header-field reader (the ocr_sidecar_pins shape — LOCAL, the
/// contract's union-across-lines parse, not the kernel's parser).
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

// ── the BRAIN_DOCLING_PATH env seam (the ocr file's lock shape) ─────────────
// The probe is read PER REQUEST; cargo runs a crate's tests concurrently —
// a docling test that pointed the path at a stub while another asserted the
// absence posture would be a scheduling flake, not a pin. Every docling test
// holds this lock for its whole body; the guard restores ALL the keys on
// drop.

fn docling_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Holds the lock + restores every env key the stub pins touch on drop.
struct DoclingEnv {
    _guard: MutexGuard<'static, ()>,
    saved: Vec<(&'static str, Option<String>)>,
    /// The stub's home: held alive for the run's whole body (dropping it
    /// deletes the stub before the route probes it — the measured hazard).
    _dir: Option<tempfile::TempDir>,
}

const STUB_KEYS: &[&str] = &[
    "BRAIN_DOCLING_PATH",
    "DC_STUB_RECORD",
    "DC_STUB_EXIT",
    "DC_STUB_TOOL",
    "DC_STUB_VERSION",
    "DC_STUB_PAGES",
    "DC_STUB_MARKERS",
    "DC_STUB_NO_DOC",
    "DC_STUB_NO_META",
    "DOCLING_SERVICE_URL",
    "DOCLING_SERVICE_API_KEY",
    "DOCLING_RS_VLM_ENDPOINT",
    "BRAIN_VISION_BRIDGE",
];

impl Drop for DoclingEnv {
    fn drop(&mut self) {
        for (key, saved) in &self.saved {
            match saved {
                Some(v) => unsafe { std::env::set_var(key, v) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

fn env_guard(dir: Option<tempfile::TempDir>) -> DoclingEnv {
    let guard = docling_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    DoclingEnv {
        saved: STUB_KEYS
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect(),
        _guard: guard,
        _dir: dir,
    }
}

/// Point BRAIN_DOCLING_PATH at a contract stub tool; return its home.
fn stub_env() -> (DoclingEnv, PathBuf) {
    let dir = tempfile::TempDir::new().expect("stub dir");
    let stub = write_stub(dir.path());
    let env = env_guard(Some(dir));
    unsafe { std::env::set_var("BRAIN_DOCLING_PATH", stub.to_str().expect("utf8 stub")) };
    (env, dir_join_stub(&stub))
}

fn dir_join_stub(stub: &std::path::Path) -> PathBuf {
    stub.to_path_buf()
}

/// Acquire the lock with the converter path ABSENT — the deterministic
/// absence posture on every tree.
fn absent_env() -> DoclingEnv {
    let env = env_guard(None);
    unsafe { std::env::remove_var("BRAIN_DOCLING_PATH") };
    env
}

// ── the stub tool ────────────────────────────────────────────────────────────

/// Write a one-shot stub converter: honors the protocol's exit vocabulary,
/// writes the two output files, and takes its behavior from DC_STUB_* env
/// knobs so each pin shapes its own scenario deterministically.
fn write_stub(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("docling-stub");
    let script = r#"#!/bin/sh
# the DC-pin stub tool (IMPROVE06 S2): docs/DOCLING_TOOL_PROTOCOL.md,
# minimally, deterministically, on any OS.
in=$1
out=$2
# the DC-1 refusal shape: a remote/generative posture in the environment
# is refused named, exit 5, BEFORE any work.
refused=""
[ -n "${DOCLING_SERVICE_URL:-}" ] && refused="$refused,DOCLING_SERVICE_URL"
[ -n "${DOCLING_SERVICE_API_KEY:-}" ] && refused="$refused,DOCLING_SERVICE_API_KEY"
[ -n "${DOCLING_RS_VLM_ENDPOINT:-}" ] && refused="$refused,DOCLING_RS_VLM_ENDPOINT"
if [ -n "$refused" ]; then
  echo "stub: remote configuration refused (${refused#,} set)" >&2
  exit 5
fi
[ -n "${DC_STUB_RECORD:-}" ] && printf '%s' "$in" > "$DC_STUB_RECORD"
[ -f "$in" ] || { echo "stub: input is not a file" >&2; exit 3; }
mkdir -p "$out" || exit 3
# DC-5 scenario knobs: a missing file or a marker/page disagreement.
if [ -z "${DC_STUB_NO_DOC:-}" ]; then
  markers=$(printf '%s' "${DC_STUB_MARKERS:-1}")
  body='stub page one \002\003 cafe - *literal* stays'
  i=0
  : > "$out/doc.md"
  while [ "$i" -le "$markers" ]; do
    if [ "$i" -gt 0 ]; then printf '<!-- page-break -->\n\n' >> "$out/doc.md"; fi
    printf '%b page %s\n' "$body" "$((i + 1))" >> "$out/doc.md"
    i=$((i + 1))
  done
fi
if [ -z "${DC_STUB_NO_META:-}" ]; then
  printf '{"tool":"%s","version":"%s","pages":%s}\n' \
    "${DC_STUB_TOOL:-docling-stub}" "${DC_STUB_VERSION:-1.0.0-stub}" \
    "${DC_STUB_PAGES:-2}" > "$out/meta.json"
fi
exit "${DC_STUB_EXIT:-0}"
"#;
    std::fs::write(&path, script).expect("stub written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("stub executable");
    }
    path
}

// ── DC-3 — the absence posture (the Linux-green path) ────────────────────────

/// The converter path is ABSENT: the docling tier refuses NAMED 503
/// `converter_unavailable`, naming the fix. Tier 0 and the ocr arm are
/// unaffected — the three probes are independent (OV-4's shape, the
/// docling sibling).
#[tokio::test]
async fn dc3_docling_tier_refuses_converter_unavailable_when_absent() {
    let s = server();
    let _env = absent_env();
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("two-column.pdf")),
            "title": "DC-3",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "expected 503 converter_unavailable, got {status} {resp}"
    );
    assert_eq!(
        code_of(&resp),
        "converter_unavailable",
        "the Tier-2 seam's own name — NOT the ocr arm's ocr_unavailable"
    );
}

/// The three probes are independent, the docling side: a configured
/// converter does not fix the ocr arm, and Tier 0 runs with both unset.
#[tokio::test]
async fn dc3_the_probes_are_independent_docling_side() {
    let s = server();
    let _env = stub_env();
    // the ocr bridge is forced to a missing path — the deterministic absence
    // posture (the dev-tree resolution would otherwise find a built
    // shell-vision and RUN the hybrid instead of refusing; the measured
    // flake the ocr_sidecar_pins solved with missing_env).
    unsafe { std::env::set_var("BRAIN_VISION_BRIDGE", "/nonexistent/shell-vision") };
    // the ocr arm's refusal names ITS seam, with the converter configured:
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("hybrid.pdf")),
            "title": "DC-3 ocr independence",
            "tier": "ocr",
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        code_of(&resp),
        "ocr_unavailable",
        "the converter stub configures nothing for the ocr arm: {status} {resp}"
    );
    // Tier 0 needs neither configuration:
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("single-column.pdf")),
            "title": "DC-3 tier0 independence",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Tier 0 unaffected: {resp}");
}

// ── DC-1 — the remote/generative refusal ────────────────────────────────────

/// `DOCLING_SERVICE_URL` set → the tool exits 5 → the route answers
/// `converter_remote_refused` (503). Stub-testable on any OS.
#[tokio::test]
async fn dc1_remote_service_url_is_refused_named() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DOCLING_SERVICE_URL", "http://remote.example") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-1",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{resp}");
    assert_eq!(code_of(&resp), "converter_remote_refused", "{resp}");
}

/// The API-key half of the same posture (the wrapper refuses either).
#[tokio::test]
async fn dc1_remote_api_key_is_refused_named() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DOCLING_SERVICE_API_KEY", "secret") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-1 key",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{resp}");
    assert_eq!(code_of(&resp), "converter_remote_refused", "{resp}");
}

// ── DC-2 — the tool's bytes, verbatim ────────────────────────────────────────

/// A stub emitting ODD bytes (C0 controls, diacritics, markdown specials)
/// proves the assembled markdown carries the tool's output VERBATIM: no
/// rewrite, no smoothing, no escaping pass exists (DC-2). The kernel's only
/// transformation is the page split.
#[tokio::test]
async fn dc2_the_tools_odd_bytes_survive_verbatim() {
    let s = server();
    let (_env, _stub) = stub_env();
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-2",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "the stub run creates: {resp}");
    let markdown = resp["markdown"].as_str().expect("the response markdown");
    // the stub's page-one body, byte for byte (the \u{2}\u{3} controls and
    // the markdown specials ride through untouched):
    let odd = "stub page one \u{2}\u{3} cafe - *literal* stays page 1";
    assert!(
        markdown.contains(odd),
        "DC-2 red: the tool's bytes were rewritten somewhere — the block must carry \
         them verbatim; got: {markdown}"
    );
    assert_eq!(resp["tier"], "docling");
}

// ── DC-5 — the marker/page agreement ────────────────────────────────────────

/// The stub declares 3 pages but writes 1 marker (2 blocks) → the route
/// refuses BY NAME: the mismatch is in the message, the document lands no
/// row.
#[tokio::test]
async fn dc5_marker_page_mismatch_refuses_by_name() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_PAGES", "3") }; // 1 marker → 2 blocks
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-5 mismatch",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(code_of(&resp), "converter_failed", "{resp}");
    let msg = resp["error"]["message"].as_str().unwrap_or_default();
    assert!(
        msg.contains("2 blocks for 3 declared pages"),
        "the refusal names the mismatch: {msg}"
    );
}

/// A missing doc.md is a contract violation named (never a half-ingest).
#[tokio::test]
async fn dc5_missing_doc_md_refuses_named() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_NO_DOC", "1") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-5 no doc",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(code_of(&resp), "converter_failed", "{resp}");
    let msg = resp["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains("no doc.md"), "named: {msg}");
}

/// A missing meta.json is a contract violation named.
#[tokio::test]
async fn dc5_missing_meta_json_refuses_named() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_NO_META", "1") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-5 no meta",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(code_of(&resp), "converter_failed", "{resp}");
    let msg = resp["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains("no meta.json"), "named: {msg}");
}

// ── DC-6 — the version is recorded per run; a moved version refuses ─────────

/// The stub's meta version lands in the row's extractor columns and the
/// header's extractor field — the four-way agreement (meta == header ==
/// row == response) the Tier-2 tier inherits from DI-3.
#[tokio::test]
async fn dc6_the_tool_version_is_recorded_per_run() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_TOOL", "docling-stub") };
    unsafe { std::env::set_var("DC_STUB_VERSION", "9.9.9-pin") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-6",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["extractor"], "docling-stub/9.9.9-pin", "{resp}");
    let markdown = resp["markdown"].as_str().expect("markdown");
    assert_eq!(
        header_field(markdown, "extractor").as_deref(),
        Some("docling-stub/9.9.9-pin"),
        "the header stamps the tool's own identity"
    );
    assert_eq!(
        header_field(markdown, "pages-tier").as_deref(),
        Some("docling,docling"),
        "the Tier-2 map is all-docling (the whole-document read)"
    );
    assert_eq!(
        header_field(markdown, "ocr-engine"),
        None,
        "no ocr-engine field exists on a docling header"
    );
    assert!(markdown.contains("<!-- anchor: p1 -->"));
    assert!(markdown.contains("<!-- anchor: p2 -->"));
    assert_eq!(
        resp["page_tiers"],
        serde_json::json!(["docling", "docling"])
    );
    assert_eq!(resp["low_confidence_pages"], 0);
    assert_eq!(resp["ocr_engine"], serde_json::Value::Null);
}

/// A re-serve under a MOVED tool version refuses `document_row_mismatch`
/// (the improve05 engine-pin law, extractor-shaped).
#[tokio::test]
async fn dc6_a_moved_version_refuses_the_rederive() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_VERSION", "1.0.0-pinned") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-6 move",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create under v1: {resp}");
    let doc_sha = resp["sha256"].as_str().expect("sha").to_string();
    // the tool MOVES under the stored document:
    unsafe { std::env::set_var("DC_STUB_VERSION", "2.0.0-moved") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "DC-6 move (re-serve)",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the moved version refuses the re-derive: {resp}"
    );
    assert_eq!(code_of(&resp), "document_row_mismatch", "{resp}");
    let _ = doc_sha;
}

// ── the exit-4 arm — a self-timed tool is converter_timeout ─────────────────

/// The protocol's reserved exit 4 (a tool that self-times and refuses to
/// emit a partial document) maps to `converter_timeout` (422) — the same
/// answer the kernel's own kill at CONVERTER_TIMEOUT_SECS produces.
#[tokio::test]
async fn exit4_self_timed_tool_maps_to_converter_timeout() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_EXIT", "4") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "exit 4",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(code_of(&resp), "converter_timeout", "{resp}");
}

/// Exit 3 (the bytes' fault) carries the bounded stderr tail in the MESSAGE
/// and lands no row.
#[tokio::test]
async fn exit3_failure_carries_the_stderr_tail_named() {
    let s = server();
    let (_env, _stub) = stub_env();
    unsafe { std::env::set_var("DC_STUB_EXIT", "3") };
    let (status, resp) = post_document(
        &s.state,
        serde_json::json!({
            "data": b64(&fixture("table.pdf")),
            "title": "exit 3",
            "tier": "docling",
        })
        .to_string(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{resp}");
    assert_eq!(code_of(&resp), "converter_failed", "{resp}");
    let msg = resp["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains("the conversion failed"), "named: {msg}");
}

// ── the structural pins (read the committed sources) ────────────────────────

/// DC-1 structural: neither implementation carries a remote client, and the
/// Python CLI's API subcommand (`convert-remote`) appears nowhere.
#[test]
fn dc1_structural_no_remote_client_in_either_implementation() {
    let root = env!("CARGO_MANIFEST_DIR");
    let py = std::fs::read_to_string(format!("{root}/scripts/docling-kernel.py"))
        .expect("the committed wrapper");
    let sh = std::fs::read_to_string(format!("{root}/scripts/docling-rs-kernel.sh"))
        .expect("the committed shim");
    for (name, source) in [("docling-kernel.py", &py), ("docling-rs-kernel.sh", &sh)] {
        assert!(
            !source.contains("convert-remote"),
            "DC-1: {name} must never touch the API subcommand"
        );
        assert!(
            !source.contains("http://") && !source.contains("https://"),
            "DC-1: {name} carries no endpoint URL"
        );
        assert!(
            source.contains("DOCLING_SERVICE_URL") && source.contains("DOCLING_SERVICE_API_KEY"),
            "DC-1: {name} refuses the remote-services env posture by name"
        );
    }
    assert!(
        sh.contains("DOCLING_RS_VLM_ENDPOINT"),
        "DC-1: the shim refuses docling.rs's remote-LLM surface by name"
    );
    assert!(
        sh.contains("--no-url-fetch"),
        "DC-1: the shim refuses URL sources outright"
    );
}

/// DC-6 structural: the deterministic standard pipeline is pinned by
/// omission in BOTH implementations — no VLM import in the wrapper's
/// import surface, no pipeline/VLM flag on the shim's invocation.
#[test]
fn dc6_structural_the_deterministic_pipeline_is_pinned_by_omission() {
    let root = env!("CARGO_MANIFEST_DIR");
    let py = std::fs::read_to_string(format!("{root}/scripts/docling-kernel.py"))
        .expect("the committed wrapper");
    let sh = std::fs::read_to_string(format!("{root}/scripts/docling-rs-kernel.sh"))
        .expect("the committed shim");
    // the wrapper's docling import surface is a WHITELIST: every docling
    // import is one of the deterministic-pipeline types. A VLM import (the
    // mutation proof) fails this pin.
    let allowed = [
        "from docling.datamodel.base_models import",
        "from docling.datamodel.pipeline_options import PdfPipelineOptions",
        "from docling.document_converter import",
        "from docling_core.types.doc import ImageRefMode",
        "import docling",
    ];
    // The wrapper's docling import surface is a WHITELIST over STRIPPED
    // lines: the imports live inside main() (indented), and the first draft
    // of this pin filtered on column-0 `starts_with` only — the mutation
    // proof (an injected VlmPipeline import) sailed through a VACUOUS pin.
    // Stripped, every docling import line must be one of the allowed set;
    // any other docling import (a VLM/generative surface) is a found bug.
    let mut docling_imports = 0;
    for line in py.lines() {
        let t = line.trim();
        if t.starts_with('#') || !t.starts_with("from docling") && !t.starts_with("import docling")
        {
            continue;
        }
        docling_imports += 1;
        assert!(
            allowed.iter().any(|a| t.starts_with(a)),
            "DC-6: the wrapper imports {t:?} — outside the deterministic-pipeline \
             whitelist (a generative/VLM import is a found bug)"
        );
    }
    assert!(
        docling_imports >= 4,
        "DC-6: the whitelist saw {docling_imports} docling imports — the pin must be \
         reading the real import surface, not passing vacuously"
    );
    // the shim's invocation line carries no pipeline/VLM flag at all:
    for line in sh.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            continue;
        }
        assert!(
            !t.contains("--pipeline") && !t.contains("--vlm") && !t.contains("vlm_endpoint"),
            "DC-6: the shim's execution surface carries a generative flag: {line:?}"
        );
    }
}

/// DC-4 structural: the license-refusal record exists in the protocol doc
/// and cites the CURRENT terms by name.
#[test]
fn dc4_the_license_refusal_record_cites_the_current_terms() {
    let root = env!("CARGO_MANIFEST_DIR");
    let doc = std::fs::read_to_string(format!("{root}/docs/DOCLING_TOOL_PROTOCOL.md"))
        .expect("the protocol doc");
    for term in [
        "Marker",
        "AI PUBS Open RAIL-M",
        "MinerU",
        "100 M MAU",
        "MIT",
        "Apache-2.0",
        "docling.rs",
        "olmOCR",
    ] {
        assert!(
            doc.contains(term),
            "DC-4: the refusal record must cite {term:?} (the current terms, verified \
             2026-11-08)"
        );
    }
    assert!(
        doc.contains("SANCTIONED"),
        "DC-4: the record names the sanctioned posture, not just the refusals"
    );
}

// ── the real-tool goldens (tool-gated — the honest Linux/macOS dual) ────────

/// Where the operator toolchain exists (the venv / the binary + models),
/// the committed goldens round-trip BYTE-IDENTICALLY on the recorded tool
/// versions. Where it does not, the pin records the split honestly and
/// passes — the absence path is the CI-green path everywhere (the
/// improve05 dual).
#[test]
fn docling_goldens_round_trip_on_the_recorded_versions() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join("tests/fixtures/documents");
    let two_col = fixtures.join("two-column.pdf");

    // candidate A: the venv + the wrapper + the artifacts
    let venv_python = root.join(".venv-docling/bin/python");
    let artifacts = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".cache/docling-artifacts"))
        .ok()
        .filter(|p| p.is_dir());
    if venv_python.is_file()
        && let Some(artifacts) = artifacts
    {
        let work = tempfile::TempDir::new().expect("temp");
        let out = work.path().join("out");
        let mut cmd = std::process::Command::new(&venv_python);
        cmd.arg(root.join("scripts/docling-kernel.py"))
            .arg(&two_col)
            .arg(&out)
            .env("BRAIN_DOCLING_ARTIFACTS", &artifacts);
        let st = cmd.status().expect("spawn the wrapper");
        assert!(st.success(), "candidate A's golden probe failed");
        for f in ["doc.md", "meta.json"] {
            let fresh = std::fs::read(out.join(f)).expect("the wrapper's output");
            let golden =
                std::fs::read(fixtures.join(format!("docling-goldens/docling-py-2.148.0/{f}")))
                    .expect("the committed golden");
            assert_eq!(
                fresh, golden,
                "candidate A's {f} moved on the recorded version — re-run the probe and \
                 re-commit under a new version-named directory"
            );
        }
    } else {
        println!(
            "docling goldens: candidate A's toolchain is absent — the split is recorded, the pin passes"
        );
    }

    // candidate B: the binary + its models, wherever it is on PATH
    let probe = std::process::Command::new("docling-rs")
        .arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .status();
    if matches!(&probe, Ok(st) if st.success()) {
        let work = tempfile::TempDir::new().expect("temp");
        let out = work.path().join("out");
        let shim = root.join("scripts/docling-rs-kernel.sh");
        let st = std::process::Command::new(&shim)
            .arg(&two_col)
            .arg(&out)
            .status()
            .expect("spawn the shim");
        assert!(st.success(), "candidate B's golden probe failed");
        for f in ["doc.md", "meta.json"] {
            let fresh = std::fs::read(out.join(f)).expect("the shim's output");
            let golden =
                std::fs::read(fixtures.join(format!("docling-goldens/docling-rs-2.67.1/{f}")))
                    .expect("the committed golden");
            assert_eq!(
                fresh, golden,
                "candidate B's {f} moved on the recorded version — re-run the probe and \
                 re-commit under a new version-named directory"
            );
        }
    } else {
        println!(
            "docling goldens: candidate B's toolchain is absent — the split is recorded, the pin passes"
        );
    }
}

/// The Tier-2 budget topology (the r47 line-order shape): the document route
/// merges AFTER the shared request timeout in mod.rs — the converter tier's
/// named 300 s bound is reachable, never pre-empted by the shared 30 s wall
/// (the measured 2026-11-08 e2e finding: a real 6-page clinical-guideline
/// conversion took 29.5 s warm — inside the wall by 0.45 s — and answered a
/// generic 408 before the named refusal could travel).
#[test]
fn the_document_route_merges_outside_the_shared_request_timeout() {
    let mod_rs = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/mod.rs"
    ));
    let shared_timeout = mod_rs
        .find(".layer(TimeoutLayer::with_status_code(")
        .expect("the shared TimeoutLayer call exists");
    let document_merge = mod_rs
        .find(".merge(memory::document_ingest_router())")
        .expect("the document router merge exists");
    let catch_panic = mod_rs
        .find(".layer(CatchPanicLayer::new())")
        .expect("CatchPanic");
    assert!(
        document_merge > shared_timeout,
        "the document route must merge AFTER the shared timeout (outside the 30 s wall)"
    );
    assert!(
        document_merge < catch_panic,
        "the document route must merge BEFORE CatchPanic/RBAC/auth — the later layers \
         still cover it (the routes-registered-so-far rule)"
    );
    let memory_rs = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/memory.rs"
    ));
    assert!(
        memory_rs.contains("config::DOCUMENT_INGEST_TIMEOUT_SECS"),
        "the route's own budget is the named constant, not an inline literal"
    );
    assert!(
        !memory_rs
            .split("pub fn router()")
            .next()
            .expect("router() exists")
            .contains("ingest/document"),
        "the shared router no longer registers /ingest/document (it would sit inside \
         the 30 s wall again)"
    );
}
