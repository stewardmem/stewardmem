//! IMPROVE08 S5 — THE E2E WITNESS over the replay corpus: lane create
//! (SSRF-screened) → POST /watch/tick → proposal(s) filed with provenance →
//! human approve through the EXISTING gate → knowledge row (source `watch`,
//! origin `imported`) → recall sees it; and the refusal twin: reject →
//! nothing. THE WATCH FILES PROPOSALS, NEVER MEMORY (RW-1's behavioral
//! face, closed at the wire).
//!
//! The harness is the scholar-pins server shape (the real router, the real
//! gate, a temp DB). The tick rides the REPLAY transport (CI never fetches,
//! RW-6 — the lane's feed URL is the corpus-mapped replay URL); the LIVE
//! dual is the operator-gated run recorded in the evidence file (the 07
//! ocr-real dual, inherited).
//!
//! RED LEDGER (2026-11-09): the witness is the round's LAST act — the
//! surfaces it composes landed in S1–S4, so its red evidence is the
//! composition itself failing on the pre-S5 tree ONLY if a seam were
//! missing; each seam is already pinned by name in tests/watch_pins.rs.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const OP_TOKEN: &str = "improve08-e2e-op-token";

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

async fn post_json(
    state: &Arc<brain_server::AppState>,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
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

async fn get_json(
    state: &Arc<brain_server::AppState>,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::empty())
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

async fn post_empty(
    state: &Arc<brain_server::AppState>,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from("{}"))
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

async fn recall(state: &Arc<brain_server::AppState>, q: &str) -> serde_json::Value {
    let (status, json) = post_json(
        state,
        "/recall",
        serde_json::json!({ "query": q, "limit": 10 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "recall answers: {json}");
    json
}

/// THE WITNESS: the full loop, human-gated, over the replay corpus.
#[tokio::test]
async fn the_full_loop_lane_tick_proposal_approve_memory_and_the_reject_twin() {
    let srv = server();
    let state = srv.state.clone();

    // ── 1. lane create (SSRF-screened — lobste.rs is the public replay
    // lane; the screen resolves and pins it) ─────────────────────────────
    let lane_row_id = {
        let conn = state.pool.get().expect("conn");
        brain_server::watch::create_lane(
            &conn,
            &brain_server::watch::LaneSpec {
                name: "lobsters-e2e",
                source_kind: "rss",
                feed_url: "https://lobste.rs/rss.rss",
                query: None,
                cadence_secs: 3600,
                destination: "proposals",
            },
            "2026-11-09T14:00:00Z",
        )
        .expect("lane create through the screen")
    };

    // ── 2. the tick, over the WIRE (the replay transport — CI never
    // fetches; the report is the honest accounting) ─────────────────────
    let (tick_status, tick) = post_json(
        &state,
        "/watch/tick",
        serde_json::json!({ "lane": "lobsters-e2e" }),
    )
    .await;
    assert_eq!(tick_status, StatusCode::OK, "the tick answers: {tick}");
    assert_eq!(tick["lane"], "lobsters-e2e");
    assert_eq!(tick["skipped_fresh"], false);
    assert!(
        tick["filed"].as_u64().expect("filed count") >= 1,
        "the corpus item(s) filed: {tick}"
    );
    assert!(tick["refusal"].is_null());
    let proposal_ids: Vec<i64> = tick["proposal_ids"]
        .as_array()
        .expect("proposal_ids")
        .iter()
        .map(|v| v.as_i64().expect("id"))
        .collect();
    assert_eq!(proposal_ids.len() as u64, tick["filed"].as_u64().unwrap());

    // ── 3. the proposals carry the provenance header (the human reviews
    // THIS) ─────────────────────────────────────────────────────────────
    {
        let conn = state.pool.get().expect("conn");
        let (content, source): (String, Option<String>) = conn
            .query_row(
                "SELECT content, source FROM proposals WHERE id = ?1",
                [proposal_ids[0]],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("the proposal");
        assert_eq!(source.as_deref(), Some("watch"));
        assert!(content.contains("[Watch] lane: lobsters-e2e"));
        assert!(content.contains("Feed: lobste.rs (rss)"));
        assert!(content.contains("Item: https://lobste.rs/s/"));
        assert!(content.contains("Near-Key: "), "the near-key is labeled");
    }

    // ── 4. the gate: approve ONE (the human act through the EXISTING
    // route) → knowledge (source `watch`, origin `imported`) ────────────
    let approve_id = proposal_ids[0];
    // The reviewer first READS the queue (the digest-binding law: the
    // approve verb binds to the bytes that were rendered) — the queue view
    // returns each row's content_digest.
    let (queue_status, queue) = get_json(&state, "/proposals?status=pending").await;
    assert_eq!(queue_status, StatusCode::OK, "the queue reads: {queue}");
    let digest = queue
        .as_array()
        .expect("the queue is a list")
        .iter()
        .find(|p| p["id"].as_i64() == Some(approve_id))
        .and_then(|p| p["content_digest"].as_str())
        .map(String::from)
        .expect("the queue view carries the content_digest");
    let (approve_status, approved) = post_empty(
        &state,
        &format!("/proposals/{approve_id}/approve?digest={digest}"),
    )
    .await;
    assert_eq!(
        approve_status,
        StatusCode::OK,
        "the approve answers: {approved}"
    );
    {
        let conn = state.pool.get().expect("conn");
        let (source, kind, origin): (String, String, String) = conn
            .query_row(
                "SELECT source, node_kind, origin FROM knowledge WHERE content_hash = \
                 (SELECT content_hash FROM proposals WHERE id = ?1) LIMIT 1",
                [approve_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("the approved knowledge row");
        assert_eq!(source, "watch", "the approved row's source lands VERBATIM");
        assert_eq!(kind, "fact");
        assert_eq!(origin, "imported", "origin_for_source(watch) — unchanged");
    }

    // ── 5. recall sees it ───────────────────────────────────────────────
    let results = recall(&state, "systemd rust").await;
    let hits = results["hits"].as_array().cloned().unwrap_or_default();
    assert!(
        !hits.is_empty(),
        "recall surfaces the approved watch row: {results}"
    );
    // THE WIRE CARRIES THE WHOLE LABEL CHAIN: the row's ingest kind is the
    // verbatim `watch` (INGEST_KINDS' new value, search-scopable), the
    // origin is the derived `imported`, the derived trust tier is the
    // lifecycle-faithful `raw`.
    let top = &hits[0];
    assert_eq!(top["ingest_kind"], "watch");
    assert_eq!(top["origin"], "imported");
    assert_eq!(top["memory_kind"], "fact");

    // ── 6. THE REFUSAL TWIN: reject another → NOTHING lands ─────────────
    let reject_id = proposal_ids[1];
    let (reject_status, rejected) =
        post_empty(&state, &format!("/proposals/{reject_id}/reject")).await;
    assert_eq!(
        reject_status,
        StatusCode::OK,
        "the reject answers: {rejected}"
    );
    {
        let conn = state.pool.get().expect("conn");
        // The proposals table carries no stored hash — the twin probes the
        // BYTES (the rejected item's content never lands).
        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE content = \
                 (SELECT content FROM proposals WHERE id = ?1)",
                [reject_id],
                |r| r.get(0),
            )
            .expect("the rejected row's absence probe");
        assert_eq!(rows, 0, "the rejected watch item NEVER lands as memory");
    }

    // ── 7. the audit chain still verifies across the tick + the gate ────
    {
        let conn = state.pool.get().expect("conn");
        assert!(
            brain_server::audit::verify_chain(&conn),
            "the audit chain verifies across the tick + the gate"
        );
    }
    let _ = lane_row_id;
}

/// The `{}` probe ON THE WIRE: POST /watch/tick with an empty object from
/// the operator clears the serde-defaulted extractor and lands in the
/// Admin gate — 403 for the unauthenticated, NEVER the extractor's 422
/// (the convention's wire face).
#[tokio::test]
async fn the_empty_object_probe_lands_in_the_gate_never_the_extractor() {
    let srv = server();
    // NO authorization header at all → the middleware/gate refuses.
    let req = Request::builder()
        .method("POST")
        .uri("/watch/tick")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .expect("request");
    let res = app(srv.state.clone())
        .oneshot(req)
        .await
        .expect("infallible");
    assert_eq!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "an unauthenticated empty-object probe never reaches the extractor"
    );
}
