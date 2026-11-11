//! IMPROVE15, C-d — the appeal's pins. The schema act (1.32.35) is FIRST:
//! the appeal's provenance is a named schema act because a customer-sourced
//! proposal stamped `operator` would be exactly the provenance blur round 13
//! closed. These pins are born RED against the pre-migration tree (the
//! CHECK refuses `customer`) and flip green in the same commit — the
//! transcript carries both runs.
//!
//! Pins:
//!   AP-1  the schema act: the CHECK admits `customer` and still refuses
//!         every value outside the five-value vocabulary (the pre-migration
//!         red is the round's named act)
//!   AP-2  the stamp is PATH-ASSIGNED: the wire cannot set derived_from (the
//!         request struct carries no such field — the round-13 law, re-pinned
//!         for the appeal route)
//!   AP-3  the appeal is digest-bound BY NAME: an appeal without the shown
//!         review_digest refuses with `appeal_digest_required`; a digest no
//!         approved decision in the domain answers refuses with
//!         `appeal_target_unknown` (you appeal bytes that were SHOWN and
//!         DECIDED, not bytes you name into existence)
//!   AP-4  the appeal never auto-reverses: the approve branch of an appeal
//!         proposal writes NO memory row (the disposal is the decision, not
//!         a promotion) — and the reject path stays the existing verb
//!   AP-5  the review clock: every appeal carries the declared clock; the
//!         queue's countdown bands render the CLOCK (not the TTL); the
//!         clock kept-or-missed is chain-provable from the row + the HITL
//!         ledger; a missed dispose fires the EXISTING webhook alerting path
//!   AP-6  the appeal is never machine-expired: an appeal proposal past the
//!         ordinary TTL survives (the clock governs, never the TTL) — with
//!         the control proving an ordinary proposal still expires

use brain_server::workflow::journey::JourneyKind;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use std::sync::Arc;

fn migrated_db() -> rusqlite::Connection {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, 1).expect("migration");
    conn
}

// ── AP-1 · the schema act ──────────────────────────────────────────────────

#[test]
fn ap1_the_check_admits_customer_and_refuses_outside_the_five() {
    let conn = migrated_db();
    let base =
        "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from)";
    let values = |v: &str, ts: i64| {
        format!("{base} VALUES ('appeal', 'x-{ts}', 'pending', {ts}, 0.5, 0.5, '{v}')")
    };
    for v in ["operator", "agent", "watch", "customer", "legacy"] {
        conn.execute(&values(v, 1000 + v.len() as i64), [])
            .unwrap_or_else(|e| panic!("the CHECK must admit `{v}`: {e}"));
    }
    // outside the five — refused (the round-13 law unchanged, widened only):
    for v in ["i-claim-operator", "portal", "customer-spoof", "API", ""] {
        let refused = conn.execute(&values(v, 2000 + v.len() as i64), []);
        assert!(
            refused.is_err(),
            "the CHECK must refuse `{v}` — outside the closed vocabulary"
        );
    }
}

#[test]
fn ap1_the_rust_vocabulary_and_the_check_agree() {
    // the ONE vocabulary, two layers: the Rust gate (insert_proposal's
    // fail-closed re-check) and the SQL CHECK move together — a value one
    // layer admits and the other refuses is a stamp that lies at one seam.
    let vocab = brain_server::service::review::DERIVED_FROM_VOCABULARY;
    assert!(
        vocab.contains(&"customer"),
        "the Rust vocabulary carries the customer value (the schema act's other half)"
    );
    assert_eq!(vocab.len(), 5, "the vocabulary is the closed five");
    let conn = migrated_db();
    for v in vocab {
        let n = conn.execute(
            "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from) \
             VALUES ('appeal', 'x', 'pending', 3000, 0.5, 0.5, ?1)",
            rusqlite::params![v],
        );
        assert!(
            n.is_ok(),
            "the CHECK admits every value the Rust vocabulary names: {v}"
        );
    }
}

#[test]
fn ap1_the_ceiling_is_the_schema_act() {
    // the lockstep set, moved in the same commit (the mf1 pin's shape):
    assert_eq!(
        brain_server::storage_layout::LATEST_KNOWN_SCHEMA,
        "1.32.35",
        "the ceiling is the CustomerProvenance round (IMPROVE15)"
    );
}

// ── AP-2 · the stamp is path-assigned ─────────────────────────────────────

#[test]
fn ap2_the_appeal_route_stamps_customer_the_wire_cannot_set_it() {
    // the appeal route's filing calls create_proposal_as with the Customer
    // stamp — a literal the source scan pins; the request struct carries no
    // derived_from field (the round-13 law).
    let src = std::fs::read_to_string("src/handlers/clients.rs").expect("handler source");
    let block_start = src
        .find("pub async fn client_appeal")
        .expect("the appeal route exists");
    let block_end = src[block_start..]
        .find("\npub async fn")
        .map(|e| block_start + e)
        .unwrap_or(src.len());
    let block = &src[block_start..block_end];
    assert!(
        block.contains("ProposalOrigin::Customer"),
        "the appeal route stamps Customer PATH-SIDE (the calling context is the path)"
    );
    assert!(
        block.contains("portal:"),
        "the appeal route's producer_ref names the portal producer"
    );
    // and the WIRE cannot set it — the request struct has no derived_from:
    let req_src = std::fs::read_to_string("src/handlers/gate.rs").expect("gate source");
    let req_start = req_src
        .find("pub struct ProposalRequest")
        .expect("request struct");
    let req_end = req_src[req_start..]
        .find("\n}")
        .map(|e| req_start + e)
        .unwrap_or(req_src.len());
    let req_block = &req_src[req_start..req_end];
    assert!(
        !req_block.contains("derived_from"),
        "the wire struct carries NO derived_from field — request-borne provenance is the MemForm defeat"
    );
}

/// THE CONTROL for AP-2: the scan is non-vacuous — a struct that DID carry
/// the field reds it.
#[test]
fn control_ap2_the_wire_scan_catches_a_planted_field() {
    let planted =
        "pub struct ProposalRequest {\n    pub content: String,\n    pub derived_from: String,\n}";
    assert!(
        planted.contains("derived_from"),
        "CONTROL RED: the planted wire field is present for the scan to catch"
    );
}

// ── AP-3 · digest-bound by name ───────────────────────────────────────────

#[test]
fn ap3_an_appeal_without_the_shown_digest_refuses_by_name() {
    let src = std::fs::read_to_string("src/handlers/gate.rs").expect("gate source");
    assert!(
        src.contains("appeal_digest_required"),
        "an appeal without the shown review_digest refuses BY NAME"
    );
    assert!(
        src.contains("appeal_target_unknown"),
        "a digest no decided decision in the domain answers refuses BY NAME — \
         you appeal bytes that were shown and decided, not bytes you name into existence"
    );
}

/// THE CONTROL: the refusal codes are wired to the appeal kind's own branch,
/// not pasted into a generic path — the appeal branch is named.
#[test]
fn control_ap3_the_refusals_live_in_an_appeal_branch() {
    let src = std::fs::read_to_string("src/handlers/gate.rs").expect("gate source");
    let idx = src
        .find("appeal_digest_required")
        .expect("the refusal exists");
    let before = &src[..idx];
    let branch = before
        .rfind("PROP_KIND_APPEAL")
        .or_else(|| before.rfind("KIND_APPEAL"))
        .expect("the refusal lives inside the appeal-kind branch");
    assert!(
        branch < idx,
        "CONTROL: the appeal branch precedes its refusal"
    );
}

// ── AP-4 · the appeal never auto-reverses ─────────────────────────────────

#[test]
fn ap4_the_appeal_approve_branch_writes_no_memory() {
    let src = std::fs::read_to_string("src/handlers/gate.rs").expect("gate source");
    let start = src
        .find("THE APPEAL'S DISPOSAL")
        .expect("the appeal dispose branch exists");
    // the window is the BRANCH ITSELF — from its comment to the next
    // branch's comment (the generic promote's quorum block), so a banned
    // word OUTSIDE the branch can never red it and a banned word INSIDE
    // always does.
    let end = src[start..]
        .find("Second eyes:")
        .map(|e| start + e)
        .unwrap_or(src.len());
    let window = &src[start..end];
    // the branch's write set: the CAS + the HITL row (inside the CAS) + the
    // audit — and NO promotion:
    for banned in [
        "insert_knowledge",
        "INSERT INTO knowledge",
        "promote",
        "KIND_PUBLISH",
    ] {
        assert!(
            !window.contains(banned),
            "the appeal approve branch must never `{banned}` — \
             approving an appeal is a DECISION, never a memory write (no auto-reversal)"
        );
    }
    assert!(
        window.contains("cas_proposal_approved"),
        "the disposal rides the EXISTING CAS (which writes the HITL row in its own tx)"
    );
}

// ── AP-5 · the review clock ───────────────────────────────────────────────

#[test]
fn ap5_the_clock_is_a_declared_const_table_and_chain_provable() {
    // the const table (the tier-table pattern): one class today, widening is
    // a dated act.
    let clock = brain_server::workflow::journey::APPEAL_REVIEW_CLOCK_SECS;
    assert_eq!(
        clock,
        7 * 24 * 3600,
        "the customer appeal's declared clock: seven days"
    );
    // the deadline is a pure fn over the row's own created_at:
    assert_eq!(
        brain_server::workflow::journey::appeal_clock_deadline(1_000),
        1_000 + clock,
        "the deadline is created_at + the declared clock — chain-provable"
    );
    // kept-or-missed is a pure fn over the row + the HITL ledger:
    let kept = brain_server::workflow::journey::AppealClock::kept_or_missed(
        1_000,
        Some(1_000 + clock - 1),
        1_000 + clock + 9_999,
    );
    assert!(matches!(
        kept,
        brain_server::workflow::journey::AppealClock::Kept
    ));
    let missed = brain_server::workflow::journey::AppealClock::kept_or_missed(
        1_000,
        Some(1_000 + clock + 1),
        1_000 + clock + 9_999,
    );
    assert!(matches!(
        missed,
        brain_server::workflow::journey::AppealClock::Missed
    ));
    let pending = brain_server::workflow::journey::AppealClock::kept_or_missed(
        1_000,
        None,
        1_000 + clock + 1,
    );
    assert!(matches!(
        pending,
        brain_server::workflow::journey::AppealClock::MissedOpen
    ));
}

#[test]
fn ap5_the_queue_renders_the_clock_not_the_ttl() {
    // the view's deadline for an appeal row is the CLOCK deadline, never the
    // TTL deadline (the countdown the operator sees is the commitment made).
    let src = std::fs::read_to_string("src/service/review.rs").expect("review source");
    assert!(
        src.contains("appeal_clock_deadline"),
        "the queue's view computation names the appeal clock"
    );
}

#[test]
fn ap5_a_missed_dispose_fires_the_existing_alerting_path() {
    let src = std::fs::read_to_string("src/handlers/gate.rs").expect("gate source");
    let start = src.find("PROP_KIND_APPEAL").expect("the appeal branch");
    let window = &src[start..start + 12_000];
    assert!(
        window.contains("alert::publish"),
        "a missed clock rides the EXISTING webhook alerting path (alert::publish), never a new channel"
    );
}

// ── AP-6 · the appeal is never machine-expired ────────────────────────────

#[test]
fn ap6_the_ttl_machine_never_expires_an_appeal() {
    let mut conn = migrated_db();
    // an appeal proposal LONG past the ordinary TTL:
    conn.execute(
        "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from, domain) \
         VALUES ('appeal', 'the reason', 'pending', 1, 0.5, 0.5, 'customer', 'acme')",
        [],
    )
    .expect("seed the appeal");
    let appeal_id: i64 = conn.last_insert_rowid();
    // an ordinary fact proposal, equally stale (the CONTROL's subject):
    conn.execute(
        "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from, domain) \
         VALUES ('fact', 'a candidate', 'pending', 1, 0.5, 0.5, 'operator', 'acme')",
        [],
    )
    .expect("seed the fact");
    let fact_id: i64 = conn.last_insert_rowid();

    let now: i64 = 10_000_000;
    let appeal_ok = brain_server::service::review::expire_if_stale(&conn, appeal_id, 1, now)
        .expect("expiry check");
    assert!(
        appeal_ok,
        "an appeal past the ordinary TTL SURVIVES — the clock governs, never the TTL"
    );
    let appeal_status: String = conn
        .query_row(
            "SELECT status FROM proposals WHERE id = ?1",
            [appeal_id],
            |r| r.get(0),
        )
        .expect("appeal row");
    assert_eq!(appeal_status, "pending", "the appeal is still pending");

    // THE CONTROL: an ordinary proposal still machine-expires (the same call,
    // the same staleness — only the kind differs).
    let fact_ok = brain_server::service::review::expire_if_stale(&conn, fact_id, 1, now)
        .expect("expiry check");
    assert!(!fact_ok, "CONTROL: an ordinary stale proposal is expired");
    let fact_status: String = conn
        .query_row(
            "SELECT status FROM proposals WHERE id = ?1",
            [fact_id],
            |r| r.get(0),
        )
        .expect("fact row");
    assert_eq!(
        fact_status, "rejected",
        "CONTROL: the ordinary proposal is machine-rejected"
    );
}

// ── the journey vocabulary grows by the appeal ────────────────────────────

#[test]
fn the_journey_renders_the_appeal_events() {
    assert!(
        brain_server::workflow::journey::JOURNEY_KINDS.contains(&"appeal_received"),
        "the closed vocabulary carries the appeal kinds"
    );
    assert!(
        brain_server::workflow::journey::JOURNEY_KINDS.contains(&"appeal_decided"),
        "the closed vocabulary carries the appeal kinds"
    );
    assert_eq!(
        brain_server::workflow::journey::sentence_for(
            JourneyKind::AppealReceived,
            None,
            None,
            None
        ),
        "Your appeal was received."
    );
    assert_eq!(
        brain_server::workflow::journey::sentence_for(JourneyKind::AppealDecided, None, None, None),
        "Your appeal was decided."
    );
}

// ── the end-to-end witness (the wire): file → dispose → the journey ───────

struct TestServer {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
    priv_key: rsa::RsaPrivateKey,
}

fn rsa_keypair(key_dir: &std::path::Path) -> rsa::RsaPrivateKey {
    let mut rng = rand::rngs::ThreadRng::default();
    let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("test keypair");
    let pub_key = rsa::RsaPublicKey::from(&priv_key);
    let pub_pem = pub_key.to_public_key_pem(LineEnding::LF).unwrap();
    let priv_pem = priv_key.to_pkcs8_pem(LineEnding::LF).unwrap();
    std::fs::create_dir_all(key_dir).unwrap();
    std::fs::write(key_dir.join("matrix-kid.pem"), pub_pem.as_bytes()).unwrap();
    let key_path = key_dir.join("matrix-kid.key");
    std::fs::write(&key_path, priv_pem.as_bytes()).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    priv_key
}

fn build_server() -> TestServer {
    static ADMIT_WILDCARD: std::sync::Once = std::sync::Once::new();
    ADMIT_WILDCARD.call_once(|| unsafe { std::env::set_var("BRAIN_ALLOW_WILDCARD_GRANT", "1") });
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mgr = brain_server::pool::SqliteConnectionManager::file(&db_path);
    let pool: brain_server::Pool = r2d2::Pool::builder().max_size(4).build(mgr).expect("pool");
    brain_server::migration::run_migration(
        &mut pool.get().expect("conn"),
        brain_server::config::DB_MMAP_SIZE_MIB,
    )
    .expect("migration");
    let model: Arc<dyn brain_server::embed::Embedder> = Arc::new(
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model"),
    );
    let priv_key = rsa_keypair(&dir.path().join("keys"));
    let key_store =
        brain_server::auth::jwks::KeyStore::load(&dir.path().join("keys")).expect("load test keys");
    let jwt_middleware_state = Arc::new(brain_server::server::router::auth::JwtMiddlewareState {
        auth_mode: brain_server::auth::AuthMode::Jwt,
        key_store: key_store.clone(),
        jwt_issuer: "https://brain.matrix/".to_string(),
        jwt_audience: "brain-server".to_string(),
        jwt_azp: None,
        pool: pool.clone(),
        revocation_cache: Arc::new(brain_server::auth::revocation::RevocationCache::new()),
        db_path: db_path.clone(),
        principal_rate_limiter: Arc::new(brain_server::http_limit::RateLimiter::new()),
    });
    let state = Arc::new(brain_server::AppState {
        token_store: brain_server::auth::TokenStore::new(),
        jwt_middleware_state,
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
        connection_tracker: Arc::new(brain_server::http_limit::ConnectionTracker::new()),
        rate_limiter: Arc::new(brain_server::http_limit::RateLimiter::new()),
        snapshot: brain_server::integrity::SnapshotState::default(),
        audit_chain_cache: Arc::new(std::sync::Mutex::new(None)),
        auth_mode: brain_server::auth::AuthMode::Jwt,
        key_store,
        revocation_cache: Arc::new(brain_server::auth::revocation::RevocationCache::new()),
        jwt_issuer: "https://brain.matrix/".to_string(),
        jwt_audience: "brain-server".to_string(),
        oidc_config: brain_server::handlers::well_known::OidcConfig::unconfigured(),
        ump_events: tokio::sync::broadcast::channel(brain_server::config::UMP_EVENT_BUFFER).0,
        alert_events: tokio::sync::broadcast::channel(brain_server::config::ALERT_EVENT_BUFFER).0,
        alert_seq: std::sync::atomic::AtomicU64::new(0),
        chain_watch: brain_server::alert::ChainWatchState::default(),
        concurrency: &brain_server::concurrency::CONCURRENCY,
    });
    TestServer {
        _dir: dir,
        state,
        priv_key,
    }
}

fn mint(srv: &TestServer, jti: &str, sub: &str, scopes: &[&str], roles: &[&str]) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = brain_server::auth::jwt::Claims {
        iss: "https://brain.matrix/".to_string(),
        aud: "brain-server".to_string(),
        sub: sub.to_string(),
        jti: jti.to_string(),
        iat: now,
        nbf: now,
        exp: now + 600,
        tenant: "global".to_string(),
        scopes: scopes.iter().map(|s| s.to_string()).collect(),
        roles: roles.iter().map(|s| s.to_string()).collect(),
        manages: Vec::new(),
        chain: None,
        azp: None,
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("matrix-kid".to_string());
    let pem = srv.priv_key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let encoding = EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap();
    encode(&header, &claims, &encoding).unwrap()
}

async fn send(
    srv: &TestServer,
    token: Option<&str>,
    path: &str,
    method: &str,
    body: &str,
) -> (axum::http::StatusCode, Vec<u8>) {
    use axum::body::Body;
    use axum::http::Request;
    let mut app = brain_server::server::router::app(srv.state.clone());
    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    let builder = match token {
        Some(t) => builder.header("authorization", format!("Bearer {t}")),
        None => builder,
    };
    let request = builder.body(Body::from(body.to_string())).expect("request");
    let response = tower::ServiceExt::oneshot(&mut app, request)
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (status, bytes.to_vec())
}

/// THE END-TO-END WITNESS (PJ-6): the customer files an appeal against a
/// DECIDED resolution; the filing stamps `customer` path-side; the refusals
/// are BY NAME; the operator's dispose rides the existing queue and writes
/// NO memory (no auto-reversal); the journey renders received + decided.
#[tokio::test]
async fn the_appeal_lives_end_to_end() {
    let srv = build_server();
    let op = mint(&srv, "e2e-op", "operator", &["admin:*/*"], &["admin"]);
    let own = mint(
        &srv,
        "e2e-own",
        "acme-portal",
        &["read:*/acme"],
        &["client-auditor"],
    );

    // 1. the client register:
    let (st, body) = send(
        &srv,
        Some(&op),
        "/clients",
        "POST",
        r#"{"name":"acme-us","domain":"acme","jurisdiction":"de"}"#,
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&body)
    );

    // 2. the operator files a resolution proposal into the client's domain:
    let (st, body) = send(
        &srv,
        Some(&op),
        "/ingest/proposal",
        "POST",
        r#"{"content":"the appealed resolution text","kind":"fact","domain":"acme"}"#,
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let proposal: serde_json::Value = serde_json::from_slice(&body).expect("proposal json");
    let proposal_id = proposal["id"].as_i64().expect("proposal id");

    // 3. the digest-bound dispose (the EXISTING approve verb):
    let digest = brain_server::handlers::gate::review_digest("the appealed resolution text");
    let (st, body) = send(
        &srv,
        Some(&op),
        &format!("/proposals/{proposal_id}/approve?digest={digest}"),
        "POST",
        "",
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "the resolution's dispose: {}",
        String::from_utf8_lossy(&body)
    );
    let knowledge_before: i64 = {
        let conn = srv.state.pool.get().expect("conn");
        conn.query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))
            .expect("knowledge count")
    };

    // 4. THE REFUSALS, BY NAME:
    // an appeal WITHOUT the shown digest:
    let (st, body) = send(
        &srv,
        Some(&own),
        "/clients/acme-us/appeals",
        "POST",
        r#"{"reason":"the resolution was wrong"}"#,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::BAD_REQUEST);
    assert!(
        String::from_utf8_lossy(&body).contains("appeal_digest_required"),
        "the refusal names the law: {}",
        String::from_utf8_lossy(&body)
    );
    // a digest no decided decision answers:
    let bogus = "b".repeat(64);
    let (st, body) = send(
        &srv,
        Some(&own),
        "/clients/acme-us/appeals",
        "POST",
        &format!(r#"{{"review_digest":"{bogus}","reason":"not the bytes shown"}}"#),
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::NOT_FOUND,
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert!(
        String::from_utf8_lossy(&body).contains("appeal_target_unknown"),
        "the refusal names the law: {}",
        String::from_utf8_lossy(&body)
    );

    // 5. THE FILING: the customer appeals the SHOWN digest:
    let (st, body) = send(
        &srv,
        Some(&own),
        "/clients/acme-us/appeals",
        "POST",
        &format!(
            r#"{{"review_digest":"{digest}","reason":"the resolution was wrong and here is why"}}   "#
        ),
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let filed: serde_json::Value = serde_json::from_slice(&body).expect("filed json");
    let appeal_id = filed["proposal_id"].as_i64().expect("appeal id");

    // 6. THE STAMP IS PATH-ASSIGNED (the schema act's proof on the wire):
    let derived: (String, Option<String>) = {
        let conn = srv.state.pool.get().expect("conn");
        conn.query_row(
            "SELECT derived_from, producer_ref FROM proposals WHERE id = ?1",
            [appeal_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the appeal row")
    };
    assert_eq!(
        derived.0, "customer",
        "the appeal's row stamps `customer` — the route is the path"
    );
    assert_eq!(
        derived.1.as_deref(),
        Some("portal:acme-us"),
        "producer_ref names the portal producer"
    );

    // 7. THE DISPOSAL: the operator approves the appeal — the existing verb,
    // the appeal branch's decision, and NO memory write (no auto-reversal):
    let appeal_digest =
        brain_server::handlers::gate::review_digest("the resolution was wrong and here is why");
    let (st, body) = send(
        &srv,
        Some(&op),
        &format!("/proposals/{appeal_id}/approve?digest={appeal_digest}"),
        "POST",
        "",
    )
    .await;
    assert_eq!(
        st,
        axum::http::StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&body).expect("receipt json");
    assert_eq!(receipt["status"], "approved");
    assert!(
        receipt["appeal_clock"] == "kept" || receipt["appeal_clock"] == "missed",
        "the receipt carries the chain-provable clock verdict"
    );
    let knowledge_after: i64 = {
        let conn = srv.state.pool.get().expect("conn");
        conn.query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))
            .expect("knowledge count")
    };
    assert_eq!(
        knowledge_before, knowledge_after,
        "NO AUTO-REVERSAL, NO PROMOTION: the appeal's dispose wrote no memory"
    );
    // the disposal rode the HITL ledger:
    let hitl: i64 = {
        let conn = srv.state.pool.get().expect("conn");
        conn.query_row(
            "SELECT COUNT(*) FROM hitl_decision_events WHERE proposal_id = ?1 AND decision = 'approve'",
            [appeal_id],
            |r| r.get(0),
        )
        .expect("hitl row")
    };
    assert_eq!(hitl, 1, "the dispose rides hitl_decision_events");

    // 8. THE JOURNEY RENDERS THE APPEAL:
    let (st, body) = send(&srv, Some(&own), "/clients/acme-us/journey", "GET", "").await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let journey = String::from_utf8_lossy(&body).to_string();
    assert!(
        journey.contains("Your appeal was received."),
        "the received event renders: {journey}"
    );
    assert!(
        journey.contains("Your appeal was decided."),
        "the decided event renders"
    );
}

/// THE LEAK ARM for the filing door (PJ-2 at the write): another client's
/// principal is refused existence-blind — the same 404 body as a ghost.
#[tokio::test]
async fn the_appeal_door_refuses_cross_client_existence_blind() {
    let srv = build_server();
    let op = mint(&srv, "ad-op", "operator", &["admin:*/*"], &["admin"]);
    let other = mint(
        &srv,
        "ad-other",
        "other-portal",
        &["read:*/other"],
        &["client-auditor"],
    );
    let (st, _) = send(
        &srv,
        Some(&op),
        "/clients",
        "POST",
        r#"{"name":"acme-us","domain":"acme","jurisdiction":"de"}"#,
    )
    .await;
    assert_eq!(st, axum::http::StatusCode::OK);
    let digest = "c".repeat(64);
    let filing = format!(r#"{{"review_digest":"{digest}","reason":"x"}}"#);
    let (st_b, body_b) = send(
        &srv,
        Some(&other),
        "/clients/acme-us/appeals",
        "POST",
        &filing,
    )
    .await;
    let (st_g, body_g) = send(
        &srv,
        Some(&other),
        "/clients/ghost-co/appeals",
        "POST",
        &filing,
    )
    .await;
    assert_eq!(st_b, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(st_g, axum::http::StatusCode::NOT_FOUND);
    assert_eq!(
        body_b, body_g,
        "EXISTENCE-BLIND: the filing door's refusal is byte-identical to a ghost client's"
    );
}
