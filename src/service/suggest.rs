//! The suggest-feedback aggregate: the last-wins feedback ledger and its
//! outcome metrics.
//!
//! OWNS the `suggest_feedback` storage story: the chunk-existence fence (a
//! never-existed id is refused so the metric isn't poisoned by typos; a
//! DELETED chunk's id still counts — the feedback was real when given), the
//! last-wins upsert (conflict key `(chunk_id, COALESCE(session, ''))`;
//! session + tenant_id are identity and never updated), and the grouped
//! outcome counts behind the false-positive metric.
//!
//! NO audit_events row — by contract: the feedback table IS the audit
//! surface (append-only, hash-of-reason, tenant-scoped). Do not "fix" that
//! during a move.
//!
//! Read seam + metric arithmetic stay handler-side: this module returns the
//! raw (feedback, count) pairs; the wire response is shaped at the handler.

use std::fmt;

use rusqlite::{Connection, params};

/// One grouped outcome count: (feedback, n).
pub(crate) type FeedbackCount = (String, i64);

/// A storage failure. `Database`'s Display carries the exact pre-move
/// message; the handler wraps it in `HandlerError::internal` unchanged.
/// `NoSuchChunk` maps to the route's frozen probe-blind 404.
#[derive(Debug)]
pub(crate) enum FeedbackError {
    Database(String),
    NoSuchChunk(i64),
}

impl fmt::Display for FeedbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeedbackError::Database(m) => f.write_str(m),
            FeedbackError::NoSuchChunk(id) => write!(f, "no chunk with id {id}"),
        }
    }
}

impl From<rusqlite::Error> for FeedbackError {
    fn from(e: rusqlite::Error) -> Self {
        FeedbackError::Database(e.to_string())
    }
}

/// The suggest-feedback last-wins upsert, shared by the `/suggest/feedback`
/// handler and the `/ump/feedback` binding. The optional `ump_outcome`
/// carries the granular UMP outcome (`followed`/`overridden`/`ignored`/
/// `contradicted`) in its own column; the suggest path passes `None`
/// (column stays NULL).
///
/// The optional `owner` is the JWT principal (`sub`) that gave the feedback —
/// the erasure join evidence. Session ids are client-owned opaque
/// labels and can never substitute for it: without the owner, a certified
/// purge/DSAR could not reach the subject's feedback rows on chunks the
/// purge never touched. No principal (opaque/loopback callers) stores NULL —
/// those rows stay reachable only through the tenant + chunk arms (the
/// disclosed ceiling).
///
/// The existence check's fail-open read posture is preserved verbatim: a
/// QUERY ERROR on the EXISTS probe reads as "no chunk" (a 404), not a 500 —
/// the same `.unwrap_or(false)` the handler had.
#[allow(clippy::too_many_arguments)] // 9 positional params, 2 call sites; a struct would be ceremony for a private fn
pub(crate) fn record_feedback(
    conn: &mut Connection,
    chunk_id: i64,
    feedback: &str,
    reason_hash: Option<String>,
    ts: i64,
    session: Option<String>,
    tenant: &str,
    ump_outcome: Option<&str>,
    owner: Option<&str>,
) -> Result<(), FeedbackError> {
    // chunk_id validity: refuse feedback on a non-existent chunk so the
    // metric isn't poisoned by typos. (A deleted chunk's id still counts —
    // the feedback was real when given; only never-existed ids are rejected.)
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM knowledge WHERE id = ?1)",
            params![chunk_id],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n != 0)
        .unwrap_or(false);
    if !exists {
        return Err(FeedbackError::NoSuchChunk(chunk_id));
    }
    // One transaction: the upsert and the contradicted consumer's freshness
    // pull commit or roll back together — a CAS failure fails the whole
    // feedback act, never a silent half-consumer. BEGIN IMMEDIATE per the
    // write-discipline law (a read-modify-write transition, not a deferred
    // seam).
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| FeedbackError::Database(format!("feedback tx begin failed: {e}")))?;
    tx.execute(
        "INSERT INTO suggest_feedback(chunk_id, feedback, reason_hash, ts, session, tenant_id, ump_outcome, owner)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(chunk_id, COALESCE(session, '')) DO UPDATE SET
           feedback = excluded.feedback,
           reason_hash = excluded.reason_hash,
           ts = excluded.ts,
           ump_outcome = excluded.ump_outcome,
           owner = excluded.owner",
        params![chunk_id, feedback, reason_hash, ts, session, tenant, ump_outcome, owner],
    )
    .map_err(|e| FeedbackError::Database(format!("feedback insert failed: {e}")))?;
    // The feedback consumer (blind-spot 8): ONLY the granular
    // `"contradicted"` outcome touches memory state — the `/suggest/feedback`
    // surface passes `ump_outcome = None` and never fires. The record gate
    // (`row_access_meta`/`authorize_target_row`) ran BEFORE this in the
    // handler, so a foreign private row cannot be freshness-touched.
    if ump_outcome == Some("contradicted") {
        crate::service::gate::freshness_pull_earlier(&tx, chunk_id, ts)
            .map_err(|e| FeedbackError::Database(format!("freshness pull failed: {e}")))?;
    }
    tx.commit()
        .map_err(|e| FeedbackError::Database(format!("feedback commit failed: {e}")))?;
    Ok(())
}

/// The ledger's grouped outcome counts for one tenant, optionally narrowed
/// by a `since` timestamp and a session label — the (tenant_id, ts) index
/// keeps it cheap. Raw pairs; the handler shapes the rates.
pub(crate) fn feedback_counts(
    conn: &Connection,
    tenant: &str,
    since: Option<&str>,
    session: Option<&str>,
) -> Result<Vec<FeedbackCount>, FeedbackError> {
    let mut sql = String::from(
        "SELECT feedback, COUNT(*) FROM suggest_feedback \
         WHERE tenant_id = ?1",
    );
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(tenant.to_string())];
    if let Some(since) = since {
        sql.push_str(" AND ts >= ?");
        params_vec.push(Box::new(since.to_string()));
    }
    if let Some(sess) = session {
        sql.push_str(" AND session = ?");
        params_vec.push(Box::new(sess.to_string()));
    }
    sql.push_str(" GROUP BY feedback");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| FeedbackError::Database(format!("metrics prepare failed: {e}")))?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| FeedbackError::Database(format!("metrics query failed: {e}")))?;
    let mut out = Vec::new();
    for row in rows.flatten() {
        out.push(row);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::run_migration;
    use crate::register_sqlite_vec::register_sqlite_vec;

    fn seed() -> Connection {
        register_sqlite_vec();
        let mut conn = Connection::open_in_memory().unwrap();
        run_migration(&mut conn, 1).unwrap();
        conn.execute(
            "INSERT INTO knowledge (content, content_hash, node_kind) \
             VALUES ('suggested memory', 'h-own', 'fact')",
            [],
        )
        .unwrap();
        conn
    }

    /// The erasure join evidence exists: the feedback upsert stores the JWT
    /// principal (`sub`) in `suggest_feedback.owner`, so a certified
    /// purge/DSAR can reach the subject's rows even when the session label
    /// (client-owned, never a principal id) is the only other key. The
    /// no-principal call stores NULL — the disclosed ceiling.
    #[test]
    fn feedback_owner_captured_from_principal() {
        let mut conn = seed();
        let chunk_id: i64 = conn
            .query_row("SELECT id FROM knowledge", [], |r| r.get(0))
            .unwrap();
        record_feedback(
            &mut conn,
            chunk_id,
            "dismiss",
            None,
            1,
            Some("session-7".into()),
            "default",
            None,
            Some("jane"),
        )
        .unwrap();
        let owner: Option<String> = conn
            .query_row(
                "SELECT owner FROM suggest_feedback WHERE chunk_id = ?1",
                params![chunk_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            owner.as_deref(),
            Some("jane"),
            "the principal evidence must ride the feedback row"
        );
        // Last-wins replay from a different principal REPLACES the owner
        // (the row's evidence reflects who spoke last).
        record_feedback(
            &mut conn,
            chunk_id,
            "accept",
            None,
            2,
            Some("session-7".into()),
            "default",
            None,
            None,
        )
        .unwrap();
        let owner: Option<String> = conn
            .query_row(
                "SELECT owner FROM suggest_feedback WHERE chunk_id = ?1",
                params![chunk_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(owner, None, "an opaque replay overwrites the evidence");
    }
}

#[cfg(test)]
mod feedback_consumer_tests {
    use super::*;
    use crate::migration::run_migration;
    use crate::register_sqlite_vec::register_sqlite_vec;

    fn seed() -> Connection {
        register_sqlite_vec();
        let mut conn = Connection::open_in_memory().unwrap();
        run_migration(&mut conn, 1).unwrap();
        conn.execute(
            "INSERT INTO knowledge (content, content_hash, node_kind) \
             VALUES ('consumer memory', 'h-consumer', 'fact')",
            [],
        )
        .unwrap();
        conn
    }

    fn chunk_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM knowledge", [], |r| r.get(0))
            .unwrap()
    }

    fn due(conn: &Connection, id: i64) -> Option<i64> {
        conn.query_row(
            "SELECT freshness_review_due FROM knowledge WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// F-1: contradicted feedback on a known chunk pulls
    /// `freshness_review_due` to `ts`; the row surfaces in
    /// `article_worklist` output (the widened predicate admits a
    /// freshness-due `none` row).
    #[test]
    fn f1_contradicted_pulls_freshness_and_surfaces_in_the_worklist() {
        let mut conn = seed();
        let id = chunk_id(&conn);
        assert_eq!(due(&conn, id), None, "precondition: no due date");
        record_feedback(
            &mut conn,
            id,
            "dismiss",
            None,
            1_700_000_000,
            Some("s1".into()),
            "default",
            Some("contradicted"),
            None,
        )
        .unwrap();
        assert_eq!(
            due(&conn, id),
            Some(1_700_000_000),
            "contradicted feedback must pull the review to its ts"
        );
        let rows = crate::workflow::kcs::article_worklist(&conn, None, None).unwrap();
        assert!(
            rows.iter().any(|r| r.0 == id),
            "the freshness-due raw row must surface in the worklist"
        );
    }

    /// F-2: monotone + idempotent — a second contradicted act does not
    /// move an earlier due date; a future-due date is pulled earlier,
    /// never pushed later.
    #[test]
    fn f2_the_pull_is_monotone_and_idempotent() {
        let mut conn = seed();
        let id = chunk_id(&conn);
        // Future-due: pulled earlier to the act's ts.
        conn.execute(
            "UPDATE knowledge SET freshness_review_due = 9_999_999_999 WHERE id = ?1",
            params![id],
        )
        .unwrap();
        record_feedback(
            &mut conn,
            id,
            "dismiss",
            None,
            1_700_000_000,
            Some("s1".into()),
            "default",
            Some("contradicted"),
            None,
        )
        .unwrap();
        assert_eq!(due(&conn, id), Some(1_700_000_000));
        // A later act must NOT push the due date later.
        record_feedback(
            &mut conn,
            id,
            "dismiss",
            None,
            1_800_000_000,
            Some("s1".into()),
            "default",
            Some("contradicted"),
            None,
        )
        .unwrap();
        assert_eq!(
            due(&conn, id),
            Some(1_700_000_000),
            "a later contradicted act must not push the review later"
        );
        // An earlier act pulls it earlier still.
        record_feedback(
            &mut conn,
            id,
            "dismiss",
            None,
            1_600_000_000,
            Some("s1".into()),
            "default",
            Some("contradicted"),
            None,
        )
        .unwrap();
        assert_eq!(due(&conn, id), Some(1_600_000_000));
    }

    /// F-3: only `"contradicted"` fires — followed/overridden/ignored
    /// (and `ump_outcome = None`) never touch freshness.
    #[test]
    fn f3_only_contradicted_fires() {
        for outcome in [Some("followed"), Some("overridden"), Some("ignored"), None] {
            let mut conn = seed();
            let id = chunk_id(&conn);
            record_feedback(
                &mut conn,
                id,
                "dismiss",
                None,
                1_700_000_000,
                Some("s1".into()),
                "default",
                outcome,
                None,
            )
            .unwrap();
            assert_eq!(
                due(&conn, id),
                None,
                "outcome {outcome:?} must never touch freshness"
            );
        }
    }

    /// F-4: the `/suggest/feedback` surface passes `ump_outcome = None` —
    /// it can never fire the consumer. Structural: the suggest handler's
    /// `record_feedback` call site names `None` for the outcome.
    #[test]
    fn f4_the_suggest_surface_passes_no_outcome() {
        let src = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/handlers/suggest.rs"
        ))
        .expect("handlers/suggest.rs must be readable from the test");
        assert!(
            src.contains("record_feedback"),
            "the suggest handler must call record_feedback"
        );
        // The call site's ump_outcome argument is None — the consumer can
        // only fire on the granular "contradicted" string from /ump/feedback.
        let call_at = src.find("record_feedback").unwrap();
        let window = &src[call_at..(call_at + 600).min(src.len())];
        assert!(
            window.contains("None"),
            "the suggest surface must pass ump_outcome = None: {window}"
        );
        assert!(
            !window.contains("\"contradicted\""),
            "the suggest surface must never name the contradicted outcome"
        );
    }

    /// F-6: atomicity — the feedback upsert + the CAS are one transaction.
    /// A CAS failure (the column dropped in this fixture) fails the whole
    /// act: no feedback row survives.
    #[test]
    fn f6_a_cas_failure_rolls_back_the_upsert() {
        let mut conn = seed();
        let id = chunk_id(&conn);
        // Break ONLY the CAS: the row still exists (the EXISTS probe
        // passes), but the freshness column is gone, so the UPDATE fails.
        conn.execute_batch("ALTER TABLE knowledge DROP COLUMN freshness_review_due")
            .unwrap();
        let err = record_feedback(
            &mut conn,
            id,
            "dismiss",
            None,
            1_700_000_000,
            Some("s1".into()),
            "default",
            Some("contradicted"),
            None,
        );
        assert!(
            err.is_err(),
            "a CAS failure must fail the whole feedback act"
        );
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM suggest_feedback WHERE chunk_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n, 0,
            "the upsert must roll back with the failed CAS — no half-consumer"
        );
    }
}
