//! THE CG-10 E4 LIVE ACCEPTANCE: the chat spine serves the MLX tier end to
//! end — through the REAL kernel HTTP surface, the REAL bridge child, and
//! the REAL Qwen3-8B weights. Three arms, each a named acceptance from the
//! lane doc:
//!
//! 1. **The Vitamin D sequence** — a grounded turn, then a follow-up that
//!    resolves through the session topic (the CG-08 shape) — both answering
//!    `tier: "mlx_native"`, the second citing the SAME evidence the kernel
//!    recalled.
//! 2. **The green-tea control** — zero evidence in the store, so the
//!    kernel's own deterministic abstain gate answers WITHOUT the bridge:
//!    the MLX tier must not bypass the abstain law (the final's tier is
//!    "deterministic", never a model's).
//! 3. **The bridge-kill recovery** — a transcript-only fact, the child
//!    killed mid-session, and the follow-up answering IN-CONTEXT through
//!    the respawn + reseed on the MLX path. This is the end-to-end C-9
//!    witness the lane inherited from CG-09: the `history` field crosses
//!    the wire into a ChatSession this request creates, and only the seed
//!    can answer.
//!
//! Tier posture: every arm sets `BRAIN_CHAT_MODEL=qwen3-8b-4bit` under the
//! shared lock (tests/common::chat_tier) — the AFM-tier pins in
//! `chat_seeding.rs` / `chat_followup_grounding.rs` hold the same lock
//! pinned to None, so the env window is exactly one test body.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

#[allow(dead_code)] // the stripper is inert here; the tier seam is what we use
mod common;

const OP_TOKEN: &str = "chat-mlx-operator-token";
const MLX_TIER: &str = "qwen3-8b-4bit";

/// One SSE line of interest: `(event, data)` pairs in arrival order — the
/// same reader `chat_seeding.rs` drives, so both files read the wire the
/// same way.
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

fn vitamin_d_memory() -> String {
    [
        "Vitamin D3 (cholecalciferol) is produced in the skin when UVB light",
        "converts 7-dehydrocholesterol. It aids calcium absorption in the gut,",
        "supports bone mineralization, and modulates immune function. Adults",
        "commonly take 1000-2000 IU daily with a meal containing fat, because",
        "the vitamin is fat-soluble and uptake improves with dietary lipids.",
    ]
    .join(" ")
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

async fn mlx_app_with_memory(
    memory: Option<String>,
) -> (Arc<brain_server::AppState>, tempfile::TempDir) {
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
    if let Some(memory) = memory {
        let ingest = Request::builder()
            .method("POST")
            .uri("/ingest/memory")
            .header("content-type", "text/plain")
            .header("authorization", format!("Bearer {OP_TOKEN}"))
            .body(Body::from(memory))
            .expect("ingest request");
        let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
        assert_eq!(resp.status(), StatusCode::OK, "the memory must ingest");
    }
    (state, dir)
}

/// ARM 1 — the Vitamin D sequence on the MLX tier: a grounded turn, then a
/// follow-up that the CG-08 session-topic shape carries, both through the
/// ChatSession path (`tier: "mlx_native"`).
#[tokio::test]
// The tier lock's guard INTENTIONALLY spans the body's awaits: it serializes
// the process-global BRAIN_CHAT_MODEL window (tests/common — the
// scheduling-flake guard); a single-test mutex, no deadlock shape.
#[allow(clippy::await_holding_lock)]
async fn the_vitamin_d_sequence_answers_on_the_mlx_tier() {
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the MLX-tier acceptance is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    let (_tier_lock, _tier_env) = common::chat_tier::set_chat_model(Some(MLX_TIER));

    let (state, _dir) = mlx_app_with_memory(Some(vitamin_d_memory())).await;

    // Turn 1: the grounded question.
    let turn1 = post_turn(
        state.clone(),
        "mlx-d",
        "What should I know about vitamin D?",
    )
    .await;
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("mlx_native"),
        "turn 1 must answer on the MLX tier: {final1}"
    );
    let answer1 = final1["text"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        answer1.contains("vitamin") || answer1.contains("d3") || answer1.contains("calcium"),
        "turn 1 answers from the recalled evidence: {answer1}"
    );

    // Turn 2: the CG-08 follow-up — no topic term of its own beyond "it".
    let turn2 = post_turn(
        state.clone(),
        "mlx-d",
        "Why is it best to take it with food?",
    )
    .await;
    let final2 = final_of(&turn2);
    assert_eq!(
        final2["tier"].as_str(),
        Some("mlx_native"),
        "turn 2 must answer on the MLX tier too (the session carries across turns): {final2}"
    );
    let answer2 = final2["text"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        answer2.contains("fat") || answer2.contains("absorb"),
        "turn 2 answers IN-CONTEXT (the fat-soluble fact lives only in memory): {answer2}"
    );
}

/// ARM 2 — the abstain-gate control, via the REACHABLE arm: on an empty
/// corpus the calibrated estimator classifies empty results as
/// IncreaseTopK, never ClarifyQuery (recorded in the CG-08 lane evidence),
/// so the deterministic abstain fires through `degraded` — a follow-up
/// whose message plus the persisted prior topic exceeds the recall query
/// bound (2,000 chars) fails grounding closed with zero hits. The kernel
/// then answers WITHOUT the bridge: the MLX tier must not bypass the
/// abstain law — the model that cannot cite does not get the turn at all.
/// (A turn whose decision is `ok` with zero hits DELIBERATELY still
/// reaches the model — the store may genuinely hold nothing and the model
/// may say so; on the MLX tier that case is the tier's named boundary: no
/// guided abstention schema exists there yet, so an
/// unanswerable-from-evidence question records the tier's honest answer in
/// the evidence file rather than a pin.)
#[tokio::test]
#[allow(clippy::await_holding_lock)] // the tier lock spans the body by law (see arm 1)
async fn the_zero_evidence_control_abstains_deterministically_without_the_bridge() {
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the MLX-tier acceptance is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    let (_tier_lock, _tier_env) = common::chat_tier::set_chat_model(Some(MLX_TIER));

    // An EMPTY corpus. Turn 1 answers through the bridge (honest ok) and
    // persists the prior turn; turn 2's message plus the prior topic
    // exceeds the recall query bound → degraded + zero hits.
    let (state, _dir) = mlx_app_with_memory(None).await;
    let turn1 = post_turn(
        state.clone(),
        "mlx-control",
        "what is the capital of the moon?",
    )
    .await;
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("mlx_native"),
        "turn 1 answers through the bridge (empty store, honest ok): {final1}"
    );

    let long_followup = format!("please find out {}", "a".repeat(1975));
    let turn2 = post_turn(state, "mlx-control", &long_followup).await;
    let abstain = final_of(&turn2);
    assert_eq!(
        abstain["tier"].as_str(),
        Some("deterministic"),
        "a degraded zero-hit turn abstains through the KERNEL's own gate — no model tier answers: {abstain}"
    );
    let text = abstain["text"].as_str().unwrap_or_default();
    assert!(
        !text.to_lowercase().contains("paris"),
        "the deterministic abstain never names an answer: {text}"
    );
}

/// ARM 3 — the bridge-kill recovery on the MLX tier, the end-to-end C-9
/// witness: the transcript-only fact survives a child kill, because the
/// respawned session is seeded from the persisted transcript and the
/// `history` field hydrates the fresh ChatSession.
#[tokio::test]
#[allow(clippy::await_holding_lock)] // the tier lock spans the body by law (see arm 1)
async fn killing_the_bridge_mid_chat_loses_no_in_context_answer_on_the_mlx_tier() {
    let Some(bridge_binary) = common::bridge_binary() else {
        eprintln!(
            "brain-afm not built (run `swift build` in shell-macos); the MLX-tier acceptance is owned by the macOS lane — SKIPPING"
        );
        return;
    };
    unsafe { std::env::set_var("BRAIN_AFM_BRIDGE", bridge_binary) };
    let (_tier_lock, _tier_env) = common::chat_tier::set_chat_model(Some(MLX_TIER));

    // One UNRELATED memory, so each turn's grounding carries fenced
    // evidence (the AFM T-2 harness shape — the instructions read best
    // with a fence present). The discriminator is unchanged: the MORNING
    // fact exists ONLY in the transcript, never in any memory.
    let (state, _dir) = mlx_app_with_memory(Some(
        [
            "Green tea is a tea made from Camellia sinensis leaves that are",
            "steamed and dried without oxidation. It is rich in catechins,",
            "especially EGCG, and carries moderate caffeine plus L-theanine.",
        ]
        .join(" "),
    ))
    .await;

    // Turn 1: a transcript-only fact (never in any memory).
    let turn1 = post_turn(
        state.clone(),
        "mlx-kill",
        "Please remember this: my doctor said to take my vitamin D drops in the morning.",
    )
    .await;
    let final1 = final_of(&turn1);
    assert_eq!(
        final1["tier"].as_str(),
        Some("mlx_native"),
        "turn 1 answers on the MLX tier: {final1}"
    );

    // Kill the child mid-session.
    let killed = std::process::Command::new("pkill")
        .args(["-f", "brain-afm"])
        .status()
        .expect("pkill runs");
    assert!(killed.success(), "the bridge process was killed");
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Turn 2: only the RESEEDED transcript can answer this.
    let turn2 = post_turn(
        state,
        "mlx-kill",
        "When did my doctor say to take the drops?",
    )
    .await;
    let final2 = final_of(&turn2);
    assert_eq!(
        final2["tier"].as_str(),
        Some("mlx_native"),
        "turn 2 answers through the respawned bridge on the MLX tier (no 503, no refusal): {final2}"
    );
    let answer = final2["text"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        answer.contains("morning"),
        "turn 2 answers IN-CONTEXT from the seeded history (the morning fact was never in any memory): answer={answer} final={final2}"
    );
}
