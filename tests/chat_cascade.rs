//! The cascade-into-chat wiring pins (round 18) — the composed-server lane.
//!
//! BRAIN_CHAT_CASCADE arms the cascade routing inside `POST /chat/turns`:
//! the turn's own grounding feeds the coordinator, the typed route drives
//! the execution, and every claim's cites are KERNEL-DERIVED by span match
//! (judge_answer) — the model never reports indices on this path. The pins:
//!
//! 1. **the OFF law** (the default): the gate unset/unarmed → the existing
//!    flow serves the turn (tier `on_device`, the AFM contract) —
//!    byte-identical to the pre-round behavior; and the gate's truth table
//!    (the BIND_PUBLIC armed-values precedent).
//! 2. **the invariant** (the live-traffic finding's class): on CASCADE
//!    turns, `support.unknown_refs` is ZERO — a derived cite indexes the
//!    real universe, so the judge never reads an unknown ref. The 1-based
//!    cite class cannot enter.
//! 3. **the tier stamps**: the evidence-as-answer route stamps
//!    `deterministic` (the existing abstain-path vocabulary), the writer
//!    route stamps `mlx_native` (the bridge's own tier).
//! 4. **the persist law**: the cascade turn persists through the SAME
//!    service tx (the session loop's input, whatever tier served it).
//!
//! The live lane: skips by name when the brain-afm child is absent (the
//! `chat_mlx_tier` convention — the acceptance is owned by the macOS lane).

mod common;

use axum::body::Body;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use std::sync::{Mutex, MutexGuard};
use tower::ServiceExt;

const OP_TOKEN: &str = "r18-cascade-token";
const GREEN_TEA: &str = "Green tea is made from Camellia sinensis leaves that are steamed soon after picking, which preserves catechins like EGCG. Its benefits include antioxidant support, improved metabolic markers, and a milder caffeine lift than coffee.";

/// The gate-window guard: the chat tests run in parallel in one process, so
/// every env mutation serializes behind ONE mutex (the `chat_tier` guard's
/// shape, keyed on this round's own env).
static CASCADE_GATE_LOCK: Mutex<()> = Mutex::new(());

struct CascadeGateEnv;

impl CascadeGateEnv {
    fn set(armed: bool) -> (MutexGuard<'static, ()>, Self) {
        let guard = CASCADE_GATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            if armed {
                std::env::set_var("BRAIN_CHAT_CASCADE", "1");
            } else {
                std::env::remove_var("BRAIN_CHAT_CASCADE");
            }
        }
        (guard, Self)
    }
}

impl Drop for CascadeGateEnv {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("BRAIN_CHAT_CASCADE");
        }
    }
}

#[test]
fn cascade_gate_truth_table_is_the_armed_values_law() {
    // The BIND_PUBLIC precedent: only the explicit armed values count; a
    // set-but-unrecognised value is NOT armed (fail-closed).
    unsafe {
        std::env::remove_var("BRAIN_CHAT_CASCADE");
        assert!(!brain_server::config::chat_cascade_enabled(), "unset = OFF");
        std::env::set_var("BRAIN_CHAT_CASCADE", "0");
        assert!(!brain_server::config::chat_cascade_enabled(), "0 = OFF");
        std::env::set_var("BRAIN_CHAT_CASCADE", "yes");
        assert!(
            !brain_server::config::chat_cascade_enabled(),
            "a set-but-unrecognised value is NOT armed"
        );
        std::env::set_var("BRAIN_CHAT_CASCADE", "1");
        assert!(brain_server::config::chat_cascade_enabled(), "1 = ON");
        std::env::set_var("BRAIN_CHAT_CASCADE", "true");
        assert!(brain_server::config::chat_cascade_enabled(), "true = ON");
        std::env::remove_var("BRAIN_CHAT_CASCADE");
    }
}

async fn cascade_app_with_memory(
    memory: &str,
) -> (std::sync::Arc<brain_server::AppState>, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mgr = SqliteConnectionManager::file(&db_path);
    let pool: brain_server::Pool = r2d2::Pool::builder().max_size(4).build(mgr).expect("pool");
    brain_server::migration::run_migration(
        &mut pool.get().expect("conn"),
        brain_server::config::DB_MMAP_SIZE_MIB,
    )
    .expect("migration");
    let model: std::sync::Arc<dyn brain_server::embed::Embedder> = std::sync::Arc::new(
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model"),
    );
    let tok_file = dir.path().join("tokens");
    std::fs::write(&tok_file, format!("{OP_TOKEN}\n")).expect("token file");
    let token_store = brain_server::auth::TokenStore::from_file(Some(tok_file));
    token_store.reload_parts_from(vec![OP_TOKEN.to_string()], None);
    let state = std::sync::Arc::new(brain_server::AppState {
        token_store,
        jwt_middleware_state: std::sync::Arc::new(JwtMiddlewareState::opaque_for_tests(
            pool.clone(),
            db_path.clone(),
        )),
        cors: tower_http::cors::CorsLayer::new(),
        chat_answer_cache: std::sync::Arc::new(
            brain_server::service::chat_answer_cache::ChatAnswerCache::new(),
        ),
        durability: Default::default(),
        loom: Default::default(),
        model,
        registry: brain_server::domain_registry::DomainRegistry::new(pool.clone(), &db_path, false),
        pool: pool.clone(),
        db_path: db_path.clone(),
        connection_tracker: std::sync::Arc::new(brain_server::http_limit::ConnectionTracker::new()),
        rate_limiter: std::sync::Arc::new(brain_server::http_limit::RateLimiter::new()),
        snapshot: brain_server::integrity::SnapshotState::default(),
        audit_chain_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
        auth_mode: brain_server::auth::AuthMode::Opaque,
        key_store: brain_server::auth::jwks::KeyStore::default(),
        revocation_cache: std::sync::Arc::new(
            brain_server::auth::revocation::RevocationCache::new(),
        ),
        jwt_issuer: String::new(),
        jwt_audience: String::new(),
        oidc_config: brain_server::handlers::well_known::OidcConfig::unconfigured(),
        ump_events: tokio::sync::broadcast::channel(brain_server::config::UMP_EVENT_BUFFER).0,
        alert_events: tokio::sync::broadcast::channel(brain_server::config::ALERT_EVENT_BUFFER).0,
        alert_seq: std::sync::atomic::AtomicU64::new(0),
        chain_watch: brain_server::alert::ChainWatchState::default(),
        concurrency: &brain_server::concurrency::CONCURRENCY,
    });
    let ingest = axum::http::Request::builder()
        .method("POST")
        .uri("/ingest/memory")
        .header("content-type", "text/plain")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(memory.to_string()))
        .expect("ingest request");
    let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::OK,
        "the memory must ingest"
    );
    (state, dir)
}

async fn post_turn(
    state: std::sync::Arc<brain_server::AppState>,
    session: &str,
    message: &str,
) -> Vec<(String, serde_json::Value)> {
    let turn = axum::http::Request::builder()
        .method("POST")
        .uri("/chat/turns")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(format!(
            r#"{{"session":{session:?},"message":{message:?}}}"#
        )))
        .expect("chat request");
    let resp = app(state).oneshot(turn).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::OK,
        "the turn must start"
    );
    let bytes = axum::body::to_bytes(resp.into_body(), 8 * 1024 * 1024)
        .await
        .expect("sse body")
        .to_vec();
    let mut events = Vec::new();
    let mut current = String::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            current = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data: ")
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(rest)
        {
            events.push((current.clone(), v));
        }
    }
    events
}

fn frame_of<'a>(
    events: &'a [(String, serde_json::Value)],
    name: &str,
) -> Option<&'a serde_json::Value> {
    events.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v)
}

#[tokio::test(flavor = "multi_thread")]
// The gate guard INTENTIONALLY spans the body's awaits: it serializes the
// process-global BRAIN_CHAT_CASCADE window (the chat_tier guard's shape) —
// a single-test mutex, no deadlock shape.
#[allow(clippy::await_holding_lock)]
async fn cascade_off_serves_the_existing_flow() {
    let (_lock, _env) = CascadeGateEnv::set(false);
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the \
             OFF-law acceptance is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    let (state, _dir) = cascade_app_with_memory(GREEN_TEA).await;
    let events = post_turn(
        state.clone(),
        "off-law",
        "What are the benefits of green tea?",
    )
    .await;
    let final_frame = frame_of(&events, "final").expect("a final frame");
    // The OFF law: the existing flow serves the turn — the AFM tier, the
    // model's own claims — NEVER the cascade's `deterministic` stamp.
    assert_eq!(
        final_frame["tier"].as_str(),
        Some("on_device"),
        "the gate is OFF: the existing flow serves the turn, not the cascade"
    );
    unsafe { std::env::remove_var("BRAIN_AFM_BRIDGE") };
}

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn cascade_on_turns_never_produce_unknown_refs() {
    let _lock_owner = CASCADE_GATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the \
             cascade wiring acceptance is owned by the macOS lane — SKIPPING"
        );
        unsafe { std::env::remove_var("BRAIN_CHAT_CASCADE") };
        return;
    };
    unsafe {
        std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary);
        std::env::set_var("BRAIN_CHAT_CASCADE", "1");
    }
    let (state, _dir) = cascade_app_with_memory(GREEN_TEA).await;
    let events = post_turn(
        state.clone(),
        "cascade-on",
        "What are the benefits of green tea?",
    )
    .await;
    let final_frame = frame_of(&events, "final").expect("a final frame");
    let support = frame_of(&events, "support").expect("a support frame");
    // THE INVARIANT (the live-traffic finding's class): the derived cites
    // index the real universe — the judge NEVER reads an unknown ref.
    assert_eq!(
        support["unknown_refs"].as_i64(),
        Some(0),
        "cascade turns carry kernel-DERIVED cites: unknown_refs must be 0 \
         (the finding's class cannot enter) — support={support}"
    );
    // The tier stamp: whichever route served, the stamp is one of the
    // cascade's two vocabulary values — never the model's own tier choice.
    let tier = final_frame["tier"].as_str().unwrap_or_default();
    assert!(
        tier == "deterministic" || tier == "mlx_native",
        "the cascade turn stamps its own tier vocabulary, got {tier:?}"
    );
    // The claims ride the final (the kernel-derived shape: text + cites).
    assert!(
        final_frame["claims"].as_array().is_some(),
        "the final carries the derived claims"
    );
    // THE PERSIST LAW: the turn pair landed (the session loop's input).
    // The persist rides a spawned task — poll briefly rather than race it.
    let mut count: i64 = -1;
    for _ in 0..40 {
        count = {
            let conn = state.pool.get().expect("conn");
            conn.query_row(
                "SELECT COUNT(*) FROM chat_turns t JOIN chat_sessions s ON s.id = t.session_id \
                 WHERE s.session_key = 'cascade-on'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(-1)
        };
        if count == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(count, 2, "the user+assistant pair persists (2 rows)");
    unsafe {
        std::env::remove_var("BRAIN_CHAT_CASCADE");
        std::env::remove_var("BRAIN_AFM_BRIDGE");
    }
}
