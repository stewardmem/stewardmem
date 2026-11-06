//! The hand-down act: a verified procedure becomes executable by the
//! lowest tier. Pure-typed sealing plus the chain-row act; no model
//! anywhere in the snapshot, the match, or the comparison. The sealed
//! artifact is the plan plus the verify step plus the dead-end plus
//! the executed rows — never the differential, which stays with the
//! tier that owns it.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::gdl::{DeadEnd, GdlCase, PlanStep, TestLogRow, VerifyStepSpec, red_flag_is_open};
use super::session_log;

/// The step ceiling. The runbook step law is the precedent: a procedure
/// is a bounded step list, and a bound past this is a corpus, not a
/// procedure. Rational ceiling: no pilot source case exists to
/// calibrate tighter, so the ceiling is named and never retuned silent.
pub(crate) const MAX_HAND_DOWN_STEPS: usize = 100;
/// Per-operand text ceiling. Every text operand travels through the
/// launch bound (the launch ticket law); the snapshot builder pre-sizes
/// and caps each operand at this, and refuses overflow by name rather
/// than truncating. Rational ceiling for the same reason as the step
/// ceiling: named, never retuned silent.
pub(crate) const MAX_HAND_DOWN_TEXT_BYTES: usize = 64 * 1024;
/// Whole-snapshot ceiling. The sealed snapshot JSON is built, never
/// pasted; a snapshot past this is refused by name. Rational ceiling,
/// same provenance as the per-operand ceiling.
pub(crate) const MAX_HAND_DOWN_TOTAL_BYTES: usize = 256 * 1024;
/// The scan horizon for the intake matcher. Beyond this many marked
/// procedures the matcher refuses by name; it never truncates the scan
/// silently and never reads past the horizon.
pub(crate) const MAX_HAND_DOWN_MARKS_SCANNED: usize = 1024;
/// The chain-row payload ceiling. The mark payload is a few hundred
/// bytes of typed fields; this bounds the row beside the session-log
/// payload cap.
pub(crate) const HAND_DOWN_ROW_MAX_BYTES: usize = 4096;
/// The chain-row kind the act appends.
pub(crate) const HAND_DOWN_ROW_KIND: &str = "control:handdown";
/// The match-row kind the intake matcher appends.
pub(crate) const HAND_DOWN_MATCH_ROW_KIND: &str = "control:match";

/// The conformance receipt carried on the mark. Closed fields, typed
/// decode with unknown fields refused; the kernel never reads the
/// sibling pack — the receipt is the whole precondition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConformanceReceipt {
    pub run_id: String,
    pub pack_hash: String,
    pub outputs_hash: String,
    pub verdict: String,
    pub decided_at: i64,
}

/// The handed-down identity: the source run that earned the procedure,
/// the receipt that gates it, and the digest that binds the mark to
/// the sealed bytes. Additive case state; old states parse to absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandDownMark {
    pub procedure_id: i64,
    pub conformance: ConformanceReceipt,
    pub snapshot_digest: String,
}

/// The sealed snapshot: plan plus verify step plus dead-end plus the
/// executed rows plus the source ticket (the match needle). The
/// hypotheses are deliberately absent — the differential is not
/// laundered downstream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub ticket: String,
    pub plan: Vec<PlanStep>,
    pub verify_step: VerifyStepSpec,
    pub dead_end: DeadEnd,
    pub test_log: Vec<TestLogRow>,
}

/// The chain-row payload the act appends. Typed, unknown fields
/// refused, bounded by the row ceiling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandDownRowPayload {
    pub procedure_id: i64,
    pub conformance: ConformanceReceipt,
    pub snapshot_digest: String,
    pub steps: usize,
}

/// The match-row payload the intake matcher appends on a match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandDownMatchPayload {
    pub procedure_id: i64,
    pub conformance_run_id: String,
    pub scan_position: usize,
    pub snapshot_digest: String,
    pub skipped: Vec<i64>,
}

/// A refusal, by name. The message names the missing or malformed
/// limb; the caller maps it to its own disposition.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HandDownError(pub String);

impl HandDownError {
    fn named(law: &str, detail: &str) -> Self {
        Self(format!("{law}: {detail}"))
    }
}

impl std::fmt::Display for HandDownError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for HandDownError {}

fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn check_text_bytes(value: &str, what: &str) -> Result<(), HandDownError> {
    if value.len() > MAX_HAND_DOWN_TEXT_BYTES {
        return Err(HandDownError::named(
            "hand-down bounds",
            &format!(
                "{what} is {} bytes — at most {MAX_HAND_DOWN_TEXT_BYTES} may be sealed",
                value.len()
            ),
        ));
    }
    Ok(())
}

/// Seal a resolved source case into a snapshot plus a mark. Every
/// eligibility limb is refused by name; no model, no normalization,
/// no truncation. The digest binds the mark to the canonical bytes.
pub(crate) fn seal(
    source: &GdlCase,
    source_run_id: i64,
    receipt: &ConformanceReceipt,
    source_status: &str,
) -> Result<(HandDownMark, Snapshot), HandDownError> {
    if receipt.verdict != "pass" {
        return Err(HandDownError::named(
            "hand-down conformance",
            &format!(
                "conformance verdict {:?} is not pass — the mark refuses without a passing receipt",
                receipt.verdict
            ),
        ));
    }
    if !is_lower_hex_64(&receipt.pack_hash) || !is_lower_hex_64(&receipt.outputs_hash) {
        return Err(HandDownError::named(
            "hand-down conformance",
            "conformance hashes must be 64 lowercase hex characters",
        ));
    }
    if receipt.pack_hash == receipt.outputs_hash {
        return Err(HandDownError::named(
            "hand-down conformance",
            "conformance pack and outputs hashes must be distinct",
        ));
    }
    if receipt.run_id.is_empty() || receipt.run_id.len() > 64 {
        return Err(HandDownError::named(
            "hand-down conformance",
            "conformance run_id must be 1..=64 characters",
        ));
    }
    if receipt.decided_at < 0 {
        return Err(HandDownError::named(
            "hand-down conformance",
            "conformance decided_at must be non-negative",
        ));
    }
    if source_status != "resolved" {
        return Err(HandDownError::named(
            "hand-down eligibility",
            &format!(
                "source run status {source_status:?} is not resolved — only a resolved case may be handed down"
            ),
        ));
    }
    if source.parent_case_id.is_some() {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source run is a split child — a child is not a procedure source",
        ));
    }
    let verify = source.verify.as_ref().ok_or_else(|| {
        HandDownError::named(
            "hand-down eligibility",
            "source case carries no verify artifact",
        )
    })?;
    if !verify.pass || !verify.negative_check {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source verify is not a passing verify with the negative check",
        ));
    }
    if source.capture.is_none() {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source case carries no capture artifact",
        ));
    }
    if source.closure.is_none() {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source case carries no closure artifact",
        ));
    }
    let triage = source.triage.as_ref().ok_or_else(|| {
        HandDownError::named(
            "hand-down eligibility",
            "source case carries no triage artifact",
        )
    })?;
    if triage.verdict != "accept" || red_flag_is_open(&triage.red_flag) {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source triage is not an accepted, red-flag-closed record",
        ));
    }
    if source.ticket.trim().chars().count() < super::gdl::MIN_CITATION_LOCATOR_CHARS {
        return Err(HandDownError::named(
            "hand-down eligibility",
            "source ticket is below the locator floor — it cannot be a match needle",
        ));
    }
    if source.plan.is_empty() {
        return Err(HandDownError::named(
            "hand-down plan",
            "source plan is empty — a procedure with no steps seals nothing",
        ));
    }
    if source.plan.len() > MAX_HAND_DOWN_STEPS {
        return Err(HandDownError::named(
            "hand-down plan",
            &format!(
                "source plan carries {} steps — at most {MAX_HAND_DOWN_STEPS} may be sealed",
                source.plan.len()
            ),
        ));
    }
    let n = source.plan.len() as i64;
    for (i, step) in source.plan.iter().enumerate() {
        if step.order != i as i64 + 1 {
            return Err(HandDownError::named(
                "hand-down plan",
                &format!(
                    "plan step order {} is not contiguous at position {}",
                    step.order,
                    i + 1
                ),
            ));
        }
        if !matches!(step.skill_gate.as_str(), "L0" | "L1" | "L2" | "L3") {
            return Err(HandDownError::named(
                "hand-down plan",
                &format!(
                    "plan step {} skill_gate {:?} is not one of L0..L3",
                    step.order, step.skill_gate
                ),
            ));
        }
        if step.expected.trim().is_empty() {
            return Err(HandDownError::named(
                "hand-down plan",
                &format!("plan step {} expected is empty", step.order),
            ));
        }
        check_text_bytes(&step.description, "plan step description")?;
        check_text_bytes(&step.command, "plan step command")?;
        check_text_bytes(&step.expected, "plan step expected")?;
    }
    let verify_step = source.verify_step.clone().ok_or_else(|| {
        HandDownError::named(
            "hand-down plan",
            "source case carries no verify step — the pass bar must exist",
        )
    })?;
    let dead_end = source
        .dead_end
        .clone()
        .ok_or_else(|| HandDownError::named("hand-down plan", "source case carries no dead-end"))?;
    check_text_bytes(&verify_step.re_run, "verify step re_run")?;
    check_text_bytes(&verify_step.pass_condition, "verify step pass_condition")?;
    check_text_bytes(&source.ticket, "source ticket")?;
    // The harness rows cover every plan order exactly once.
    if source.test_log.len() != source.plan.len() {
        return Err(HandDownError::named(
            "hand-down harness",
            &format!(
                "test log carries {} rows for {n} plan steps — every step executes exactly once",
                source.test_log.len()
            ),
        ));
    }
    let mut orders: Vec<i64> = source.test_log.iter().map(|r| r.order).collect();
    orders.sort_unstable();
    for (i, order) in orders.iter().enumerate() {
        if *order != i as i64 + 1 {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!(
                    "test log order {order} does not cover plan step {} exactly once",
                    i + 1
                ),
            ));
        }
    }
    for row in &source.test_log {
        if row.expected.trim().is_empty() {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!("harness row {} expected is empty", row.order),
            ));
        }
        let actual = row.actual.as_deref().unwrap_or("");
        if actual.trim().is_empty() {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!("harness row {} actual is empty", row.order),
            ));
        }
        if row.verdict == super::gdl::Verdict::Pending {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!("harness row {} is unresolved", row.order),
            ));
        }
        let evidence = row.evidence_ref.as_deref().unwrap_or("");
        if evidence.trim().is_empty() {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!("harness row {} evidence_ref is empty", row.order),
            ));
        }
        if row.dtfvc.diagnose.trim().is_empty() || row.dtfvc.test.trim().is_empty() {
            return Err(HandDownError::named(
                "hand-down harness",
                &format!("harness row {} missing its diagnose/test floor", row.order),
            ));
        }
        check_text_bytes(&row.expected, "harness row expected")?;
        check_text_bytes(actual, "harness row actual")?;
        check_text_bytes(evidence, "harness row evidence_ref")?;
        check_text_bytes(&row.dtfvc.diagnose, "harness row diagnose")?;
        check_text_bytes(&row.dtfvc.test, "harness row test")?;
    }
    let mut sorted_log = source.test_log.clone();
    sorted_log.sort_by_key(|r| r.order);
    let snapshot = Snapshot {
        ticket: source.ticket.clone(),
        plan: source.plan.clone(),
        verify_step,
        dead_end,
        test_log: sorted_log,
    };
    let canonical = serde_json::to_string(&snapshot)
        .map_err(|e| HandDownError::named("hand-down digest", &format!("snapshot encode: {e}")))?;
    if canonical.len() > MAX_HAND_DOWN_TOTAL_BYTES {
        return Err(HandDownError::named(
            "hand-down bounds",
            &format!(
                "snapshot JSON is {} bytes — at most {MAX_HAND_DOWN_TOTAL_BYTES} may be sealed",
                canonical.len()
            ),
        ));
    }
    let digest = crate::audit::hash(&canonical);
    let mark = HandDownMark {
        procedure_id: source_run_id,
        conformance: receipt.clone(),
        snapshot_digest: digest,
    };
    Ok((mark, snapshot))
}

/// The result of the act: a new mark, or the no-op receipt for a
/// repeat of the same pair (the existing row is unchanged).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HandDownResult {
    Created(HandDownMark),
    NoOp(HandDownMark),
}

impl HandDownResult {
    pub(crate) fn mark(&self) -> &HandDownMark {
        match self {
            HandDownResult::Created(m) | HandDownResult::NoOp(m) => m,
        }
    }
    pub(crate) fn is_created(&self) -> bool {
        matches!(self, HandDownResult::Created(_))
    }
}

fn read_source_run(
    conn: &Connection,
    procedure_id: i64,
) -> Result<(String, String, String, String), HandDownError> {
    let row: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT domain, kind, status, state_json FROM workflow_runs WHERE id = ?1",
            params![procedure_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| HandDownError::named("hand-down act", &format!("source read: {e}")))?;
    row.ok_or_else(|| {
        HandDownError::named(
            "hand-down act",
            &format!("source run {procedure_id} not found"),
        )
    })
}

fn existing_marks(
    conn: &Connection,
    procedure_id: i64,
) -> Result<Vec<HandDownRowPayload>, HandDownError> {
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM agent_session_events \
             WHERE run_id = ?1 AND kind = 'control:handdown' ORDER BY seq",
        )
        .map_err(|e| HandDownError::named("hand-down act", &format!("mark scan: {e}")))?;
    let rows = stmt
        .query_map(params![procedure_id], |r| r.get::<_, String>(0))
        .map_err(|e| HandDownError::named("hand-down act", &format!("mark scan: {e}")))?;
    let mut out = Vec::new();
    for row in rows {
        let payload =
            row.map_err(|e| HandDownError::named("hand-down act", &format!("mark scan: {e}")))?;
        let parsed: HandDownRowPayload = serde_json::from_str(&payload).map_err(|e| {
            HandDownError::named("hand-down act", &format!("existing mark is malformed: {e}"))
        })?;
        out.push(parsed);
    }
    Ok(out)
}

/// The act, in the caller's transaction. Loads the resolved source
/// case, seals it, and appends exactly one chain row per
/// (procedure, conformance run) pair. A repeat of the same pair is
/// the no-op receipt; a different conformance run for the same
/// procedure is a refusal by name (one procedure, one mark).
pub(crate) fn hand_down_in_tx(
    conn: &Connection,
    procedure_id: i64,
    receipt: &ConformanceReceipt,
) -> Result<HandDownResult, HandDownError> {
    let (domain, kind, status, state_json) = read_source_run(conn, procedure_id)?;
    if kind != "troubleshoot" {
        return Err(HandDownError::named(
            "hand-down act",
            "source run kind is not troubleshoot",
        ));
    }
    let source: GdlCase = serde_json::from_str(&state_json).map_err(|e| {
        HandDownError::named("hand-down act", &format!("source state is not a case: {e}"))
    })?;
    let (mark, _snapshot) = seal(&source, procedure_id, receipt, &status)?;
    if let Some(existing) = existing_marks(conn, procedure_id)?.into_iter().next() {
        if existing.conformance.run_id == receipt.run_id {
            return Ok(HandDownResult::NoOp(HandDownMark {
                procedure_id: existing.procedure_id,
                conformance: existing.conformance,
                snapshot_digest: existing.snapshot_digest,
            }));
        }
        return Err(HandDownError::named(
            "hand-down act",
            &format!(
                "procedure {procedure_id} is already handed down under conformance run {:?} — one procedure, one mark",
                existing.conformance.run_id
            ),
        ));
    }
    let payload = HandDownRowPayload {
        procedure_id: mark.procedure_id,
        conformance: mark.conformance.clone(),
        snapshot_digest: mark.snapshot_digest.clone(),
        steps: source.plan.len(),
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| HandDownError::named("hand-down act", &format!("mark encode: {e}")))?;
    if payload_json.len() > HAND_DOWN_ROW_MAX_BYTES {
        return Err(HandDownError::named(
            "hand-down act",
            "mark row exceeds its byte ceiling",
        ));
    }
    let key = format!("handdown:{}:{}", mark.procedure_id, mark.conformance.run_id);
    let now = chrono::Utc::now().timestamp();
    let (inserted, _seq) = session_log::append(
        conn,
        procedure_id,
        HAND_DOWN_ROW_KIND,
        &payload_json,
        &key,
        now,
    )
    .map_err(|e| HandDownError::named("hand-down act", &format!("mark append: {e}")))?;
    if !inserted {
        // The idempotency guard already proved this pair exists with
        // these bytes; the scan above is the authority, and this is
        // the no-op receipt.
        return Ok(HandDownResult::NoOp(mark));
    }
    let detail = format!(
        "procedure={} conf={} steps={} digest={}",
        mark.procedure_id, mark.conformance.run_id, payload.steps, mark.snapshot_digest
    );
    crate::audit::record_tenant_checked(
        conn,
        crate::audit::AuditKind::Procedure,
        crate::workflow::ACTOR,
        &format!("procedure:{}", mark.procedure_id),
        crate::audit::AuditStatus::Ok,
        &detail,
        &domain,
    )
    .map_err(|e| HandDownError::named("hand-down act", &format!("mark audit: {e}")))?;
    Ok(HandDownResult::Created(mark))
}

/// Re-derive the snapshot for a mark from the resolved source row and
/// re-check the digest. Returns the snapshot only when the digest
/// binds; a stale or corrupt candidate is refused by name, never
/// trusted. This is the matcher's and the narrow loop's one source
/// of sealed bytes.
pub(crate) fn load_snapshot(
    conn: &Connection,
    mark: &HandDownMark,
) -> Result<Snapshot, HandDownError> {
    let (_domain, kind, status, state_json) = read_source_run(conn, mark.procedure_id)?;
    if kind != "troubleshoot" {
        return Err(HandDownError::named(
            "hand-down snapshot",
            "source run kind is not troubleshoot",
        ));
    }
    let source: GdlCase = serde_json::from_str(&state_json).map_err(|e| {
        HandDownError::named(
            "hand-down snapshot",
            &format!("source state is not a case: {e}"),
        )
    })?;
    let (resealed, snapshot) = seal(&source, mark.procedure_id, &mark.conformance, &status)?;
    if resealed.snapshot_digest != mark.snapshot_digest {
        return Err(HandDownError::named(
            "hand-down snapshot",
            &format!(
                "source digest {} does not bind mark digest {} — stale candidate skipped",
                resealed.snapshot_digest, mark.snapshot_digest
            ),
        ));
    }
    Ok(snapshot)
}

/// List every marked procedure in a domain, procedure ascending, capped
/// at the scan horizon. Each entry is the mark as stored; snapshot
/// validity is checked by the caller per candidate.
pub(crate) fn marked_procedures(
    conn: &Connection,
    domain: &str,
) -> Result<Vec<HandDownMark>, HandDownError> {
    let mut stmt = conn
        .prepare(
            "SELECT w.id FROM workflow_runs w \
             WHERE w.domain = ?1 AND w.kind = 'troubleshoot' \
             AND EXISTS (SELECT 1 FROM agent_session_events e \
                         WHERE e.run_id = w.id AND e.kind = 'control:handdown') \
             ORDER BY w.id ASC",
        )
        .map_err(|e| HandDownError::named("hand-down scan", &format!("mark scan: {e}")))?;
    let rows = stmt
        .query_map(params![domain], |r| r.get::<_, i64>(0))
        .map_err(|e| HandDownError::named("hand-down scan", &format!("mark scan: {e}")))?;
    let mut ids: Vec<i64> = Vec::new();
    for id in rows {
        ids.push(
            id.map_err(|e| HandDownError::named("hand-down scan", &format!("mark scan: {e}")))?,
        );
    }
    if ids.len() > MAX_HAND_DOWN_MARKS_SCANNED {
        return Err(HandDownError::named(
            "hand-down scan",
            &format!(
                "marked procedures exceed the scan horizon {MAX_HAND_DOWN_MARKS_SCANNED} — refuse by name, never truncate"
            ),
        ));
    }
    let mut marks = Vec::new();
    for id in ids {
        let existing = existing_marks(conn, id)?;
        if let Some(first) = existing.into_iter().next() {
            marks.push(HandDownMark {
                procedure_id: first.procedure_id,
                conformance: first.conformance,
                snapshot_digest: first.snapshot_digest,
            });
        }
    }
    Ok(marks)
}

/// The deterministic containment match: the current ticket, casefolded,
/// contains the source ticket, casefolded, as a substring, and the
/// source ticket clears the locator floor. Byte-similar only; the
/// caller walks candidates in procedure order and takes the first
/// match.
pub(crate) fn ticket_matches(current_ticket: &str, source_ticket: &str) -> bool {
    let needle = source_ticket.trim();
    if needle.chars().count() < super::gdl::MIN_CITATION_LOCATOR_CHARS {
        return false;
    }
    current_ticket
        .to_lowercase()
        .contains(&needle.to_lowercase())
}

/// The pure oracle comparison: per step order, the delivered row must
/// equal the handed-down row byte-for-byte over the compared fields.
/// The first mismatch names its step. No model, no normalization.
pub(crate) fn harness_deviation(
    expected_rows: &[TestLogRow],
    delivered_rows: &[TestLogRow],
) -> Option<String> {
    let mut expected: Vec<&TestLogRow> = expected_rows.iter().collect();
    expected.sort_by_key(|r| r.order);
    let mut delivered: Vec<&TestLogRow> = delivered_rows.iter().collect();
    delivered.sort_by_key(|r| r.order);
    for want in &expected {
        let Some(got) = delivered.iter().find(|r| r.order == want.order) else {
            return Some(format!(
                "hand-down deviation at step {}: the harness left the record — escalate",
                want.order
            ));
        };
        if got.expected != want.expected
            || got.actual != want.actual
            || got.verdict != want.verdict
            || got.evidence_ref != want.evidence_ref
            || got.dtfvc != want.dtfvc
        {
            return Some(format!(
                "hand-down deviation at step {}: the harness left the record — escalate",
                want.order
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::run_migration;
    use crate::pool::SqliteConnectionManager;
    use crate::register_sqlite_vec::register_sqlite_vec;
    use crate::workflow::gdl::{Dtfvc, GdlCase, GdlPhase, Verdict};

    fn receipt(run_id: &str) -> ConformanceReceipt {
        ConformanceReceipt {
            run_id: run_id.to_string(),
            pack_hash: "a".repeat(64),
            outputs_hash: "b".repeat(64),
            verdict: "pass".to_string(),
            decided_at: 1,
        }
    }

    fn resolved_source() -> GdlCase {
        let mut case = GdlCase::fresh("gradebook workbook fails to open");
        case.phase = GdlPhase::Handoff;
        case.intake = Some(crate::workflow::gdl::IntakeArtifact {
            is_not: crate::workflow::gdl::IsNotTable::default(),
            telemetry_refs: vec!["tsr://gradebook".to_string()],
            what_changed: "unknown".to_string(),
            known_good: "none available".to_string(),
            diagnostic_seams: vec![],
            decomposition: None,
        });
        case.triage = Some(crate::workflow::gdl::TriageArtifact {
            priority: "P2".to_string(),
            stabilized: true,
            search_hits: vec!["kb:1".to_string()],
            verdict: "accept".to_string(),
            acuity_band: Some("GREEN".to_string()),
            esi_level: None,
            resource_estimate: None,
            care_setting: Some("self_care".to_string()),
            modality_adequacy: None,
            confirmed_matches: Some(vec!["kb:1".to_string()]),
            red_flag: crate::workflow::gdl::RedFlagArtifact {
                worst_case: "data loss".to_string(),
                ruled_out: serde_json::Value::Bool(true),
                rule_out_basis: vec!["verification:gradebook".to_string()],
                first_would_miss_impact: "loss".to_string(),
            },
        });
        case.hypotheses = vec![crate::workflow::gdl::Hypothesis {
            statement: "s".to_string(),
            prediction: "p".to_string(),
            sources: vec!["verify:a".to_string(), "test:b".to_string()],
            confidence: None,
            rank: Some(1),
        }];
        case.plan = vec![crate::workflow::gdl::PlanStep {
            order: 1,
            kind: "check".to_string(),
            skill_gate: "L1".to_string(),
            description: "open the workbook".to_string(),
            command: "open".to_string(),
            expected: "opens".to_string(),
            fail_action: None,
            seam: None,
            invasiveness: 0,
            justification: None,
        }];
        case.verify_step = Some(crate::workflow::gdl::VerifyStepSpec {
            re_run: "open the workbook".to_string(),
            pass_condition: "opens".to_string(),
        });
        case.dead_end = Some(crate::workflow::gdl::DeadEnd {
            escalate_to: "operator".to_string(),
            required_evidence: vec![],
        });
        case.test_log = vec![TestLogRow {
            order: 1,
            kind: "check".to_string(),
            description: "open the workbook".to_string(),
            playbook_ref: Some("kb:1".to_string()),
            variables: vec!["file".to_string()],
            expected: "opens".to_string(),
            actual: Some("opens".to_string()),
            verdict: Verdict::Pass,
            evidence_ref: Some("verification:gradebook".to_string()),
            dtfvc: Dtfvc {
                diagnose: "d".to_string(),
                test: "t".to_string(),
                fix: None,
                verify: None,
                capture: None,
            },
            invasiveness: 0,
            justification: None,
        }];
        case.verify = Some(crate::workflow::gdl::VerifyArtifact {
            re_run: "open the workbook".to_string(),
            pass_condition: "opens".to_string(),
            pass: true,
            stability_window_min: 5,
            negative_check: true,
            rehypothesis: None,
        });
        case.capture = Some(crate::workflow::gdl::CaptureArtifact {
            resolution: "fixed".to_string(),
            bundle_hash: "h".to_string(),
        });
        case.closure = Some(crate::workflow::gdl::ClosureArtifact {
            decision: "fixed".to_string(),
            communicated_to: vec!["teacher".to_string()],
            shared_decision: true,
            shared_decision_note: None,
            warning_signs: vec!["fails again".to_string()],
            escalation_path: "operator".to_string(),
            follow_up: "".to_string(),
            modality: crate::workflow::gdl::ClosureModality {
                telehealth: false,
                modality_adequacy: None,
            },
            closure_means: "email".to_string(),
        });
        case
    }

    fn pool_with_source() -> r2d2::Pool<SqliteConnectionManager> {
        register_sqlite_vec();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        std::mem::forget(tmp);
        let mgr = SqliteConnectionManager::file(&path);
        let pool: r2d2::Pool<SqliteConnectionManager> =
            r2d2::Pool::builder().max_size(2).build(mgr).unwrap();
        run_migration(&mut pool.get().unwrap(), crate::config::DB_MMAP_SIZE_MIB).unwrap();
        let conn = pool.get().unwrap();
        let source = resolved_source();
        conn.execute(
            "INSERT INTO workflow_runs(domain, kind, state_json, state_revision, status, created_at, updated_at) VALUES ('acme', 'troubleshoot', ?1, 0, 'resolved', 1, 1)",
            params![serde_json::to_string(&source).unwrap()],
        )
        .unwrap();
        pool
    }

    #[test]
    fn seal_carries_no_hypotheses() {
        let source = resolved_source();
        let (mark, snapshot) = seal(&source, 7, &receipt("conf-1"), "resolved").unwrap();
        assert_eq!(mark.procedure_id, 7);
        assert_eq!(snapshot.plan.len(), 1);
        assert_eq!(snapshot.test_log.len(), 1);
        // The snapshot has no hypotheses field by construction: the
        // sealed bytes carry plan plus rows only.
        let bytes = serde_json::to_string(&snapshot).unwrap();
        assert!(!bytes.contains("hypotheses"));
        assert!(!bytes.contains("statement"));
    }

    #[test]
    fn seal_refuses_without_passing_conformance_receipt() {
        let source = resolved_source();
        let mut bad = receipt("conf-1");
        bad.verdict = "skip".to_string();
        let err = seal(&source, 7, &bad, "resolved").unwrap_err();
        assert!(err.0.contains("conformance"), "refused by name: {err}");
        let mut bad = receipt("conf-1");
        bad.verdict = "fail".to_string();
        assert!(seal(&source, 7, &bad, "resolved").is_err());
    }

    #[test]
    fn hand_down_is_exactly_once_per_pair() {
        let pool = pool_with_source();
        let conn = pool.get().unwrap();
        let first = hand_down_in_tx(&conn, 1, &receipt("conf-1")).unwrap();
        assert!(first.is_created());
        let again = hand_down_in_tx(&conn, 1, &receipt("conf-1")).unwrap();
        assert!(!again.is_created());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_session_events WHERE kind='control:handdown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let other = hand_down_in_tx(&conn, 1, &receipt("conf-2")).unwrap_err();
        assert!(
            other.0.contains("one procedure, one mark"),
            "refused by name: {other}"
        );
    }

    #[test]
    fn mutated_source_refuses_the_match_fail_closed() {
        let pool = pool_with_source();
        let conn = pool.get().unwrap();
        let mark = hand_down_in_tx(&conn, 1, &receipt("conf-1"))
            .unwrap()
            .mark()
            .clone();
        assert!(load_snapshot(&conn, &mark).is_ok());
        // Flip one source byte after the mark: the digest no longer
        // binds, so the re-derived snapshot is refused by name.
        let mut source = resolved_source();
        source.plan[0].expected = "mutated".to_string();
        conn.execute(
            "UPDATE workflow_runs SET state_json = ?1 WHERE id = 1",
            params![serde_json::to_string(&source).unwrap()],
        )
        .unwrap();
        let err = load_snapshot(&conn, &mark).unwrap_err();
        assert!(err.0.contains("stale candidate"), "refused by name: {err}");
    }

    #[test]
    fn deviation_names_the_first_mismatched_step() {
        let source = resolved_source();
        let (_mark, snapshot) = seal(&source, 7, &receipt("conf-1"), "resolved").unwrap();
        assert!(harness_deviation(&snapshot.test_log, &snapshot.test_log).is_none());
        let mut delivered = snapshot.test_log.clone();
        delivered[0].actual = Some("does not open".to_string());
        let msg = harness_deviation(&snapshot.test_log, &delivered).unwrap();
        assert!(msg.contains("step 1"), "named: {msg}");
    }

    #[test]
    fn ticket_match_is_casefolded_containment() {
        assert!(ticket_matches(
            "RE: gradebook workbook fails to open today",
            "gradebook workbook fails to open"
        ));
        assert!(!ticket_matches(
            "a different ticket entirely",
            "gradebook workbook fails to open"
        ));
        assert!(!ticket_matches("ab query", "ab"));
    }
}
