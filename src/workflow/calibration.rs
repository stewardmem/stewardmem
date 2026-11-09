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
///
/// The HITL telemetry rides the SAME audited row as the `hitl:` suffix
/// (IMPROVE11, the complaints pattern): the approver-measurement segment,
/// derived read-only from `hitl_decision_events` — absent-honest when the
/// ledger is empty (no zero-cohort segment dressed up as data). Telemetry
/// reads, never gates: no threshold in this file may influence an approval.
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
    let detail = {
        let base = if kcs_summary.is_empty() {
            record.detail()
        } else {
            format!("{} {kcs_summary}", record.detail())
        };
        let hitl = hitl_summary(conn);
        if hitl.is_empty() {
            base
        } else {
            format!("{base} hitl:{hitl}")
        }
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
/// rides the same row — joined, never parallel machinery. The HITL
/// telemetry rides as the final `hitl:` suffix (IMPROVE11 — the
/// measurement the operator signs beside κ; absent-honest when the ledger
/// is empty).
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
    let detail = {
        let hitl = hitl_summary(conn);
        if hitl.is_empty() {
            detail
        } else {
            format!("{detail} hitl:{hitl}")
        }
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

// ── IMPROVE11: the HITL approver-measurement aggregate ──────────────────────
// The derivation behind the monthly report's `hitl:` segment and the
// DPO-gated scoreboard's `hitl` block — the report and the board measure
// the SAME numbers. ALL SQL/Rust over `hitl_decision_events` +
// `proposals`; NO model anywhere (law c). Read-only: nothing here writes,
// and NO threshold here gates anything — the inalienability law's only
// consumer is the human reviewing their own pattern.

/// One per-source decay-vs-decision ratio: a proposal source that only ever
/// TTL-expires is visible as noise-generation, informing source tuning
/// (plan 11 §1.3). `expired` = proposals the MACHINE aged out
/// (`expire_if_stale`, age past the TTL at decision); `decided` = proposals
/// a HUMAN decided — DISTINCT proposals, not acts (an edit-then-approve pair
/// is one decided proposal; both sides of the ratio count proposals).
pub(crate) struct DecayRatio {
    pub source: String,
    pub expired: i64,
    pub decided: i64,
}

/// The approver-measurement aggregate. Zeros and NULLs are honest answers:
/// an empty ledger reads 0/0/0 with NULL latency and depth — absence is
/// never dressed up as perfection (the aftersales law).
pub(crate) struct HitlMeasures {
    pub approved: i64,
    pub rejected: i64,
    pub edited: i64,
    pub median_latency_ms: Option<i64>,
    /// The burst signal: the largest count of acts by ONE actor inside one
    /// wall-clock minute bucket (ts/60). 1 = no burst observed.
    pub burst_max_acts_per_min: i64,
    /// Mean of the ABSOLUTE edit deltas — the depth the human changed the
    /// artifact by, averaged over edits (the signed raw deltas stay in the
    /// ledger; this is the report-side magnitude).
    pub edit_depth_mean_chars: Option<f64>,
    pub decay_by_source: Vec<DecayRatio>,
}

pub(crate) fn hitl_measures(conn: &Connection) -> rusqlite::Result<HitlMeasures> {
    let mut counts =
        conn.prepare("SELECT decision, COUNT(*) FROM hitl_decision_events GROUP BY decision")?;
    let mut approved = 0;
    let mut rejected = 0;
    let mut edited = 0;
    let mut rows = counts.query([])?;
    while let Some(r) = rows.next()? {
        let (decision, n): (String, i64) = (r.get(0)?, r.get(1)?);
        match decision.as_str() {
            "approve" => approved = n,
            "reject" => rejected = n,
            "edit" => edited = n,
            // the CHECK closes the vocabulary; an unknown value is not a row.
            _ => {}
        }
    }
    // Median latency: sorted collect, middle element (even count: the mean of
    // the two middles). Empty → NULL.
    let latencies: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT latency_ms FROM hitl_decision_events
             WHERE latency_ms IS NOT NULL ORDER BY latency_ms",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let median_latency_ms = if latencies.is_empty() {
        None
    } else if latencies.len() % 2 == 1 {
        Some(latencies[latencies.len() / 2])
    } else {
        let hi = latencies.len() / 2;
        Some((latencies[hi - 1] + latencies[hi]) / 2)
    };
    let burst_max_acts_per_min: i64 = conn.query_row(
        "SELECT COALESCE(MAX(c), 0) FROM
             (SELECT COUNT(*) AS c FROM hitl_decision_events
              GROUP BY actor, ts / 60)",
        [],
        |r| r.get(0),
    )?;
    let edit_depth_mean_chars: Option<f64> = conn
        .query_row(
            "SELECT AVG(ABS(edit_depth_chars)) FROM hitl_decision_events
             WHERE edit_depth_chars IS NOT NULL",
            [],
            |r| r.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten();
    // Decay-vs-decision per source. Machine-expired: rejected with an age at
    // decision PAST the TTL — exactly `expire_if_stale`'s branch (the human
    // paths refuse past-TTL rows, so a within-TTL reject is a human act with
    // its ledger row). Human-decided: ledger rows joined to their proposal's
    // source. Sources present in either set appear; NULL source reads
    // 'unknown'.
    let ttl = crate::config::proposal_ttl_secs();
    let mut decay: std::collections::BTreeMap<String, (i64, i64)> = Default::default();
    {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(source, 'unknown'), COUNT(*) FROM proposals
             WHERE status = 'rejected' AND decided_at IS NOT NULL
               AND (decided_at - created_at) > ?1
             GROUP BY 1",
        )?;
        let rows = stmt.query_map([ttl], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for r in rows.flatten() {
            decay.entry(r.0).or_default().0 = r.1;
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(p.source, 'unknown'), COUNT(DISTINCT p.id) FROM proposals p
             JOIN hitl_decision_events e ON e.proposal_id = p.id
             GROUP BY 1",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        for r in rows.flatten() {
            decay.entry(r.0).or_default().1 = r.1;
        }
    }
    let decay_by_source = decay
        .into_iter()
        .map(|(source, (expired, decided))| DecayRatio {
            source,
            expired,
            decided,
        })
        .collect();
    Ok(HitlMeasures {
        approved,
        rejected,
        edited,
        median_latency_ms,
        burst_max_acts_per_min,
        edit_depth_mean_chars,
        decay_by_source,
    })
}

/// The report-segment renderer (the complaints pattern's shape): the compact
/// `hitl:` payload, or EMPTY when the ledger carries no acts — absence is
/// honest, never a zero-cohort segment. `pub` because the format is the
/// fact the audit chain commits to: an auditor holding the chain verifies
/// the segment against the ledger with this exact rendering (the SDK's
/// `CalibrationRecord::detail` precedent).
pub fn hitl_summary(conn: &Connection) -> String {
    let Ok(m) = hitl_measures(conn) else {
        return String::new();
    };
    if m.approved + m.rejected + m.edited == 0 {
        return String::new();
    }
    let mut s = format!(
        "approved:{} rejected:{} edited:{} median_latency_ms:{} \
         burst_max_acts_per_min:{} edit_depth_mean_chars:{}",
        m.approved,
        m.rejected,
        m.edited,
        m.median_latency_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string()),
        m.burst_max_acts_per_min,
        m.edit_depth_mean_chars
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "null".to_string()),
    );
    if !m.decay_by_source.is_empty() {
        s.push_str(" decay:");
        let parts: Vec<String> = m
            .decay_by_source
            .iter()
            .map(|d| format!("{}:{}/{}", d.source, d.expired, d.decided))
            .collect();
        s.push_str(&parts.join(","));
    }
    s
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
