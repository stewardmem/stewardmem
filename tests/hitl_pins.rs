//! The HITL-telemetry pins (IMPROVE11): the approver is measured the way the
//! artifact always was — one append-only row per act, written in the act's own
//! transaction, reported beside κ and NEVER allowed to answer back.
//!
//! RED LEDGER (2026-11-09, against `60704289` before this round's code):
//!   1. absence — `hitl_decision_events` does not exist (the gap this round
//!      closes; tree-verified: `grep -c hitl_decision_events src/migration.rs`
//!      = 0). The accounting tests red with "no such table" while the ROUTE
//!      still succeeds — the decision was real, its measurement was not.
//!   2. lockstep — the ceiling sits at 1.32.32 with the probe above it; every
//!      `ht4` pin reds on the unmoved set (the drift-guard that caught
//!      plan 08's landing).
//!   3. inventory — the rehearsal binary's table inventory does not carry the
//!      ledger; `ht4_rehearsal_inventory_carries_the_ledger` reds.
//!
//! The laws pinned here (plan 11 §3 + the execution prompt's gating laws):
//!   HT-1  every approve/reject/edit emits exactly ONE decision-event row,
//!         written in the decision's own tx (the plan-08 CHECKED accounting
//!         pattern); a failing event write fails the DECISION, never the
//!         reverse. Latency is (decided_at − created_at) at the clocks'
//!         second granularity, expressed in ms; edit depth is the signed char
//!         delta; the interval is the gap to the actor's previous event.
//!   HT-4  the ceiling, the stamp, the probe and the rehearsal inventory move
//!         TOGETHER (a ceiling moved alone reds).
//!   The append-only law: nothing in the tree UPDATEs, DELETEs from, or DROPs
//!   the ledger — the only writes in the tree are the recorder's INSERTs.
//!   (HT-2, the inalienability scan, and HT-3, the report pin, land with S3
//!   and S2 respectively; this file grows per step and the census re-derives
//!   in-commit.)
//!
//! TTL exclusion, named: `expire_if_stale` rejects an aged-out proposal with
//! its own UPDATE (`src/service/review.rs`) — the MACHINE's act, audited as
//! `proposal_expired`, carrying no HITL actor. It deliberately records no
//! decision-event row and the accounting checker below deliberately does not
//! count it: this ledger measures the approver, not the timer.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use rusqlite::Connection;
use tower::ServiceExt;

/// The opaque operator token: with no JWT principal in extensions both
/// `authorize` and `authorize_role(.., "approve"/"reject")` return `Ok(())`
/// (`src/handlers/mod.rs`), so every act here is entered as the superuser.
/// `principal_label` resolves the absent principal to the operator loopback
/// label — the actor every row in this file records.
const OP_TOKEN: &str = "i11-hitl-op-token";

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
        revocation_cache: Arc::new(brain_server::auth::RevocationCache::new()),
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

/// Seed a pending proposal directly. `created_at = now` so the TTL check
/// passes and the act reaches the decision (an aged-out row would answer
/// `400 proposal_expired` — the machine's act, never this ledger's).
fn seed_proposal(conn: &Connection, content: &str, created_at: i64) -> i64 {
    conn.execute(
        "INSERT INTO proposals(kind, content, source, novelty, salience, created_at, domain, owner)
         VALUES ('fact', ?1, 'agent', 0.9, 0.5, ?2, 'global', 'proposer@acme')",
        rusqlite::params![content, created_at],
    )
    .expect("seed proposal");
    conn.last_insert_rowid()
}

/// The digest an approve must carry, read from the STORED content (the handler
/// digests what the row holds — a missing digest is a protocol violation).
fn digest_for(state: &Arc<brain_server::AppState>, pid: i64) -> String {
    let conn = state.pool.get().expect("conn");
    let content: String = conn
        .query_row("SELECT content FROM proposals WHERE id = ?1", [pid], |r| {
            r.get(0)
        })
        .expect("stored content");
    brain_server::handlers::gate::review_digest(&content)
}

/// POST any route with the operator bearer and read (status, JSON) back.
async fn post_json(
    state: &Arc<brain_server::AppState>,
    uri: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let res = app(state.clone()).oneshot(req).await.expect("oneshot");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn approve(state: &Arc<brain_server::AppState>, pid: i64) -> (StatusCode, serde_json::Value) {
    let d = digest_for(state, pid);
    post_json(state, &format!("/proposals/{pid}/approve?digest={d}"), "{}").await
}

async fn reject(state: &Arc<brain_server::AppState>, pid: i64) -> (StatusCode, serde_json::Value) {
    post_json(state, &format!("/proposals/{pid}/reject"), "{}").await
}

/// GET any route with the operator bearer and read (status, JSON) back.
async fn get(state: &Arc<brain_server::AppState>, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::empty())
        .expect("request");
    let res = app(state.clone()).oneshot(req).await.expect("oneshot");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn edit(
    state: &Arc<brain_server::AppState>,
    pid: i64,
    content: &str,
) -> (StatusCode, serde_json::Value) {
    post_json(
        state,
        &format!("/proposals/{pid}/edit"),
        &serde_json::json!({ "content": content }).to_string(),
    )
    .await
}

/// Every event row for one proposal, oldest first.
/// One recorded act: (decision, latency_ms, edit_depth_chars, interval_ms) —
/// the NULL pattern is the act's fingerprint (latency NULL on edit, depth NULL
/// off edit, interval NULL on the actor's first act).
type EventRow = (String, Option<i64>, Option<i64>, Option<i64>);

/// Every event row for one proposal, oldest first.
fn event_rows(conn: &Connection, pid: i64) -> Vec<EventRow> {
    let mut stmt = conn
        .prepare(
            "SELECT decision, latency_ms, edit_depth_chars, interval_ms
             FROM hitl_decision_events WHERE proposal_id = ?1 ORDER BY id",
        )
        .expect("event query");
    let rows = stmt
        .query_map([pid], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })
        .expect("event map");
    rows.collect::<Result<Vec<_>, _>>().expect("event rows")
}

/// The HT-1 accounting checker: every proposal DECIDED by a human arm
/// (approved | rejected) must carry exactly one matching decision-event row.
/// TTL-expired rows are the machine's act (see the module header) and are
/// deliberately out of scope: they are rejected with no HITL actor, audited
/// as `proposal_expired`.
fn decided_proposals_without_event_rows(conn: &Connection) -> Vec<(i64, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.status FROM proposals p
             WHERE p.status IN ('approved','rejected')
               AND (SELECT COUNT(*) FROM hitl_decision_events e
                    WHERE e.proposal_id = p.id
                      AND e.decision = CASE p.status
                            WHEN 'approved' THEN 'approve' ELSE 'reject' END) <> 1
             ORDER BY p.id",
        )
        .expect("checker query");
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .expect("checker map");
    rows.collect::<Result<Vec<_>, _>>().expect("checker rows")
}

// ── HT-1 · the accounting: one row per act, in the act's own tx ────────────

/// The approve act records exactly one event row with the derived telemetry:
/// latency = (decided_at − created_at) in ms, no edit depth, no interval
/// (the actor's first recorded act), the row's own domain residency.
#[tokio::test]
async fn ht1_approve_emits_exactly_one_event_row() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "the approver measured one approval", now)
    };
    let (status, body) = approve(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK, "approve must succeed: {body}");
    assert_eq!(body["status"], "approved", "the act must be a decision");

    let conn = srv.state.pool.get().expect("conn");
    let rows = event_rows(&conn, pid);
    assert_eq!(rows.len(), 1, "exactly one event row per act, got {rows:?}");
    let (decision, latency, depth, interval) = &rows[0];
    assert_eq!(decision, "approve");
    assert!(latency.is_some(), "approve carries its latency");
    assert!(depth.is_none(), "edit depth is NULL off edit");
    assert!(interval.is_none(), "first act has no prior interval");
    // The accounting is exact: latency derives from the SAME clocks the
    // decision stamped — (decided_at − created_at) × 1000, no drift.
    let exact: bool = conn
        .query_row(
            "SELECT e.latency_ms = (p.decided_at - p.created_at) * 1000
             FROM hitl_decision_events e JOIN proposals p ON p.id = e.proposal_id
             WHERE e.proposal_id = ?1 AND e.decision = 'approve'",
            [pid],
            |r| r.get(0),
        )
        .expect("latency join");
    assert!(
        exact,
        "latency_ms must equal (decided_at − created_at) × 1000"
    );
    let (actor, domain): (String, String) = conn
        .query_row(
            "SELECT actor, domain FROM hitl_decision_events WHERE proposal_id = ?1",
            [pid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("actor/domain");
    assert!(!actor.is_empty(), "the act names its actor");
    assert_eq!(domain, "global", "the act carries the row's residency");
    // The write rode the decision's tx: the accounting checker is clean.
    assert!(
        decided_proposals_without_event_rows(&conn).is_empty(),
        "every decided proposal carries its row"
    );
}

/// The reject act records exactly one event row, same shape minus depth.
#[tokio::test]
async fn ht1_reject_emits_exactly_one_event_row() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "the approver measured one refusal", now)
    };
    let (status, _) = reject(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK, "reject must succeed");

    let conn = srv.state.pool.get().expect("conn");
    let rows = event_rows(&conn, pid);
    assert_eq!(rows.len(), 1, "exactly one event row per act, got {rows:?}");
    let (decision, latency, depth, interval) = &rows[0];
    assert_eq!(decision, "reject");
    assert!(latency.is_some(), "reject carries its latency");
    assert!(depth.is_none(), "edit depth is NULL off edit");
    assert!(interval.is_none(), "first act has no prior interval");
    assert!(
        decided_proposals_without_event_rows(&conn).is_empty(),
        "every decided proposal carries its row"
    );
}

/// The edit act records exactly one event row with the SIGNED char delta of
/// the edit — computed from the before-content the handler holds in hand —
/// and NO latency (an edit is not a decision; the proposal stays pending).
#[tokio::test]
async fn ht1_edit_emits_exactly_one_event_row_with_depth() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "alpha", now)
    };
    let (status, body) = edit(&srv.state, pid, "alpha beta gamma").await;
    assert_eq!(status, StatusCode::OK, "edit must succeed: {body}");

    let conn = srv.state.pool.get().expect("conn");
    let rows = event_rows(&conn, pid);
    assert_eq!(rows.len(), 1, "exactly one event row per act, got {rows:?}");
    let (decision, latency, depth, interval) = &rows[0];
    assert_eq!(decision, "edit");
    assert!(
        latency.is_none(),
        "latency is NULL on edit — not a decision"
    );
    // "alpha beta gamma" = 16 chars, "alpha" = 5 → the signed delta is +11.
    assert_eq!(
        *depth,
        Some(11),
        "edit_depth_chars is the signed char delta of the edit"
    );
    assert!(interval.is_none(), "first act has no prior interval");
    let proposal_status: String = conn
        .query_row("SELECT status FROM proposals WHERE id = ?1", [pid], |r| {
            r.get(0)
        })
        .expect("status");
    assert_eq!(proposal_status, "pending", "an edit is not a decision");
}

/// Edit then approve is TWO rows — one per act — and the approve's interval
/// reaches back to the edit (the actor's previous recorded event). Latency
/// rides the approve only.
#[tokio::test]
async fn ht1_edit_then_approve_is_two_rows_one_per_act() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "draft one", now)
    };
    let (edit_status, _) = edit(&srv.state, pid, "draft one, revised").await;
    assert_eq!(edit_status, StatusCode::OK, "edit must succeed");
    let (status, body) = approve(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK, "approve must succeed: {body}");

    let conn = srv.state.pool.get().expect("conn");
    let rows = event_rows(&conn, pid);
    assert_eq!(rows.len(), 2, "one row per act, got {rows:?}");
    assert_eq!(rows[0].0, "edit", "the edit act first");
    assert_eq!(rows[1].0, "approve", "the approve act second");
    assert!(rows[0].1.is_none(), "no latency on the edit row");
    assert!(rows[1].1.is_some(), "latency on the approve row");
    assert!(
        rows[1].3.is_some(),
        "the approve's interval reaches back to the actor's edit"
    );
    assert!(
        decided_proposals_without_event_rows(&conn).is_empty(),
        "every decided proposal carries its row"
    );
}

/// The burst shape: two approvals by the same actor in quick succession —
/// the first has no interval, the second's interval reaches the first and is
/// non-negative (time does not run backwards).
#[tokio::test]
async fn ht1_interval_reaches_the_actors_previous_event() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let (p1, p2) = {
        let conn = srv.state.pool.get().expect("conn");
        (
            seed_proposal(&conn, "burst approval one", now),
            seed_proposal(&conn, "burst approval two", now),
        )
    };
    let (s1, _) = approve(&srv.state, p1).await;
    let (s2, _) = approve(&srv.state, p2).await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(s2, StatusCode::OK);

    let conn = srv.state.pool.get().expect("conn");
    let first = event_rows(&conn, p1);
    let second = event_rows(&conn, p2);
    assert!(first[0].3.is_none(), "first act: no prior interval");
    assert!(
        second[0].3.is_some() && second[0].3.unwrap() >= 0,
        "second act: non-negative interval to the actor's previous event"
    );
}

/// The in-test arm of the mutation proof: the accounting checker can FAIL,
/// and when it fails it NAMES the act. A stripped write is simulated by
/// deleting the row the (real, committed) approve just wrote — exactly the
/// state the tree would compile to with the recorder's INSERT absent.
#[tokio::test]
async fn control_ht1_the_accounting_reds_on_a_stripped_write_and_names_the_act() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "the checker must be able to fail", now)
    };
    let (status, _) = approve(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK);
    let conn = srv.state.pool.get().expect("conn");
    assert!(decided_proposals_without_event_rows(&conn).is_empty());
    conn.execute(
        "DELETE FROM hitl_decision_events WHERE proposal_id = ?1",
        [pid],
    )
    .expect("simulate the stripped INSERT");
    let violations = decided_proposals_without_event_rows(&conn);
    assert_eq!(
        violations,
        vec![(pid, "approved".to_string())],
        "the checker reds and names the decided proposal whose act went unrecorded"
    );
}

// ── HT-4 · the lockstep: schema const, stamp, probe, inventory move TOGETHER ──

/// The migrated stamp and the moved schema const agree on 1.32.33 — this
/// round's additive migration, not a renumbered neighbor. The equality
/// between the stamp literal and the const is separately pinned by
/// `latest_stamp_matches_migration` (storage_layout.rs) and the moved
/// main_suite/per_domain pins; this pin holds the VALUES themselves so a
/// renumber that loses the ledger's version reads as the red it is.
#[test]
fn ht4_schema_stamp_and_version_const_agree() {
    let srv = server();
    let conn = srv.state.pool.get().expect("conn");
    assert_eq!(
        brain_server::storage_layout::schema_version(&conn).as_deref(),
        Some("1.32.33"),
        "the migrated stamp is 1.32.33 — the additive hitl_decision_events migration"
    );
    assert_eq!(
        brain_server::storage_layout::LATEST_KNOWN_SCHEMA,
        "1.32.33",
        "the schema const must move WITH the stamp — refuse_newer must not bless a DB \
         this binary cannot migrate, nor refuse its own"
    );
}

/// The repo-root path for the structural reads (census shape (b): the read is
/// one call deep, in this helper the tests call by name).
fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The refuse-newer probe sits ABOVE the moved stamp by construction: it
/// asserts 1.32.34 is newer and the moved schema const 1.32.33 is not. A
/// probe pinned AT the stamp silently tests `Equal`, not `Greater` — the one
/// failure `refuse_newer` exists to prevent. The stamp lives in migration.rs.
#[test]
fn ht4_probe_moves_one_above_and_the_stamp_matches() {
    let layout = read_repo("src/storage_layout.rs");
    let migration = read_repo("src/migration.rs");
    assert!(
        layout.contains(r#"assert!(is_newer_than_known(Some("1.32.34")));"#),
        "the probe must move ABOVE the new ceiling — 1.32.34 is newer than 1.32.33"
    );
    assert!(
        layout.contains(r#"assert!(!is_newer_than_known(Some("1.32.33")));"#),
        "the ceiling itself (1.32.33) must read as not-newer"
    );
    assert!(
        !layout.contains(r#"assert!(is_newer_than_known(Some("1.32.33")));"#),
        "a probe pinned AT the ceiling tests Equal, not Greater — move it to 1.32.34"
    );
    assert!(
        migration.contains("'schema_version', '1.32.33'"),
        "run_migration must stamp 1.32.33 — the stamp and the ceiling move together"
    );
}

/// The rehearsal binary's table inventory carries the ledger — the
/// drift-guard that caught plan 08: a rehearsal that dropped the table would
/// bless a DB that has forgotten every approval act it ever recorded.
#[test]
fn ht4_rehearsal_inventory_carries_the_ledger() {
    let rehearsal = read_repo("src/bin/brain_migrate_rehearse.rs");
    assert!(
        rehearsal.contains(r#""hitl_decision_events","#),
        "the rehearsal inventory must carry hitl_decision_events — row-count parity \
         is what proves a rehearsal copied the approver-measurement layer"
    );
}

/// The ledger's shape is pinned at the storage layer: the nine columns in
/// order, and the CLOSED decision vocabulary enforced by CHECK — a decision
/// outside {approve, reject, edit} is not a row, ever.
#[test]
fn ht4_ledger_columns_and_the_closed_vocabulary() {
    let srv = server();
    let conn = srv.state.pool.get().expect("conn");
    let mut stmt = conn
        .prepare("PRAGMA table_info(hitl_decision_events)")
        .expect("pragma");
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .expect("pragma map")
        .collect::<Result<Vec<_>, _>>()
        .expect("pragma rows");
    assert_eq!(
        cols,
        vec![
            "id",
            "proposal_id",
            "decision",
            "actor",
            "latency_ms",
            "edit_depth_chars",
            "interval_ms",
            "ts",
            "domain",
        ],
        "the ledger's nine columns, in order"
    );
    let refused = conn.execute(
        "INSERT INTO hitl_decision_events
             (proposal_id, decision, actor, latency_ms, edit_depth_chars, interval_ms, ts, domain)
         VALUES (1, 'gate', 'someone', NULL, NULL, NULL, 0, 'global')",
        [],
    );
    assert!(
        refused.is_err(),
        "the CHECK law: a decision outside the closed vocabulary must be refused \
         at the storage layer, not merely by the recorder's discipline"
    );
}

// ── HT-3 · the report and the board carry the measurement ──────────────────
// (S2 pins, written before the S2 landing: the present-arms red by absence —
// the report's audited detail has no `hitl:` suffix yet, the scoreboard body
// has no `hitl` key yet — and the absent-arms green today, pinning the
// honesty arm that must SURVIVE the landing.)

/// The weekly report's audited detail carries the `hitl:` segment when
/// decisions exist. Details are HASHED at rest, so the pin recomputes the
/// expected detail — the SDK record's format plus the segment derived from
/// the ledger's own row (the latency read back from the act's event, not
/// assumed) — and matches the stored hash. This is the report the operator
/// signs beside κ: measurement-first, never gating.
#[tokio::test]
async fn ht3_report_carries_the_hitl_segment_when_decisions_exist() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "the report must carry the measurement", now)
    };
    let (status, _) = approve(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK);

    let conn = srv.state.pool.get().expect("conn");
    let latency: i64 = conn
        .query_row(
            "SELECT latency_ms FROM hitl_decision_events
             WHERE proposal_id = ?1 AND decision = 'approve'",
            [pid],
            |r| r.get(0),
        )
        .expect("the act's row carries its latency");
    brain_server::workflow::calibration::record_report(&conn, 9000, now, "").expect("report");
    let expected = format!(
        "{} hitl:approved:1 rejected:0 edited:0 median_latency_ms:{} \
         burst_max_acts_per_min:1 edit_depth_mean_chars:null decay:agent:0/1",
        brain_engine_sdk::calibration::CalibrationRecord::new(
            brain_engine_sdk::calibration::NO_KAPPA,
            0,
            ""
        )
        .detail(),
        latency
    );
    let detail_hash: String = conn
        .query_row(
            "SELECT detail_hash FROM audit_events
             WHERE kind='workflow' AND target_hash = ?1",
            [brain_server::audit::hash("calibration/report")],
            |r| r.get(0),
        )
        .expect("the report's audit row");
    assert_eq!(
        detail_hash,
        brain_server::audit::hash(&expected),
        "the audited report must carry the hitl segment beside κ — the segment \
         omitted reds here, the segment drifting from the derivation reds here"
    );
}

/// The absent-honest arm: with NO decisions, the report's detail is the
/// record's own format EXACTLY — no empty `hitl:` suffix dressed up as data.
/// Greens today (the current state IS absence) and must stay green after the
/// landing: the segment appears only when decisions exist.
#[test]
fn ht3_report_is_absent_honest_when_no_decisions_exist() {
    let conn = {
        brain_server::register_sqlite_vec::register_sqlite_vec();
        let mut c = rusqlite::Connection::open_in_memory().expect("memory db");
        brain_server::migration::run_migration(&mut c, 1).expect("migration");
        c
    };
    brain_server::workflow::calibration::record_report(&conn, 9000, 100, "").expect("report");
    let expected = brain_engine_sdk::calibration::CalibrationRecord::new(
        brain_engine_sdk::calibration::NO_KAPPA,
        0,
        "",
    )
    .detail();
    let detail_hash: String = conn
        .query_row(
            "SELECT detail_hash FROM audit_events
             WHERE kind='workflow' AND target_hash = ?1",
            [brain_server::audit::hash("calibration/report")],
            |r| r.get(0),
        )
        .expect("the report's audit row");
    assert_eq!(
        detail_hash,
        brain_server::audit::hash(&expected),
        "an empty ledger must leave the report's detail byte-stable — no \
         zero-cohort hitl segment dressed up as measurement"
    );
}

/// The DPO-gated scoreboard serves the hitl block with the derived numbers —
/// the report and the board measure the same ledger. An edit-then-approve
/// sequence shows both acts: edited 1, approved 1, a non-null median over the
/// decided act, and the per-source decay-vs-decision ratio.
#[tokio::test]
async fn ht3_scoreboard_serves_the_hitl_block() {
    let srv = server();
    let now = chrono::Utc::now().timestamp();
    let pid = {
        let conn = srv.state.pool.get().expect("conn");
        seed_proposal(&conn, "the board must serve the measurement", now)
    };
    let (es, _) = edit(
        &srv.state,
        pid,
        "the board must serve the measurement, edited",
    )
    .await;
    assert_eq!(es, StatusCode::OK, "edit must succeed");
    let (status, body) = approve(&srv.state, pid).await;
    assert_eq!(status, StatusCode::OK, "approve must succeed: {body}");

    let (st, board) = get(&srv.state, "/workflow/scoreboard").await;
    assert_eq!(
        st,
        StatusCode::OK,
        "the DPO-gated board must serve: {board}"
    );
    let hitl = &board["hitl"];
    assert!(
        !hitl.is_null(),
        "the scoreboard must carry a hitl block (the DPO-gated surface the \
         block rides — never a public route)"
    );
    assert_eq!(hitl["approved"], 1, "one approve act");
    assert_eq!(hitl["edited"], 1, "one edit act");
    assert_eq!(hitl["rejected"], 0);
    assert!(
        hitl["median_latency_ms"].is_i64(),
        "the decided act's latency is non-null: {}",
        hitl["median_latency_ms"]
    );
    assert!(
        hitl["burst_max_acts_per_min"].is_i64(),
        "the burst measure serves even for a single act"
    );
    let decay = hitl["decay_vs_decision_by_source"]
        .as_array()
        .expect("decay array");
    assert_eq!(decay.len(), 1, "one source observed");
    assert_eq!(decay[0]["source"], "agent");
    assert_eq!(
        decay[0]["machine_expired"], 0,
        "no TTL expiry in this witness"
    );
    assert_eq!(decay[0]["human_decided"], 1, "one human decision");
}

/// The zero-cohort arm: with NO decisions the block still serves — zeros and
/// nulls, never dressed up as perfection (the aftersales absence law).
#[tokio::test]
async fn ht3_scoreboard_zero_cohort_is_absent_honest() {
    let srv = server();
    let (st, board) = get(&srv.state, "/workflow/scoreboard").await;
    assert_eq!(st, StatusCode::OK, "{board}");
    let hitl = &board["hitl"];
    assert!(!hitl.is_null(), "the block serves even when empty");
    assert_eq!(hitl["approved"], 0);
    assert_eq!(hitl["edited"], 0);
    assert_eq!(hitl["rejected"], 0);
    assert!(
        hitl["median_latency_ms"].is_null(),
        "no decisions → no median, null is the honest answer"
    );
    assert_eq!(
        hitl["decay_vs_decision_by_source"]
            .as_array()
            .expect("decay array")
            .len(),
        0,
        "no proposals → no decay ratios"
    );
}
