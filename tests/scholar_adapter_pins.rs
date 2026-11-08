// IMPROVE07 S2 — the adapter-lane pins, red-first: each written against the
// pre-pin tree, each reds BY NAME, then green.
//
// ## The red ledger (measured 2026-11-08, against the S1 tree a43d7b4f with
// the S2 implementation stashed — the run split honestly by compilability:
// four pins reference S2-only symbols and CANNOT compile pre-pin, so they
// were dropped for the red run (their red is COMPILE-PRESENCE: the stash
// attempt's unresolved imports are the named red) and the compile-compatible
// six ran and red BY NAME)
//
// - COMPILE-PRESENCE reds (the stash run failed E0425/E0432/E0433 on them):
//   `ALLOWED_SCHOLAR_HOSTS` / `ScholarTransport` /
//   `install_replay_transport_for_tests` / `render_record_markdown` /
//   `backoff_decision` / `BackoffDecision` — the client layer did not
//   exist. Green: the symbols landed with the pins (SA-1 behavioral, SA-3
//   on-state, the 429 decision, the renderer round-trip).
// - SA-1 structural red: `src/scholar.rs` carried no allowlist const, no
//   `redirect::Policy::none()`, no `https_only` — panicked at the first
//   host assert. Green: the client layer landed with the pins.
// - SA-3 off red: the unconfigured route red `assertion left == right
//   failed: null` — the route 404'd (it was not registered), so no named
//   code came back. Green: 503 `scholar_unconfigured`.
// - SA-3 partial red: same 404 shape. Green: 503 `scholar_config_invalid`
//   (pubmed enabled WITHOUT its key file is incomplete, never degraded).
// - SA-4 structural red: `left: 0, right: 1` — the production source named
//   NO replay arm at all. Green: the replay arm is the ONLY test-side
//   construction (doc-hidden installer) and the route's production path
//   never touches it.
// - the doors-identical red: panicked at the route-not-found expect — the
//   router named no `scholar_fetch`. Green: the route CALLS the door
//   handler; no private INSERT exists in the route body.
// - the vocabulary red: panicked at `parse_source_filter("scholarly")` —
//   the kind was not in the allow-list. Green: both vocabularies extended;
//   `origin_for_source("scholarly")` derives `imported` unchanged.
// - the 429 backoff pin: compile-presence red — its behavioral red proof
//   is BY MUTATION (evidence file): flip the rate-shaped gate to
//   `status >= 500` only and the 429 pin reds.
//
// The env-mutating pins (SA-3) serialize on the file lock AND restore the
// environment — other suites read the same process env space.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

mod common;

/// Serializes the env-mutating pins in this file (the SA-3 on/off pair and
/// the route tests): `BRAIN_SCHOLAR_*` is process-global.
static FILE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const OP_TOKEN: &str = "improve07-sa2-pins-op-token";

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

/// SAFETY: the env-mutating pins hold FILE_LOCK; no concurrent test in this
/// binary touches the scholar namespace, and the values are test fixtures.
fn clear_scholar_env() {
    for k in SCHOLAR_ENV {
        // SAFETY: single-threaded w.r.t. this namespace by FILE_LOCK.
        unsafe { std::env::remove_var(k) };
    }
}

// ── SA-1: the allowlist — structural + behavioral ────────────────────────

/// The allowlist is pinned STRUCTURALLY: the send path host-checks, the
/// live client never follows redirects (a redirect is an allowlist
/// bypass), is https-only, and disables ambient proxies. Production text
/// only (comment-stripped) — a comment naming the law satisfies nothing.
#[test]
fn sa1_the_allowlist_is_pinned_structurally() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/scholar.rs"))
        .expect("the scholar source");
    let prod = common::code_only(&src);
    for host in [
        "www.ebi.ac.uk",
        "europepmc.org",
        "eutils.ncbi.nlm.nih.gov",
        "api.crossref.org",
    ] {
        assert!(
            prod.contains(&format!("\"{host}\"")),
            "the allowlist pins {host} by name"
        );
    }
    assert!(
        prod.contains(".redirect(reqwest::redirect::Policy::none())"),
        "the live client NEVER follows a redirect — a redirect is an allowlist bypass"
    );
    assert!(
        prod.contains(".https_only(true)"),
        "the egress is https-only"
    );
    assert!(
        prod.contains(".no_proxy()"),
        "no ambient proxy bypasses the pin"
    );
    assert!(
        prod.contains("fn assert_allowed_host"),
        "the host check is the shared enforcement point (live AND replay)"
    );
}

/// SA-1 behavioral: any non-allowlisted host is refused BY NAME — through
/// the shared check AND through the transport surface (a test transport
/// cannot bypass it either).
#[tokio::test]
async fn sa1_any_other_host_is_refused_by_name() {
    assert_eq!(
        brain_server::scholar::ALLOWED_SCHOLAR_HOSTS.len(),
        4,
        "the allowlist is exactly the four publisher hosts"
    );
    let t = brain_server::scholar::ScholarTransport::replay();
    for evil in [
        "https://evil.example.com/steal",
        "http://www.ebi.ac.uk.evil.example.com/x",
        "https://api.crossref.org.evil.example.com/works/10.1/x",
    ] {
        let err = t.get(evil).await.expect_err("the host must be refused");
        assert_eq!(
            err.to_string(),
            "scholar_host_refused: not an allowlisted scholarly host \
             (www.ebi.ac.uk | europepmc.org | eutils.ncbi.nlm.nih.gov | api.crossref.org)",
            "the refusal names the law"
        );
    }
    // A lookalike SUBDOMAIN is not the allowlisted host.
    let ok = t
        .get("https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:%2210.1016%2FS0140-6736(97)11096-0%22&format=json&resultType=core")
        .await
        .expect("the exact allowlisted host serves the fixture");
    assert_eq!(ok.status, 200);
}

// ── SA-3: OFF unless configured; the on-state audited per fetch ──────────

/// Unconfigured = OFF, named. Red-first: pre-pin this red 404 (the route
/// was not registered).
#[tokio::test]
async fn sa3_the_route_is_off_unconfigured_named() {
    let _guard = FILE_LOCK.lock().await;
    clear_scholar_env();
    let server = server();
    let (status, body) = post_json(
        &server.state,
        "/scholar/fetch",
        serde_json::json!({ "doi": "10.1016/S0140-6736(97)11096-0" }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        body["error"]["code"].as_str().or(body["code"].as_str()),
        Some("scholar_unconfigured"),
        "the refusal NAMES the operator act (SA-3)"
    );
}

/// The on-state: configured + the replay transport → the fetch resolves
/// through the frozen corpus, the record crosses the REAL door, and BOTH
/// audit rows exist (the fetch audit SA-3 + the door's ingest audit).
#[tokio::test]
async fn sa3_the_on_state_audits_the_fetch_and_crosses_the_door() {
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
    assert_eq!(
        body["fetched"]["source_api"].as_str(),
        Some("europepmc"),
        "auto selects the PRIMARY adapter"
    );
    assert_eq!(
        body["fetched"]["retracted"].as_bool(),
        Some(true),
        "the EPMC pubType string floors into the fetch summary"
    );
    assert_eq!(
        body["ingest"]["success"].as_bool(),
        Some(true),
        "the door's own response crosses verbatim"
    );
    let doc_id = body["ingest"]["document_id"].as_str().expect("doc id");

    // BOTH audit rows: the fetch audit (scholar_route) and the door's
    // ingest audit (api).
    let conn = server.state.pool.get().expect("conn");
    let fetch_audits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE actor = 'scholar_route'",
            [],
            |r| r.get(0),
        )
        .expect("audit");
    assert!(fetch_audits >= 1, "the fetch audit row exists (SA-3)");
    let ingest_audits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE actor = 'api' AND status = 'ok'",
            [],
            |r| r.get(0),
        )
        .expect("audit");
    assert!(ingest_audits >= 1, "the DOOR's ingest audit row exists");

    // The record's kind + the machine flag on the row.
    let (kind, meta): (String, Option<String>) = conn
        .query_row(
            "SELECT source, scholar_meta FROM knowledge WHERE document_id = ?1 LIMIT 1",
            [doc_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("row");
    assert_eq!(kind, "scholarly", "the route's record is kind scholarly");
    let meta: serde_json::Value =
        serde_json::from_str(&meta.expect("the flag persisted")).expect("json");
    assert_eq!(meta["doi"].as_str(), Some("10.1016/s0140-6736(97)11096-0"));
    assert_eq!(meta["source_api"].as_str(), Some("europepmc"));

    // The re-fetch is the DEDUPE path: identical chunks, the flag still
    // refreshed (the persist runs by document_id).
    let (status, body) = post_json(
        &server.state,
        "/scholar/fetch",
        serde_json::json!({ "doi": "10.1016/S0140-6736(97)11096-0" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["ingest"]["chunks_duplicate"].as_i64().unwrap_or(0) > 0
            || body["ingest"]["chunks_inserted"].as_i64().unwrap_or(1) == 0,
        "the re-serve deduped ({body})"
    );
    let conn = server.state.pool.get().expect("conn");
    let meta2: Option<String> = conn
        .query_row(
            "SELECT scholar_meta FROM knowledge WHERE document_id = ?1 LIMIT 1",
            [doc_id],
            |r| r.get(0),
        )
        .expect("row");
    assert!(meta2.is_some(), "the flag SURVIVES the deduped re-serve");
    clear_scholar_env();
}

/// A partial profile is refused — enabled PubMed without its key file is
/// the GDL incomplete-shape refusal (the boot validator's law, surfaced
/// per request).
#[tokio::test]
async fn sa3_a_partial_profile_is_a_named_refusal() {
    let _guard = FILE_LOCK.lock().await;
    clear_scholar_env();
    // SAFETY: FILE_LOCK held; fixture env.
    unsafe { std::env::set_var("BRAIN_SCHOLAR_PUBMED_ENABLED", "1") };
    let server = server();
    let (status, body) = post_json(
        &server.state,
        "/scholar/fetch",
        serde_json::json!({ "doi": "10.1016/S0140-6736(97)11096-0" }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(
        body["error"]["code"].as_str().or(body["code"].as_str()),
        Some("scholar_config_invalid"),
        "pubmed enabled WITHOUT its key file is incomplete, never degraded"
    );
    clear_scholar_env();
}

// ── SA-4: CI replays — the live arm never constructs in tests ────────────

#[test]
fn sa4_the_test_surface_constructs_the_replay_only() {
    // The production text names the replay constructor NOWHERE outside a
    // #[cfg(test)] block (the cfg(test) installer is the only test seam).
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/scholar.rs"))
        .expect("the scholar source");
    let prod = common::code_only(&src);
    let occurrences = prod.matches("ScholarTransport::replay").count();
    assert_eq!(
        occurrences, 1,
        "the replay constructor is named ONLY inside the doc-hidden test installer — \
         production never constructs the replay arm (the replay is the CI/test posture)"
    );
    let installer_at = prod
        .find("pub fn install_replay_transport_for_tests")
        .expect("the installer exists");
    assert!(
        prod[installer_at..].contains("ScholarTransport::replay"),
        "the single replay construction lives in the installer"
    );
    let route_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/memory.rs"
    ))
    .expect("the router source");
    assert!(
        !common::code_only(&route_src).contains("install_replay_transport_for_tests"),
        "the route's production path NEVER installs the replay transport — \
         the live arm is the operator posture"
    );
    // The test surfaces construct it via the cfg(test) installer.
    let pins = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/scholar_adapter_pins.rs"
    ))
    .expect("this pin file");
    assert!(
        pins.contains("install_replay_transport_for_tests()"),
        "the route pins drive the REPLAY transport (SA-4: CI never performs a live call)"
    );
    // And the live arm's only production constructor is the cfg-gated
    // process_transport path, pinned to the allowlist by the SA-1 pin.
}

// ── the doors-identical law ──────────────────────────────────────────────

#[test]
fn the_route_calls_the_real_door_no_private_insert_exists() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/memory.rs"
    ))
    .expect("the router source");
    let prod = common::code_only(&src);
    let route_at = prod
        .find("pub(crate) async fn scholar_fetch")
        .expect("the scholar route exists");
    let route_body = prod[route_at..]
        .split("fn scholar_audit_target")
        .next()
        .expect("split yields a first slice");
    assert!(
        route_body.contains("ingest_markdown("),
        "the route CALLS the door handler — an adapter is a FETCHER, never a promoter"
    );
    // And no private INSERT INTO knowledge exists in the route body.
    assert!(
        !route_body.contains("INSERT INTO knowledge"),
        "no private insert to bypass with"
    );
}

// ── the vocabularies moved together ──────────────────────────────────────

#[test]
fn the_scholarly_kind_is_in_both_vocabularies_and_derives_imported() {
    // The filter's allow-list.
    let f = brain_server::search::query::parse_source_filter("scholarly")
        .expect("scholarly is a valid ingest kind");
    match f {
        brain_server::search::query::SourceFilter::Kind(k) => assert_eq!(k, "scholarly"),
        other => panic!("unexpected filter {other:?}"),
    }
    // The JWT allowlist.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/memory.rs"
    ))
    .expect("the router source");
    assert!(
        common::code_only(&src).contains("\"scholarly\","),
        "ADD_SOURCES_FOR_JWT carries the kind (the vocabularies move together)"
    );
    // The origin derives imported — a fetched record is not a trusted
    // local write.
    assert_eq!(
        brain_server::gate::origin_for_source(Some("scholarly")),
        "imported",
        "origin_for_source is UNCHANGED: unknown kinds derive imported"
    );
}

// ── the 429 transcript drives the backoff decision (SA-7) ────────────────

#[test]
fn the_backoff_decision_honors_the_recorded_429() {
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    let raw = std::fs::read_to_string(format!("{FIXTURES}/transcript-429.json")).expect("fixture");
    let t: serde_json::Value = serde_json::from_str(&raw).expect("the transcript parses");
    let status = t["status"].as_u64().unwrap() as u16;
    let retry_after = t["headers"]["retry-after"]
        .as_str()
        .and_then(|v| v.parse::<u64>().ok());

    use brain_server::scholar::{BackoffDecision, backoff_decision};
    // Attempt 1 on a 429: retry, honoring retry-after.
    match backoff_decision(status, retry_after, 1) {
        BackoffDecision::Retry { wait_secs } => assert_eq!(wait_secs, 1, "retry-after honored"),
        BackoffDecision::Refused => panic!("the first 429 must back off, not refuse"),
    }
    // The budget is TWO attempts: the second 429 refuses (named).
    assert!(matches!(
        backoff_decision(status, retry_after, 2),
        BackoffDecision::Refused
    ));
    // 5xx is rate-shaped too; a big retry-after is CAPPED.
    match backoff_decision(503, Some(999), 1) {
        BackoffDecision::Retry { wait_secs } => {
            assert_eq!(wait_secs, 5, "the backoff cap holds")
        }
        BackoffDecision::Refused => panic!("a 5xx within budget must back off"),
    }
    // A 2xx never backs off.
    assert!(matches!(
        backoff_decision(200, None, 1),
        BackoffDecision::Refused
    ));
}

// ── the renderer: deterministic, door-shaped ─────────────────────────────

#[test]
fn the_renderer_emits_the_scholarly_frontmatter_and_the_status_line() {
    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    let raw = std::fs::read_to_string(format!("{FIXTURES}/cr-wakefield.json")).expect("fixture");
    let rec = brain_server::scholar::parse_crossref_works(&raw, "2026-11-08T00:00:00Z")
        .expect("the trap fixture parses");
    let md = brain_server::scholar::render_record_markdown(&rec);
    // The door parses it back: the quoted values unquote, the scholarly
    // vocabulary round-trips.
    let (yaml, body) = brain_server::vault::split_frontmatter(&md);
    let fm = brain_server::vault::parse_frontmatter(&yaml);
    assert_eq!(fm.doi.as_deref(), Some("10.1016/s0140-6736(97)11096-0"));
    assert_eq!(fm.source_api.as_deref(), Some("crossref"));
    assert_eq!(fm.retracted.as_deref(), Some("2010-02-06"));
    assert_eq!(fm.license, None, "Crossref carries no CC license here");
    // The status line opens the body — the human view.
    let first = body.lines().next().unwrap_or("");
    assert_eq!(
        first, "Retracted: 2010-02-06 — notice DOI 10.1016/s0140-6736(10)60175-4",
        "the witness fields feed the deterministic status line"
    );
    // Determinism: the same record renders byte-identically.
    assert_eq!(md, brain_server::scholar::render_record_markdown(&rec));
}
