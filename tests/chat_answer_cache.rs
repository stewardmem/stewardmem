//! CG-11 E3 LIVE WITNESS: the greedy answer cache answers a repeated
//! `(model, prompt)` with ZERO decode. Two FRESH sessions over the SAME
//! corpus posting the SAME message are the same greedy computation by law
//! — the first decodes (real tokens), the second is served (0/0 token
//! counts on the wire, byte-identical text, the producing tier preserved),
//! and the cache does not grow on the hit. The SSE shape stays exactly the
//! spine's (grounding → final → support): a cached turn is a turn.
//!
//! Tier posture: `BRAIN_CHAT_MODEL=qwen3-8b-4bit` under the shared lock.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

#[allow(dead_code)] // the stripper is inert here; the tier seam is what we use
mod common;

const OP_TOKEN: &str = "chat-cache-operator-token";
const MLX_TIER: &str = "qwen3-8b-4bit";

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

fn final_of(events: &[(String, serde_json::Value)]) -> serde_json::Value {
    events
        .iter()
        .rev()
        .find(|(name, _)| name == "final")
        .map(|(_, data)| data.clone())
        .unwrap_or(serde_json::Value::Null)
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

async fn cache_app_with_memory(memory: &str) -> (Arc<brain_server::AppState>, tempfile::TempDir) {
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
        chat_answer_cache: Arc::new(
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
    let ingest = Request::builder()
        .method("POST")
        .uri("/ingest/memory")
        .header("content-type", "text/plain")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(memory.to_owned()))
        .expect("ingest request");
    let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK, "the memory must ingest");
    (state, dir)
}

/// THE ZERO-DECODE WITNESS: the first turn decodes, the identical second
/// turn (a fresh session, same corpus, same message — the same (model,
/// prompt) computation) is served from the cache with 0/0 token counts and
/// the byte-identical answer, tier preserved, cache not grown.
#[tokio::test]
#[allow(clippy::await_holding_lock)] // the tier lock spans the body by law (chat_mlx_tier arm 1)
async fn the_identical_repeat_turn_is_served_with_zero_decode() {
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the cache witness is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    let (_tier_lock, _tier_env) = common::chat_tier::set_chat_model(Some(MLX_TIER));

    let (state, _dir) = cache_app_with_memory(
        "Vitamin D3 (cholecalciferol) is produced in the skin when UVB light \
         converts 7-dehydrocholesterol. It aids calcium absorption in the gut \
         and supports bone mineralization.",
    )
    .await;
    assert!(
        state.chat_answer_cache.is_empty(),
        "the witness starts from an empty cache"
    );

    let message = "What should I know about vitamin D?";

    // Turn 1, session A: a real decode.
    let turn1 = post_turn(state.clone(), "cache-a", message).await;
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("mlx_native"),
        "turn 1 answers on the MLX tier: {final1}"
    );
    let decoded_out = final1["outputTokens"].as_i64().unwrap_or_default();
    assert!(
        decoded_out > 0,
        "the first turn REALLY decoded (outputTokens {decoded_out})"
    );
    let answer = final1["text"].as_str().unwrap_or_default().to_owned();
    assert!(!answer.is_empty(), "the decoded answer is non-empty");
    assert_eq!(state.chat_answer_cache.len(), 1, "the miss stored one");

    // Turn 2, session B: the SAME (model, prompt) — served, not decoded.
    let turn2 = post_turn(state.clone(), "cache-b", message).await;
    let final2 = final_of(&turn2);
    assert_eq!(
        final2["tier"].as_str(),
        Some("mlx_native"),
        "the cached final keeps the producing tier (the answer's provenance): {final2}"
    );
    assert_eq!(
        final2["text"].as_str(),
        Some(answer.as_str()),
        "the cached answer is byte-identical to the decoded one"
    );
    assert_eq!(
        final2["inputTokens"].as_i64(),
        Some(0),
        "a served turn prefills nothing"
    );
    assert_eq!(
        final2["outputTokens"].as_i64(),
        Some(0),
        "a served turn decodes nothing — the zero-decode witness"
    );
    assert_eq!(
        state.chat_answer_cache.len(),
        1,
        "the hit never re-stores — the cache did not grow"
    );

    // The support event still trails the cached final (the wire contract).
    assert!(
        turn2.iter().any(|(name, _)| name == "support"),
        "the cached turn carries the support verdict like any final"
    );

    // A DIFFERENT prompt is a MISS (a different message decodes again).
    let turn3 = post_turn(state.clone(), "cache-b", "Why is vitamin D fat-soluble?").await;
    let final3 = final_of(&turn3);
    assert_eq!(
        final3["tier"].as_str(),
        Some("mlx_native"),
        "a different message still decodes on the tier: {final3}"
    );
    assert!(
        final3["outputTokens"].as_i64().unwrap_or_default() > 0,
        "a different prompt is not served from the cache"
    );
    assert_eq!(state.chat_answer_cache.len(), 2, "the new prompt stored");
}
