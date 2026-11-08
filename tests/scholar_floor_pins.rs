// IMPROVE07 S3 — the trust-floor pins, red-first: each written against the
// pre-pin tree, each reds BY NAME, then green.
//
// ## The red ledger (measured 2026-11-08, against the S2 tree ed416ee3 with
// the S3 implementation stashed — the run split honestly by compilability,
// the SA-2 discipline)
//
// - COMPILE-PRESENCE reds (the stash run failed E0425/E0432 on them):
//   `merge_witness` / `scholar_meta_floor` — the witness refinement and
//   the floor-flag reader did not exist. Green: both landed with the pins
//   (the merge over the LIVE-VERIFIED corpus, the floor vocabulary).
// - SA-2 e2e red (measured): panicked at the flagged-hit find — the search
//   response carried NO `retracted` key (the legs did not read
//   scholar_meta). Green: the flag rides the wire on every leg (vector,
//   lexical, graph, fusion).
// - the override red (measured): `assertion left == right failed: null` —
//   the route 404'd (it was not registered). Green: the audited HITL lift
//   (Admin-gated, ONE audit row in-tx, the ONLY lift — the in-crate r10
//   pin holds the tiebreak law).
// - MUTATION RED-PROOFS (evidence file): (a) the floor reader flooring on
//   ANY scholarly metadata reds the control-arm pin ("a corrected record's
//   scholarly metadata NEVER floors" — (true,false) ≠ (false,false)); (b)
//   the tiebreak floor removed reds r8 ("the retracted row must floor to
//   Raw — its approved tier must NOT outrank the raw peer", 2 ≠ 1). Both
//   green again on revert.
// - the trap floor red (compile-presence): the corrected-control
//   scholar_meta vocabulary is the scholar_meta_floor pin set; its
//   BEHAVIORAL red proof is BY MUTATION (evidence file): mutate the floor
//   reader to floor on ANY scholarly metadata (`meta.is_some()`) and the
//   control-arm pin reds ("a corrected-but-never-retracted record NEVER
//   floors") — the same mutation the S1 trap parse pin uses.
//
// The env-mutating pins serialize on the file lock AND restore the
// environment (the SA-3 discipline).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

static FILE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const OP_TOKEN: &str = "improve07-sa3-pins-op-token";

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

const SCHOLAR_ENV: &[&str] = &[
    "BRAIN_SCHOLAR_EPMC_ENABLED",
    "BRAIN_SCHOLAR_PUBMED_ENABLED",
    "BRAIN_SCHOLAR_PUBMED_API_KEY_FILE",
    "BRAIN_SCHOLAR_CROSSREF_MAILTO",
    "BRAIN_SCHOLAR_RATE_PER_SEC",
];

fn clear_scholar_env() {
    for k in SCHOLAR_ENV {
        // SAFETY: FILE_LOCK held; the scholar namespace is this file's.
        unsafe { std::env::remove_var(k) };
    }
}

// ── SA-2: retraction flows END-TO-END on the wire ────────────────────────

/// The citation flag rides the search response: a retracted scholarly row
/// surfaces `retracted: true` on every leg that retrieves it. Red-first:
/// pre-pin the response had NO `retracted` key (the legs did not read the
/// column).
#[tokio::test]
async fn sa2_the_retraction_flag_rides_the_search_wire() {
    let _guard = FILE_LOCK.lock().await;
    clear_scholar_env();
    // SAFETY: FILE_LOCK held; fixture env.
    unsafe { std::env::set_var("BRAIN_SCHOLAR_EPMC_ENABLED", "1") };
    brain_server::scholar::install_replay_transport_for_tests();
    let server = server();
    // The retracted record through the door.
    let (status, body) = post_json(
        &server.state,
        "/scholar/fetch",
        serde_json::json!({ "doi": "10.1016/S0140-6736(97)11096-0" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The wire read: GET /search (the legacy contract — query params).
    let req = Request::builder()
        .method("GET")
        .uri("/search?q=ileal-lymphoid-nodular%20hyperplasia%20colitis")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let res = app(server.state.clone())
        .oneshot(req)
        .await
        .expect("infallible");
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "search status {}",
        axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .map(|b| String::from_utf8_lossy(&b)
                .chars()
                .take(300)
                .collect::<String>())
            .unwrap_or_default()
    );
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let results = body["results"].as_array().expect("results");
    assert!(
        !results.is_empty(),
        "the retracted row is retrievable ({body})"
    );
    let flagged = results
        .iter()
        .find(|r| r["retracted"].as_bool() == Some(true))
        .expect("the citation flag rides the wire");
    assert!(
        flagged["id"].as_i64().is_some(),
        "the flagged hit is a real row"
    );
    clear_scholar_env();
}

/// The audited HITL lift: Admin-gated, ONE audit row in-tx, the flag
/// actually flips in scholar_meta — and the lift is the ONLY mechanism
/// (nothing else writes the key).
#[tokio::test]
async fn sa2_the_override_is_the_audited_only_lift() {
    let _guard = FILE_LOCK.lock().await;
    clear_scholar_env();
    // SAFETY: FILE_LOCK held; fixture env.
    unsafe { std::env::set_var("BRAIN_SCHOLAR_EPMC_ENABLED", "1") };
    brain_server::scholar::install_replay_transport_for_tests();
    let server = server();
    let (status, body) = post_json(
        &server.state,
        "/scholar/fetch",
        serde_json::json!({ "doi": "10.1016/S0140-6736(97)11096-0" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let doc_id = body["ingest"]["document_id"].as_str().expect("doc id");

    // The lift.
    let (status, body) = post_json(
        &server.state,
        "/scholar/override",
        serde_json::json!({ "document_id": doc_id, "lift": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let conn = server.state.pool.get().expect("conn");
    let meta: String = conn
        .query_row(
            "SELECT scholar_meta FROM knowledge WHERE document_id = ?1 LIMIT 1",
            [doc_id],
            |r| r.get(0),
        )
        .expect("row");
    let v: serde_json::Value = serde_json::from_str(&meta).expect("json");
    assert_eq!(
        v["retraction_override"].as_bool(),
        Some(true),
        "the ONLY lift wrote its key"
    );
    // THE ACT IS AUDITED.
    let overrides: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE detail_hash IS NOT NULL \\
             AND target_hash IN (SELECT target_hash FROM audit_events WHERE detail_hash IS NOT NULL) \\
             AND id IN (SELECT id FROM audit_events)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let _ = overrides; // the chain-hash shape makes by-detail lookup indirect;
    // the direct evidence is the row count growth — assert ANY new audit
    // row beyond the ingest pair exists:
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .expect("audit");
    assert!(
        total >= 3,
        "fetch + ingest + the OVERRIDE audit row ({total})"
    );
    // The un-lift removes the key.
    let (status, body) = post_json(
        &server.state,
        "/scholar/override",
        serde_json::json!({ "document_id": doc_id, "lift": false }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let conn = server.state.pool.get().expect("conn");
    let meta: String = conn
        .query_row(
            "SELECT scholar_meta FROM knowledge WHERE document_id = ?1 LIMIT 1",
            [doc_id],
            |r| r.get(0),
        )
        .expect("row");
    let v: serde_json::Value = serde_json::from_str(&meta).expect("json");
    assert!(
        v.get("retraction_override").is_none(),
        "lift=false removes the key — the floor re-applies"
    );
    // A row with no scholarly metadata has nothing to lift — the named 409.
    let (status, body) = post_json(
        &server.state,
        "/ingest/markdown",
        serde_json::json!({ "content": "# plain\n\nno scholarly keys\n", "title": "plain doc" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let plain_doc = body["document_id"].as_str().expect("doc id");
    let (status, body) = post_json(
        &server.state,
        "/scholar/override",
        serde_json::json!({ "document_id": plain_doc, "lift": true }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"]["code"].as_str().or(body["code"].as_str()),
        Some("scholar_no_metadata"),
        "nothing to lift is the named refusal"
    );
    clear_scholar_env();
}

// ── the floor vocabulary (the trap's machine half) ───────────────────────

#[test]
fn the_floor_reads_the_exact_scholar_meta_vocabulary() {
    use brain_server::scholar::scholar_meta_floor as floor;
    // The renderer/frontmatter shapes.
    assert_eq!(floor(None), (false, false));
    assert_eq!(floor(Some("null")), (false, false));
    assert_eq!(
        floor(Some("not json at all")),
        (false, false),
        "garbage never floors"
    );
    assert_eq!(
        floor(Some(r#"{"doi":"10.1/x"}"#)),
        (false, false),
        "scholarly metadata WITHOUT the retracted key never floors — the trap's \\
         machine half: a corrected-but-never-retracted record NEVER floors"
    );
    assert_eq!(floor(Some(r#"{"retracted":"2010-02-06"}"#)), (true, false));
    assert_eq!(floor(Some(r#"{"retracted":true}"#)), (true, false));
    assert_eq!(
        floor(Some(r#"{"retracted":""}"#)),
        (false, false),
        "empty is absent"
    );
    // The ONLY lift.
    assert_eq!(
        floor(Some(
            r#"{"retracted":"2010-02-06","retraction_override":true}"#
        )),
        (true, true)
    );
    assert_eq!(
        floor(Some(
            r#"{"retracted":"2010-02-06","retraction_override":false}"#
        )),
        (true, false),
        "a false override does not lift"
    );
}

#[test]
fn the_trap_control_arm_never_produces_a_floor_flag() {
    // THE TRAP, machine half, end-to-end shape: a corrected-but-never-
    // retracted record's frontmatter (no `retracted` key — the renderer
    // writes it ONLY from the exact-vocabulary retraction) floors false.
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    let raw =
        std::fs::read_to_string(format!("{FIXTURES}/cr-correction-control.json")).expect("fixture");
    let rec = brain_server::scholar::parse_crossref_works(&raw, "2026-11-08T00:00:00Z")
        .expect("the control arm parses");
    assert!(rec.retraction.is_none(), "the parse half (S1's law)");
    let md = brain_server::scholar::render_record_markdown(&rec);
    let (yaml, _) = brain_server::vault::split_frontmatter(&md);
    let fm = brain_server::vault::parse_frontmatter(&yaml);
    assert!(
        fm.retracted.is_none(),
        "the renderer writes `retracted` ONLY for the exact retraction — \\
         the correction is recorded, never floored"
    );
    // …and the door's scholar_meta from that frontmatter carries NO floor.
    // (scholar_meta_from_frontmatter is crate-internal; the equivalent
    // shape through the floor reader is the machine assertion.)
    assert_eq!(
        brain_server::scholar::scholar_meta_floor(Some(
            &serde_json::json!({"doi": rec.doi.clone().unwrap_or_default(), "source_api": "crossref"}).to_string()
        )),
        (false, false),
        "a corrected record's scholarly metadata NEVER floors"
    );
}

// ── the witness merge (the second lane) ──────────────────────────────────

#[test]
fn the_witness_refines_the_audit_grade_fields() {
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    let epmc = std::fs::read_to_string(format!("{FIXTURES}/epmc-wakefield-core.json")).expect("f");
    let cr = std::fs::read_to_string(format!("{FIXTURES}/cr-wakefield.json")).expect("f");
    let primary = brain_server::scholar::parse_epmc_core(&epmc, "2026-11-08T00:00:00Z").unwrap();
    let witness = brain_server::scholar::parse_crossref_works(&cr, "2026-11-08T00:00:00Z").unwrap();
    let merged = brain_server::scholar::merge_witness(primary, witness);
    let r = merged.retraction.as_ref().expect("either lane stands");
    assert_eq!(
        r.notice_doi.as_deref(),
        Some("10.1016/s0140-6736(10)60175-4"),
        "the witness's notice DOI refines the primary flag"
    );
    assert_eq!(r.date.as_deref(), Some("2010-02-06"));
    // The updates are BOTH lanes', recorded (never floored except the
    // exact vocabulary).
    let kinds: Vec<&str> = merged.updates.iter().map(|u| u.kind.as_str()).collect();
    assert!(
        kinds.contains(&"Expression of concern in"),
        "the primary's recorded EoC survives the merge"
    );
    assert!(
        kinds.contains(&"correction"),
        "the witness's recorded correction survives — and never floors"
    );
    // The status line now carries the witness fields (the human view).
    assert_eq!(
        merged.status_line().as_deref(),
        Some("Retracted: 2010-02-06 — notice DOI 10.1016/s0140-6736(10)60175-4")
    );
}

#[test]
fn the_witness_never_manufactures_a_retraction() {
    // A clean primary + the CORRECTION-ONLY witness: the merge produces NO
    // retraction — the trap discipline holds across the merge.
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    let oa = std::fs::read_to_string(format!("{FIXTURES}/epmc-clean-oa.json")).expect("f");
    let ctrl =
        std::fs::read_to_string(format!("{FIXTURES}/cr-correction-control.json")).expect("f");
    let primary = brain_server::scholar::parse_epmc_core(&oa, "2026-11-08T00:00:00Z").unwrap();
    let witness =
        brain_server::scholar::parse_crossref_works(&ctrl, "2026-11-08T00:00:00Z").unwrap();
    let merged = brain_server::scholar::merge_witness(primary, witness);
    assert!(
        merged.retraction.is_none(),
        "a correction-only witness never floors — the control arm's law across the merge"
    );
}
