// R147 — the CORS'd refusals: every refusal the kernel raises must be
// READABLE by the browser that asked.
//
// ## Why this file exists
//
// The R146 drill measured the failure on the wire: `GET /stats` with a
// stale bearer returned a 401 with NO `access-control-allow-origin`,
// because `auth_middleware` sat OUTSIDE the CORS layer and answered from a
// seat the response never passed back through — the webview CORS-blocked
// every real 401 before any shell code saw it, and the heal's onResponse
// choke point never fired. A refusal the browser cannot read is a refusal
// that never happened. R147 moved CORS to the outermost seat of the
// stack (`src/server/router/mod.rs` `app()`), and these pins hold it
// there — driven through the REAL `app(state)` with the REAL boot-built
// CORS policy (`cors_layer_from_config`, the ONE implementation — boot
// and pins share it), not a test-local copy that could drift.
//
// The layer order is load-bearing and easy to get exactly wrong in both
// directions: CORS too far IN re-creates the unreadable refusal; CORS
// granting foreign origins would be a policy hole. Pin 1 holds the first
// edge (the refusal carries the grant for an allowed origin), pin 2 the
// second (a foreign origin gets no grant even at the refusal layer), and
// pin 3 the preflight posture (answered by CorsLayer BEFORE auth — no
// auth-denied audit row exists for a preflight that never reached it).

use std::sync::{Arc, Once};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::bootstrap::cors_layer_from_config;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use rusqlite::Connection;
use tower::ServiceExt;

/// The opaque superuser token the fixture store accepts. The pins present
/// a DIFFERENT bearer (`STALE`) so the refusal path is the one under test.
const OP_TOKEN: &str = "r147-cors-wire-op-token";

/// The stale bearer the kernel must refuse — garbage by construction.
const STALE: &str = "stale-probe-token";

/// The shell's origin (the tauri webview seat) — allowed by the operator's
/// live plist posture, and the origin the R146 drill measured the block on.
const SHELL_ORIGIN: &str = "tauri://localhost";

/// A foreign origin the predicate must never grant anything to.
const EVIL_ORIGIN: &str = "https://evil.example";

static CORS_ENV: Once = Once::new();

/// Install the pin's origin set BEFORE the first `cors_layer_from_config()`
/// call — the fn reads the env at call time, and the default (env unset)
/// is loopback-only, which would make the tauri assertion vacuous. The set
/// mirrors the operator's live plist posture (measured 2026-11-01). Each
/// integration-test file is its own process, so this cannot leak into
/// other pins; the Once keeps the set_var single-shot under parallel tests.
fn install_cors_env() {
    CORS_ENV.call_once(|| unsafe {
        std::env::set_var(
            "CORS_ORIGINS",
            "tauri://localhost,http://localhost:3000,http://localhost:8080",
        );
    });
}

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

/// An opaque-mode server on a real file-backed, real migrated database, with
/// the REAL boot-built CORS layer (`cors_layer_from_config`) — the same
/// harness shape as `tests/trace_citation_pins.rs`; nothing under test is a
/// stand-in. A test state injecting a bare `CorsLayer::new()` here would
/// make every pin in this file vacuous.
fn server() -> Server {
    install_cors_env();
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
        cors: cors_layer_from_config(),
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

async fn send(
    state: &Arc<brain_server::AppState>,
    req: Request<Body>,
) -> axum::http::Response<Body> {
    app(state.clone()).oneshot(req).await.expect("infallible")
}

/// True when `name`'s value list (comma-split, case-insensitive) contains
/// `want` — Vary can join several values on one header line.
fn vary_lists(res: &axum::http::Response<Body>, want: &str) -> bool {
    res.headers()
        .get_all("vary")
        .iter()
        .flat_map(|v| v.to_str().unwrap_or("").split(','))
        .any(|v| v.trim().eq_ignore_ascii_case(want))
}

/// THE RED-PROOF PIN. With CORS back in its pre-R147 inner seat, the 401
/// below loses its ACAO and this pin fails — that failure IS the recorded
/// red (the R146 drill's wire measurement, reproduced in-process).
#[tokio::test]
async fn an_auth_refusal_carries_the_cors_headers() {
    let s = server();
    let req = Request::builder()
        .method("GET")
        .uri("/stats")
        .header("origin", SHELL_ORIGIN)
        .header("authorization", format!("Bearer {STALE}"))
        .body(Body::empty())
        .expect("request");
    let res = send(&s.state, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "garbage bearer");
    let acao = res
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().expect("ascii").to_string());
    assert_eq!(
        acao.as_deref(),
        Some(SHELL_ORIGIN),
        "the refusal must carry the CORS grant for the shell's origin — \
         a refusal the browser cannot read is a refusal that never happened"
    );
    assert!(
        vary_lists(&res, "origin"),
        "a dynamic (predicate) origin policy must mark Vary: origin"
    );
}

/// The predicate holds at the refusal layer too: a foreign origin gets no
/// grant on ANY response, refusal or success — the browser remains the
/// enforcer, and nothing in the stack grants it.
#[tokio::test]
async fn a_disallowed_origin_gets_no_cors_grant() {
    let s = server();
    let req = Request::builder()
        .method("GET")
        .uri("/stats")
        .header("origin", EVIL_ORIGIN)
        .header("authorization", format!("Bearer {STALE}"))
        .body(Body::empty())
        .expect("request");
    let res = send(&s.state, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "garbage bearer");
    assert!(
        res.headers().get("access-control-allow-origin").is_none(),
        "a foreign origin must not receive the grant at the refusal layer"
    );
}

/// The preflight posture: OPTIONS is answered by CorsLayer BEFORE the auth
/// layers — 2xx with the grant, and NO auth-denied audit row, because the
/// preflight never reached the auth middleware (preflights carry no
/// credentials by spec).
#[tokio::test]
async fn a_preflight_is_answered_before_auth() {
    let s = server();
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/stats")
        .header("origin", SHELL_ORIGIN)
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .expect("request");
    let res = send(&s.state, req).await;
    assert!(
        res.status().is_success(),
        "the preflight is answered by the CORS layer itself: {:?}",
        res.status()
    );
    let acao = res
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().expect("ascii").to_string());
    assert_eq!(acao.as_deref(), Some(SHELL_ORIGIN));
    let conn = Connection::open(&s.state.db_path).expect("db");
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .expect("audit count");
    assert_eq!(
        rows, 0,
        "a preflight must leave no auth-denied audit row — it never reached auth"
    );
}
