//! Calibration persistence: weekly REPORT + monthly human-signed gate, both
//! riding the workflow audit chain. The audit row carries the hashed record;
//! `schema_meta` stamps the cadence/baseline state so staleness is checkable
//! without trusting any client input.

use brain_engine_sdk::calibration::{
    CalibrationRecord, NO_KAPPA, month_due, month_index, week_due,
};
use rusqlite::{Connection, OptionalExtension};

/// Meta keys — one owner, written only through this module.
const KEY_LAST_REPORT_AT: &str = "calibration_last_report_at";
const KEY_LAST_SIGNED_MONTH: &str = "calibration_last_signed_month";
const KEY_BASELINE_UNITS: &str = "calibration_baseline_units";
const KEY_LAST_KAPPA_UNITS: &str = "calibration_last_kappa_units";

fn meta_get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM schema_meta WHERE key = ?1", [key], |r| {
        r.get::<_, String>(0)
    })
    .optional()
    .ok()
    .flatten()
}

fn meta_set(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO schema_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )
}

pub(crate) fn last_report_at(conn: &Connection) -> Option<i64> {
    meta_get(conn, KEY_LAST_REPORT_AT)?.parse().ok()
}

pub(crate) fn last_baseline_units(conn: &Connection) -> Option<i32> {
    meta_get(conn, KEY_BASELINE_UNITS)?.parse().ok()
}

pub(crate) fn last_kappa_units(conn: &Connection) -> i32 {
    meta_get(conn, KEY_LAST_KAPPA_UNITS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(NO_KAPPA)
}

/// Whether a weekly calibration REPORT is due right now.
pub(crate) fn report_due(conn: &Connection, now: i64) -> bool {
    week_due(last_report_at(conn), now)
}

/// Whether the monthly human-signed gate blocks a sign-off attempt.
pub(crate) fn signature_blocked(conn: &Connection, now: i64) -> bool {
    !month_due(
        meta_get(conn, KEY_LAST_SIGNED_MONTH).and_then(|v| v.parse().ok()),
        month_index(now),
    )
}

/// Emit a calibration REPORT (machine-generated, reviewer empty): the audit
/// row lands first (best-effort chain), then the cadence/baseline stamps.
/// Stamps are transactional with each other; a failed stamp bumps nothing and
/// the next call re-reports (idempotent at the gate).
pub fn record_report(
    conn: &Connection,
    score_units: i32,
    now: i64,
    kcs_summary: &str,
) -> Result<(), rusqlite::Error> {
    let baseline = last_baseline_units(conn);
    // Baseline absent → uplift 0 (the first report anchors, never scores).
    let uplift = baseline.map_or(0, |b| score_units - b);
    let record = CalibrationRecord::new(last_kappa_units(conn), uplift, "");
    let detail = if kcs_summary.is_empty() {
        record.detail()
    } else {
        format!("{} {kcs_summary}", record.detail())
    };
    super::audit_write_global(
        conn,
        "calibration/report",
        crate::audit::AuditStatus::Ok,
        &detail,
    );
    meta_set(conn, KEY_LAST_REPORT_AT, &now.to_string())?;
    if baseline.is_none() {
        meta_set(conn, KEY_BASELINE_UNITS, &score_units.to_string())?;
    }
    Ok(())
}

/// The preregistered conjunction's three recorded arms — Cohen's κ, Gwet's
/// AC1, and the raw percent agreement — the values a human signs TOGETHER
/// (KM-4: no single-metric signing; the sign payload refuses a single
/// metric). One value on the record path, so the conjunction cannot
/// decompose between the wire and the audited signature row.
pub(crate) struct ConjunctionUnits {
    pub kappa_units: i32,
    pub ac1_units: i32,
    pub raw_percent: i32,
}

/// Record a human-signed monthly calibration (the drift gate). Caller has
/// already enforced Admin/DPO authorization; `reviewer_id` is bounded there.
/// `conjunction` is the triple the reviewer typed — κ, AC1, raw — and it
/// rides the SAME audited signature row as a suffix on the record detail,
/// so the audit can verify the preregistered conjunction was honored
/// (KM-4). `complaints_summary` is the complaints register extract that
/// rides the same row — joined, never parallel machinery.
pub(crate) fn record_signed(
    conn: &Connection,
    conjunction: ConjunctionUnits,
    score_units: i32,
    reviewer_id: &str,
    now: i64,
    complaints_summary: &str,
) -> Result<(), rusqlite::Error> {
    let uplift = last_baseline_units(conn).map_or(0, |b| score_units - b);
    let record = CalibrationRecord::new(conjunction.kappa_units, uplift, reviewer_id);
    // The `CalibrationRecord` detail stays byte-stable (the SDK's pinned
    // format); the conjunction's other two arms append after it.
    let base = format!(
        "{} ac1_units:{} raw_percent:{}",
        record.detail(),
        conjunction.ac1_units,
        conjunction.raw_percent
    );
    let detail = if complaints_summary.is_empty() {
        base
    } else {
        format!("{base} complaints:{complaints_summary}")
    };
    super::audit_write_global(
        conn,
        "calibration/sign",
        crate::audit::AuditStatus::Ok,
        &detail,
    );
    meta_set(conn, KEY_LAST_SIGNED_MONTH, &month_index(now).to_string())?;
    meta_set(
        conn,
        KEY_LAST_KAPPA_UNITS,
        &conjunction.kappa_units.to_string(),
    )?;
    // A human signature re-anchors the baseline: deltas stay OUR-vs-US.
    meta_set(conn, KEY_BASELINE_UNITS, &score_units.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::verify_chain;
    use crate::migration::run_migration;

    fn db() -> Connection {
        crate::register_sqlite_vec::register_sqlite_vec();
        let mut conn = Connection::open_in_memory().unwrap();
        run_migration(&mut conn, 1).unwrap();
        conn
    }

    /// The conjunction triple as one value — the KM-4 shape at the call site.
    fn cj(kappa: i32, ac1: i32, raw: i32) -> ConjunctionUnits {
        ConjunctionUnits {
            kappa_units: kappa,
            ac1_units: ac1,
            raw_percent: raw,
        }
    }

    #[test]
    fn report_due_then_stamped_for_a_week() {
        let conn = db();
        assert!(report_due(&conn, 1000));
        record_report(&conn, 9000, 1000, "").unwrap();
        assert!(!report_due(&conn, 1000 + 7 * 86400 - 1));
        assert!(report_due(&conn, 1000 + 7 * 86400));
        // First report anchors the baseline; uplift 0.
        assert_eq!(last_baseline_units(&conn), Some(9000));
    }

    #[test]
    fn signed_gate_blocks_within_month_and_anchors_kappa() {
        let conn = db();
        record_report(&conn, 8000, 1000, "").unwrap();
        assert!(!signature_blocked(&conn, 2000));
        record_signed(&conn, cj(8500, 8000, 84), 8200, "dpo-1", 2000, "").unwrap();
        assert!(signature_blocked(&conn, 3000));
        assert_eq!(last_kappa_units(&conn), 8500);
        // Signature re-anchored the baseline to the signed score.
        assert_eq!(last_baseline_units(&conn), Some(8200));
        // A later report carries the human κ, not the sentinel.
        record_report(&conn, 8300, 1000 + 8 * 86400, "").unwrap();
        assert!(verify_chain(&conn), "chain stays green");
    }

    #[test]
    fn records_land_on_the_audit_chain_with_their_detail() {
        let conn = db();
        record_report(&conn, 9000, 100, "").unwrap();
        record_signed(&conn, cj(8000, 8000, 80), 9100, "dpo-9", 200, "").unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE kind='workflow'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "both records audited");
        // The chain stores hashes (target_hash/detail_hash), never raw text.
        let hashes: (bool, bool) = conn
            .query_row(
                "SELECT COUNT(*) = 2, COUNT(detail_hash) = 2 FROM audit_events WHERE kind='workflow'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(hashes.0 && hashes.1, "two rows, both with detail hashes");
        assert!(verify_chain(&conn));
    }

    /// complaint_register_report_joins_monthly_calibration (service leg):
    /// the complaints extract rides the SAME audited signature row.
    #[test]
    fn signed_row_carries_the_complaints_extract() {
        let conn = db();
        record_signed(&conn, cj(8000, 8000, 80), 8100, "dpo-1", 100, "").unwrap();
        record_report(&conn, 8200, 100 + 40 * 86400, "").unwrap();
        crate::workflow::calibration::record_signed(
            &conn,
            cj(8300, 8000, 80),
            8250,
            "dpo-2",
            200 + 40 * 86400,
            "{\"total\":7}",
        )
        .unwrap();
        // Both signatures exist; details are hashed at rest — verify the
        // chain stays green with the extract riding row three.
        assert!(crate::audit::verify_chain(&conn));
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE kind='workflow' AND target_hash = ?1",
                [crate::audit::hash("calibration/sign")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "two signature rows on the chain");
    }

    /// KM-4's audit half: the conjunction triple rides the SAME audited
    /// signature row. Details are hashed at rest, so the verification
    /// recomputes the expected detail — the SDK record's own detail format
    /// (κ arm) plus the `ac1_units`/`raw_percent` suffix — and matches the
    /// stored hash. An auditor holding the chain can verify all three
    /// values were signed together, which is what the preregistered
    /// conjunction requires.
    #[test]
    fn signed_row_carries_the_conjunction_triple() {
        let conn = db();
        record_signed(&conn, cj(8500, 8049, 84), 8200, "dpo-1", 100, "").unwrap();
        // Fresh DB: no baseline yet, so the record's uplift arm reads 0.
        let expected_detail = format!(
            "{} ac1_units:8049 raw_percent:84",
            CalibrationRecord::new(8500, 0, "dpo-1").detail()
        );
        let detail_hash: String = conn
            .query_row(
                "SELECT detail_hash FROM audit_events WHERE kind='workflow' AND target_hash = ?1",
                [crate::audit::hash("calibration/sign")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            detail_hash,
            crate::audit::hash(&expected_detail),
            "the audited detail IS the conjunction triple (κ + AC1 + raw, one row)"
        );
        assert!(verify_chain(&conn));
    }
}
