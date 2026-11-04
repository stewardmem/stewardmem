//! The CG-08 follow-up grounding pin — the Vitamin D sequence's defect,
//! reproduced behaviorally.
//!
//! The live defect (operator, 2026-11-04, live kernel): grounding reads ONLY
//! the current message (`handlers/chat.rs` built the recall query from
//! `message` alone), so a follow-up with no topic term of its own —
//! "What are the benefits?" after "what is Vitamin D" — retrieves nothing
//! usable and abstains, despite the session holding the topic and prior
//! turns being persisted but never read at grounding time.
//!
//! Why this fixture discriminates (measured, not assumed): retrieval is
//! top-k nearest with NO score cutoff, so on a two-memory fixture the
//! follow-up alone always returns SOMETHING — the live zero-hit abstain is a
//! populated-corpus phenomenon, not reproducible in a unit fixture. What IS
//! reproducible is the query-construction defect itself: the follow-up
//! alone ranks whichever memory the corpus generically prefers, indifferent
//! to the session's topic. Measured with this fixture (probe, 2026-11-04):
//! "What are the benefits?" alone → Vitamin D 0.342 / green tea 0.189; with
//! the green-tea prior turn prepended → green tea 0.563 / Vitamin D 0.171.
//! So the pin seeds BOTH memories, opens the session on green tea, and
//! asserts the follow-up grounds to the PRIOR turn's topic (the green-tea
//! memory that turn 1 itself grounded to) — pre-fix it grounds to Vitamin
//! D instead, which is the RED.
//!
//! This drives the REAL composed app — `POST /ingest/memory` →
//! `POST /chat/turns` (SSE) — with the real router, the real deterministic
//! recall core, and the REAL `brain-afm` bridge. Nothing is mocked.
//!
//! Platform honesty: same skip as `chat_grounding_flow` — where `brain-afm`
//! is not built, the test SKIPS with a stated reason.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const OP_TOKEN: &str = "chat-followup-operator-token";

fn vitamin_d_memory() -> String {
    [
        "Vitamin D is a fat-soluble vitamin essential for calcium absorption,",
        "bone health, and immune function. It is produced in skin exposed to",
        "sunlight and obtained from fatty fish and fortified foods. Regular",
        "adequate intake strengthens bones, supports immune response, and",
        "reduces the risk of deficiency-related illness.",
    ]
    .join(" ")
}

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

fn grounding_of(events: &[(String, serde_json::Value)]) -> serde_json::Value {
    events
        .iter()
        .find(|(name, _)| name == "grounding")
        .map(|(_, data)| data.clone())
        .unwrap_or(serde_json::Value::Null)
}

fn final_of(events: &[(String, serde_json::Value)]) -> serde_json::Value {
    events
        .iter()
        .rev()
        .find(|(name, _)| name == "final")
        .map(|(_, data)| data.clone())
        .unwrap_or(serde_json::Value::Null)
}

/// The grounding hits' ids, in wire order; `None` if the event is absent.
fn hit_ids(grounding: &serde_json::Value) -> Option<Vec<i64>> {
    grounding["hits"]
        .as_array()
        .map(|hits| hits.iter().filter_map(|h| h["id"].as_i64()).collect())
}

#[tokio::test]
async fn the_followup_grounds_to_the_prior_turns_topic_not_the_corpus_generic() {
    // The bridge binary is a macOS artifact; when it is not built this test
    // owns its skip with a stated reason (the macOS lane runs it for real).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("shell-macos/.build/out/Products/Debug/brain-afm"),
        manifest.join("shell-macos/.build/arm64-apple-darwin/debug/brain-afm"),
    ];
    let Some(bridge_binary) = candidates.iter().find(|p| p.is_file()) else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the follow-up grounding pin is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };

    // ── the server over its own database (the loopback fixture shape) ──
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

    // ── seed BOTH memories through the public route ──
    for memory in [vitamin_d_memory(), green_tea_memory()] {
        let ingest = Request::builder()
            .method("POST")
            .uri("/ingest/memory")
            .header("content-type", "text/plain")
            .header("authorization", format!("Bearer {OP_TOKEN}"))
            .body(Body::from(memory))
            .expect("ingest request");
        let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "the memory must ingest (check the injection screen)"
        );
    }

    // ── turn 1: the topic-setting question names the session's topic ──
    let turn1 = post_turn(state.clone(), "followup", "what is green tea?").await;
    let grounding1 = grounding_of(&turn1);
    let hits1 = hit_ids(&grounding1).expect("turn 1 grounding hits");
    assert!(
        !hits1.is_empty(),
        "turn 1 grounds (the topic question retrieves): {grounding1}"
    );
    let prior_topic_id = hits1[0];
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("on_device"),
        "turn 1 answers from the on-device model, not the abstain path: {final1}"
    );

    // ── turn 2: the follow-up carries NO topic term of its own ──
    // Alone, "What are the benefits?" ranks Vitamin D first (measured
    // 0.342 vs green tea 0.189) — indifferent to this session's green-tea
    // topic. The session's prior turn must supply the topic: turn 2's top
    // grounding hit must be the SAME memory turn 1 grounded to.
    let turn2 = post_turn(state.clone(), "followup", "What are the benefits?").await;
    let grounding2 = grounding_of(&turn2);
    let hits2 = hit_ids(&grounding2).expect("turn 2 grounding hits");
    let final2 = final_of(&turn2);
    assert!(!hits2.is_empty(), "turn 2 grounds: {grounding2}");
    assert_eq!(
        hits2[0], prior_topic_id,
        "turn 2 (a follow-up with no topic term) must ground to the prior turn's topic \
         (memory {prior_topic_id}, the hit turn 1 grounded to) — grounding the corpus \
         generic instead is the CG-08 defect. turn-2 grounding: {grounding2}; final: {final2}"
    );
    assert_eq!(
        final2["tier"].as_str(),
        Some("on_device"),
        "turn 2 must answer from the model over retrieved evidence, not the deterministic abstain: {final2}"
    );
    assert!(
        !turn2.iter().any(|(name, _)| name == "error"),
        "no error event rode the turn-2 stream: {turn2:?}"
    );
}
