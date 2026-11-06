//! The CG-09 S2 seeding pin (T-2): kill brain-afm mid-chat and the next
//! turn still answers IN-CONTEXT — because the kernel re-seeds the fresh
//! bridge session from the persisted transcript. Without seeding, the
//! follow-up after a kill has no memory of the session and cannot recall
//! what was never in the knowledge store.
//!
//! The discriminator is a fact that exists ONLY in the transcript (the
//! user's morning routine), never in any seeded memory: turn 1 states it,
//! the bridge is killed, turn 2 asks "When do I take it?" — only a seeded
//! session can answer "morning". The respawn half (a dead singleton must
//! not 503 the next turn) rides the same test.
//!
//! Platform honesty: same skip as the other bridge tests — where
//! `brain-afm` is not built, the test SKIPS with a stated reason.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const OP_TOKEN: &str = "chat-seeding-operator-token";

fn green_tea_memory() -> String {
    [
        "Green tea is a tea made from Camellia sinensis leaves that are steamed",
        "and dried without oxidation. It is rich in catechins, especially EGCG,",
        "which are natural antioxidants, and contains moderate caffeine plus",
        "L-theanine, linked to calm focused alertness.",
    ]
    .join(" ")
}

/// One SSE line of interest: `(event, data)` pairs in arrival order.
fn sse_events(body: &str) -> Vec<(String, serde_json::Value)> {
    let mut out = Vec::new();
    let mut event = String::new();
    for line in body.lines() {
        if let Some(name) = line.strip_prefix("event:") {
            event = name.trim().to_owned();
        } else if let Some(data) = line.strip_prefix("data:") {
            let parsed = serde_json::from_str(data.trim())
                .unwrap_or(serde_json::Value::String(data.trim().to_owned()));
            out.push((event.clone(), parsed));
        }
    }
    out
}

async fn post_turn(
    state: Arc<brain_server::AppState>,
    session: &str,
    message: &str,
) -> Vec<(String, serde_json::Value)> {
    let turn = Request::builder()
        .method("POST")
        .uri("/chat/turns")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(format!(
            r#"{{"session":{session:?},"message":{message:?}}}"#
        )))
        .expect("chat request");
    let resp = app(state).oneshot(turn).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK, "the chat turn must start");
    let bytes = axum::body::to_bytes(resp.into_body(), 8 * 1024 * 1024)
        .await
        .expect("sse body");
    sse_events(&String::from_utf8_lossy(&bytes))
}

fn final_of(events: &[(String, serde_json::Value)]) -> serde_json::Value {
    events
        .iter()
        .rev()
        .find(|(name, _)| name == "final")
        .map(|(_, data)| data.clone())
        .unwrap_or(serde_json::Value::Null)
}

#[allow(dead_code)] // the stripper is inert here; the tier seam is what we use
mod common;

#[tokio::test]
// The tier lock's guard INTENTIONALLY spans the body's awaits: it
// serializes the process-global BRAIN_CHAT_MODEL window (tests/common);
// a single-test mutex, no deadlock shape.
#[allow(clippy::await_holding_lock)]
async fn killing_the_bridge_mid_chat_loses_no_in_context_answer() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("shell-macos/.build/out/Products/Debug/brain-afm"),
        manifest.join("shell-macos/.build/arm64-apple-darwin/debug/brain-afm"),
    ];
    let Some(bridge_binary) = candidates.iter().find(|p| p.is_file()) else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the seeding pin is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    // THE TIER POSTURE, held for the whole body: this pin asserts
    // `on_device` finals, so the chat tier is pinned to the AFM default
    // under the shared lock (an MLX-tier test setting BRAIN_CHAT_MODEL
    // concurrently would be a scheduling flake, not a red).
    let (_tier_lock, _tier_env) = common::chat_tier::set_chat_model(None);

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
        cors: tower_http::cors::CorsLayer::new(),
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

    // One memory, so grounding has something to do each turn.
    let ingest = Request::builder()
        .method("POST")
        .uri("/ingest/memory")
        .header("content-type", "text/plain")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(green_tea_memory()))
        .expect("ingest request");
    let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK, "the memory must ingest");

    // ── turn 1: a transcript-only fact (never in any memory) ──
    let turn1 = post_turn(
        state.clone(),
        "seed",
        "I take my vitamin D in the morning with breakfast.",
    )
    .await;
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("on_device"),
        "turn 1 answers through the bridge: {final1}"
    );

    // ── kill brain-afm mid-chat ──
    let killed = std::process::Command::new("pkill")
        .args(["-f", "brain-afm"])
        .status()
        .expect("pkill runs");
    assert!(killed.success(), "the bridge process was killed");
    // The dead child's reader task notices asynchronously; give it a beat.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ── turn 2: only the seeded transcript can answer this ──
    let turn2 = post_turn(state.clone(), "seed", "When do I take it?").await;
    let final2 = final_of(&turn2);
    assert_eq!(
        final2["tier"].as_str(),
        Some("on_device"),
        "turn 2 answers through the respawned bridge (no 503, no refusal): {final2}"
    );
    let answer = final2["text"].as_str().unwrap_or_default();
    assert!(
        answer.to_lowercase().contains("morning"),
        "turn 2 answers IN-CONTEXT from the seeded history (the morning routine was never in memory): {answer}"
    );
}
