//! The κ labeling bench: the instrument that lets the operator labeling
//! round actually happen — blind rater assignment over the disagreement
//! corpus, an exactly-once append-only label store, and the per-rater-pair
//! κ report. Storage + surface only: the bench collects human judgments and
//! reports agreement as data; nothing here feeds a gate, a disposition, or
//! the loop, and the κ value never auto-passes anything.
//!
//! Blindness is a TYPE, not a discipline: a rater's read surface is built
//! from [`QueueTuple`], which carries no `governed_truth` field and no
//! other rater's labels — the compiler refuses the leak, and no core
//! function accepts a "read another slot" parameter. Labels are per-tuple
//! judgments from a CLOSED, operator-ratified vocabulary; the raw case text
//! never enters a payload (digests only, the minimization posture). The
//! frozen train/holdout split is inherited, never re-drawn: every label
//! carries the partition its tuple was mined under, and the report
//! separates the partitions so a bleed is checkable, never silently merged.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::reflection::Partition;
use super::tx::WorkflowTx;

/// The additive session-log kind carrying one rater's judgment on one
/// tuple, written under the TUPLE's run id. Every pre-existing reader
/// filters by kind, so these rows are inert to all of them (the
/// additive-kind law).
pub(crate) const KAPPA_LABEL_KIND: &str = "kappa_label";

/// The audit targets this module writes (one per created label; the
/// report call audits through the surface's `record_tenant`).
pub(crate) const AUDIT_KAPPA_LABEL: &str = "kappa_label";
pub(crate) const AUDIT_KAPPA_REPORT: &str = "kappa_report";

/// The rater roster: exactly two slots, a declared constant — no config,
/// no knob. Two is the κ floor (a pair needs two raters); widening it is a
/// dated operator decision that changes this constant and nothing else.
pub(crate) const RATER_SLOTS: u8 = 2;

/// Every tuple goes to this many slots. Equal to the roster today, so the
/// assignment is total: the pair always shares the same tuple set.
pub(crate) const REQUIRED_RATERS_PER_TUPLE: usize = 2;

/// The closed label vocabulary — the operator-ratified set raters choose
/// from over the tuple's governed truth. Widenable only by a dated
/// addendum; the κ math itself treats labels as opaque strings.
pub(crate) const LABEL_VOCABULARY: &[&str] = &["agree", "disagree", "uncertain"];

/// The acceptance bar as integer ten-thousandths (κ ≥ 0.70). REPORTED AS
/// DATA on the cells — the machine never auto-gates on it.
pub(crate) const KAPPA_BAR_UNITS: i32 = 7000;

/// The conjunction's second bar (Gwet's AC1 ≥ 0.80), the preregistration's
/// terminus rule's other arm. REPORTED AS DATA exactly like
/// [`KAPPA_BAR_UNITS`] — the machine never auto-gates on it.
pub(crate) const AC1_BAR_UNITS: i32 = 8000;

/// The κ sentinel: no signed agreement yet (the SDK calibration
/// convention).
pub(crate) const NO_KAPPA: i32 = -1;

/// The prevalence-flag value the report carries on a skewed cell (the
/// preregistration's flag law, decided at n=0).
pub(crate) const PREVALENCE_SKEWED: &str = "prevalence-skewed";

/// The flag's band edges: a modal prevalence below 20% or above 80% marks
/// the cell `prevalence-skewed` — the region where the paradox literature
/// demonstrates κ's collapse (Feinstein & Cicchetti 1990; Gwet 2008).
/// The >80% arm is the live one for this bench (over q categories the
/// modal share cannot fall below 1/q, so the low arm is latent until the
/// vocabulary ever widens past six labels — a dated addendum).
pub(crate) const PREVALENCE_FLAG_LOW: f64 = 0.20;
pub(crate) const PREVALENCE_FLAG_HIGH: f64 = 0.80;

/// The cell's positive-class prevalence, operationalized (the
/// preregistration §2): the pooled prevalence of the MODAL label — the
/// per-label proportion over BOTH raters' labels on the joint set,
/// maximized. This is the same πₖ the AC1 formula computes, and it is the
/// dominance measure the kappa paradox responds to, whatever the dominant
/// label is named (the vocabulary designates no positive class). Returns
/// the flag value when the modal share leaves the band, `None` when it
/// stays inside or when there are no labels (absence is not skew).
pub(crate) fn prevalence_flag(rater_a: &[String], rater_b: &[String]) -> Option<&'static str> {
    let pooled = rater_a.len() + rater_b.len();
    if pooled == 0 {
        return None;
    }
    let mut labels: Vec<&String> = rater_a.iter().chain(rater_b.iter()).collect();
    labels.sort();
    labels.dedup();
    let mut modal = 0.0f64;
    for label in &labels {
        let count = rater_a.iter().filter(|a| *a == *label).count()
            + rater_b.iter().filter(|b| *b == *label).count();
        modal = modal.max(count as f64 / pooled as f64);
    }
    if modal < PREVALENCE_FLAG_LOW || modal > PREVALENCE_FLAG_HIGH {
        Some(PREVALENCE_SKEWED)
    } else {
        None
    }
}

/// Cohen's κ over label vectors, integer ten-thousandths. Degenerate
/// inputs are named errors, never NaN (the eval_kappa precedent):
/// empty or length-mismatched raters, or a degenerate expected
/// agreement of 1 (both raters constant with the same marginal), are
/// refusals. Promoted verbatim from the test-scoped module it lived in —
/// same implementation, same goldens; a real labeling bench needs it as
/// domain-core API, still pure, still integer-units.
pub(crate) fn cohen_kappa_units(rater_a: &[String], rater_b: &[String]) -> Result<i32, String> {
    if rater_a.is_empty() || rater_b.is_empty() {
        return Err("kappa: no ratings supplied".into());
    }
    if rater_a.len() != rater_b.len() {
        return Err(format!(
            "kappa: rating length mismatch ({} vs {})",
            rater_a.len(),
            rater_b.len()
        ));
    }
    let n = rater_a.len() as f64;
    let observed = rater_a.iter().zip(rater_b).filter(|(a, b)| a == b).count() as f64 / n;
    let mut labels: Vec<&String> = rater_a.iter().chain(rater_b.iter()).collect();
    labels.sort();
    labels.dedup();
    let mut expected = 0.0;
    for label in labels {
        let pa = rater_a.iter().filter(|a| *a == label).count() as f64 / n;
        let pb = rater_b.iter().filter(|b| *b == label).count() as f64 / n;
        expected += pa * pb;
    }
    if expected >= 1.0 {
        return Err("kappa: degenerate — expected agreement 1; κ is undefined".into());
    }
    let kappa = (observed - expected) / (1.0 - expected);
    Ok((kappa * 10_000.0).round() as i32)
}

/// Gwet's AC1 over label vectors, integer ten-thousandths — the second
/// metric of the preregistered conjunction (κ ≥ 0.70 ∧ AC1 ≥ 0.80 ∧ raw
/// agreement recorded; `plans/improvements/KAPPA_METRIC_PREREGISTRATION_2026-11-07.md`,
/// committed before any label landed). Shape-identical to
/// [`cohen_kappa_units`] — `pub(crate)`, `&[String] × &[String] →
/// Result<i32, String>`, ten-thousandths — so the report's two metrics are
/// comparable unit-for-unit (KM-5, pinned structurally in this file).
/// `AC1 = (Pa − Pe(γ)) / (1 − Pe(γ))` with `Pe(γ) = (1/(q−1)) · Σₖ πₖ(1−πₖ)`,
/// q = the distinct pooled labels, πₖ = the average marginal proportion —
/// the q-category form that reproduces the PUBLISHED prevalence-paradox
/// fixture (82/8/8/2: 84% raw agreement → κ 0.11 / AC1 0.80; the binary
/// specialization is Pe = 2π(1−π); the fixture pins both numbers exactly,
/// 1111 / 8049). AC1 stays stable where κ collapses under skewed
/// prevalence — the conjunction's second arm, never a replacement (Vach
/// 2023: the two answer different questions). Degenerate inputs are named
/// errors, never NaN: empty, length-mismatched, or a single category (the
/// q−1 denominator) refuse under the fn's own `ac1:` prefix. Pe(γ) ≤ 0.5
/// for every q, so the denominator cannot vanish — no further degenerate
/// case exists.
pub(crate) fn gwet_ac1_units(rater_a: &[String], rater_b: &[String]) -> Result<i32, String> {
    if rater_a.is_empty() || rater_b.is_empty() {
        return Err("ac1: no ratings supplied".into());
    }
    if rater_a.len() != rater_b.len() {
        return Err(format!(
            "ac1: rating length mismatch ({} vs {})",
            rater_a.len(),
            rater_b.len()
        ));
    }
    let n = rater_a.len() as f64;
    let observed = rater_a.iter().zip(rater_b).filter(|(a, b)| a == b).count() as f64 / n;
    let mut labels: Vec<&String> = rater_a.iter().chain(rater_b.iter()).collect();
    labels.sort();
    labels.dedup();
    if labels.len() < 2 {
        return Err("ac1: degenerate — a single category; AC1 is undefined".into());
    }
    let mut expected = 0.0;
    for label in &labels {
        let pa = rater_a.iter().filter(|a| *a == *label).count() as f64 / n;
        let pb = rater_b.iter().filter(|b| *b == *label).count() as f64 / n;
        let pi = (pa + pb) / 2.0;
        expected += pi * (1.0 - pi);
    }
    let expected = expected / (labels.len() as f64 - 1.0);
    let ac1 = (observed - expected) / (1.0 - expected);
    Ok((ac1 * 10_000.0).round() as i32)
}

/// Char-bound cap, the reflection core's exact shape. `pub(super)`: the
/// agreement module shares THIS copy — the dup_guard pin forbids a second.
pub(super) fn cap_chars(s: &str, cap: usize) -> String {
    s.char_indices()
        .nth(cap)
        .map_or_else(|| s.to_string(), |(idx, _)| s[..idx].to_string())
}

/// The assignment's domain separation: a tuple's slot set derives from
/// this versioned constant, the tuple's run id, and its content digest —
/// never from label state, wall-clock, or config (the frozen-split
/// precedent's shape).
const ASSIGNMENT_DOMAIN: &str = "kappa-assign-v1";

/// The principal→slot derivation's domain separation: a rater's slot is a
/// pure function of their authenticated subject, so the slot never rides
/// the request body (the client names the judgment, not the judge).
const SLOT_DOMAIN: &str = "kappa-slot-v1";

/// The first eight hex digits of a versioned digest, folded base-16 into
/// a number (the `partition_for_run` shape). `pub(super)`: the agreement
/// module shares THIS copy — the dup_guard pin forbids a second.
pub(super) fn fold8(hex: &str) -> u32 {
    let mut acc = 0u32;
    for b in hex.bytes().take(8) {
        let d = match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' => (b - b'a' + 10) as u32,
            _ => 0,
        };
        acc = acc.wrapping_mul(16).wrapping_add(d);
    }
    acc
}

/// Which rater slots label the tuple: exactly
/// [`REQUIRED_RATERS_PER_TUPLE`] DISTINCT slots, deterministic in the
/// tuple alone. Slot i is drawn as `fold8(sha256(domain:run:digest:i))`
/// over the roster; if the draws collide before the quota fills, later
/// non-colliding indices are tried (bounded walk), and any theoretical
/// remainder fills ascending — the function is total and never returns a
/// duplicate.
pub(crate) fn assignment_slots(run_id: i64, digest: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(REQUIRED_RATERS_PER_TUPLE);
    let mut i = 0u64;
    while out.len() < REQUIRED_RATERS_PER_TUPLE && i <= 8 * RATER_SLOTS as u64 {
        let hex = crate::audit::hash(&format!("{ASSIGNMENT_DOMAIN}:{run_id}:{digest}:{i}"));
        let slot = (fold8(&hex) % RATER_SLOTS as u32) as u8;
        if !out.contains(&slot) {
            out.push(slot);
        }
        i += 1;
    }
    let mut fallback = 0u8;
    while out.len() < REQUIRED_RATERS_PER_TUPLE {
        if !out.contains(&fallback) {
            out.push(fallback);
        }
        fallback += 1;
    }
    out.sort_unstable();
    out
}

/// Whether the tuple is assigned to the rater slot — the assignment
/// authority the writer and the queue both consult.
pub(crate) fn assignment_for(run_id: i64, digest: &str, slot: u8) -> bool {
    assignment_slots(run_id, digest).contains(&slot)
}

/// A rater's slot, derived from the authenticated principal's subject.
/// Same subject → same slot, always; the request body never names a slot.
pub(crate) fn slot_for_principal(sub: &str) -> u8 {
    let hex = crate::audit::hash(&format!("{SLOT_DOMAIN}:{sub}"));
    (fold8(&hex) % RATER_SLOTS as u32) as u8
}

/// The label against the closed vocabulary: named refusals, never a
/// silent accept. Pure, total.
pub(crate) fn validate_label(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("kappa: label required".into());
    }
    if raw.len() > 64 {
        return Err(format!("kappa: label invalid: {} chars", raw.len()));
    }
    if !LABEL_VOCABULARY.contains(&raw) {
        return Err(format!("kappa: label invalid: {raw}"));
    }
    Ok(())
}

/// One stored label row's payload shape — exactly the four ratified keys.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct KappaLabelPayload {
    pub digest: String,
    pub label: String,
    pub rater_slot: u8,
    pub partition: String,
}

/// The total parser every stored label payload round-trips through: any
/// input yields the payload or a named `kappa: …` refusal. No I/O, no
/// clock, no panic. Unknown keys refuse — the payload carries exactly the
/// ratified four, never raw case text.
pub(crate) fn parse_kappa_label(value: &serde_json::Value) -> Result<KappaLabelPayload, String> {
    let obj = value
        .as_object()
        .ok_or("kappa: label payload must be a JSON object")?;
    for key in obj.keys() {
        if !matches!(
            key.as_str(),
            "digest" | "label" | "rater_slot" | "partition"
        ) {
            return Err(format!("kappa: unknown field {key}"));
        }
    }
    let digest = obj
        .get("digest")
        .and_then(serde_json::Value::as_str)
        .ok_or("kappa: digest must be a string")?;
    if digest.is_empty() {
        return Err("kappa: digest required".into());
    }
    let label = obj
        .get("label")
        .and_then(serde_json::Value::as_str)
        .ok_or("kappa: label must be a string")?;
    validate_label(label)?;
    let slot_raw = obj
        .get("rater_slot")
        .ok_or("kappa: rater_slot required")?
        .as_u64()
        .ok_or("kappa: rater_slot must be a non-negative integer")?;
    if slot_raw >= RATER_SLOTS as u64 {
        return Err(format!("kappa: rater_slot unknown: {slot_raw}"));
    }
    let partition = obj
        .get("partition")
        .and_then(serde_json::Value::as_str)
        .ok_or("kappa: partition must be a string")?;
    if Partition::parse(partition).is_none() {
        return Err("kappa: partition must be train | holdout".into());
    }
    Ok(KappaLabelPayload {
        digest: digest.to_string(),
        label: label.to_string(),
        rater_slot: slot_raw as u8,
        partition: partition.to_string(),
    })
}

/// One created (or replayed) label write: `created` is false for an
/// exactly-once replay of a known key; `supersession` counts the label
/// rows that already existed for the (tuple, slot) pair (0 = first
/// label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KappaLabelReceipt {
    pub created: bool,
    pub seq: i64,
    pub supersession: i64,
}

/// Whether the tuple exists: a disagreement row mined under the run id
/// carrying exactly this input digest. The digest-keyed enumeration is
/// the authority — a digest that matches nothing is an absent tuple, and
/// the surface answers it with the same probe-blind 404 as an assignment
/// refusal (the pinned equation).
fn tuple_exists(conn: &rusqlite::Connection, run_id: i64, digest: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(
            "SELECT payload_json FROM agent_session_events
              WHERE run_id = ?1 AND kind = ?2 ORDER BY seq",
        )
        .map_err(|e| format!("kappa: tuple read failed: {e}"))?;
    let rows = stmt
        .query_map(
            params![run_id, super::reflection::REFLECTION_DISAGREEMENT_KIND],
            |r| r.get::<_, String>(0),
        )
        .map_err(|e| format!("kappa: tuple read failed: {e}"))?;
    for row in rows {
        let payload_json = row.map_err(|e| format!("kappa: tuple read failed: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| format!("kappa: stored tuple unreadable: {e}"))?;
        if value
            .get("input_digest")
            .and_then(serde_json::Value::as_str)
            == Some(digest)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Write one label: the additive `kappa_label` row under the TUPLE's run
/// id plus its audit row, atomic in the caller's tx. One label per
/// (tuple, slot): a re-submit of the label that is already the rater's
/// latest judgment is the idempotent no-op receipt (nothing appends, the
/// audit logs nothing); a CHANGED judgment computes the next
/// supersession key (`base:{count}`) and appends a NEW row — rows are
/// never mutated, and a rolled-back retry recomputes the same key. The
/// underlying session-log append keeps its own exactly-once guard for the
/// key collision a racing writer could still produce. Refuses a label
/// outside the closed vocabulary and a tuple that does not exist or is
/// not assigned to this slot — the assignment core is the authority, the
/// surface refuses before any write lands.
pub(crate) fn write_label(
    tx: &mut WorkflowTx<'_>,
    run_id: i64,
    digest: &str,
    label: &str,
    slot: u8,
    now: i64,
) -> Result<KappaLabelReceipt, String> {
    validate_label(label)?;
    if slot >= RATER_SLOTS {
        return Err(format!("kappa: rater_slot unknown: {slot}"));
    }
    if !tuple_exists(tx.tx(), run_id, digest)? {
        return Err("label_assignment_absent".into());
    }
    if !assignment_for(run_id, digest, slot) {
        return Err("label_assignment_absent".into());
    }
    let partition = super::reflection::partition_for_run(run_id);
    let payload = serde_json::json!({
        "digest": digest,
        "label": label,
        "rater_slot": slot,
        "partition": partition.as_str(),
    })
    .to_string();
    let base = format!("run{run_id}:{KAPPA_LABEL_KIND}:{digest}:{slot}");
    let glob = format!("{base}:*");
    let existing: i64 = tx
        .tx()
        .query_row(
            "SELECT COUNT(*) FROM agent_session_events
              WHERE run_id = ?1 AND kind = ?2 AND (idempotency_key = ?3 OR idempotency_key GLOB ?4)",
            params![run_id, KAPPA_LABEL_KIND, base, glob],
            |r| r.get(0),
        )
        .map_err(|e| format!("kappa: label count read failed: {e}"))?;
    if existing > 0 {
        // Latest-wins idempotence: the SAME judgment re-submitted is the
        // no-op receipt — only a changed judgment earns a new row.
        let (latest_seq, latest_payload): (i64, String) = tx
            .tx()
            .query_row(
                "SELECT seq, payload_json FROM agent_session_events
                  WHERE run_id = ?1 AND kind = ?2 AND (idempotency_key = ?3 OR idempotency_key GLOB ?4)
                  ORDER BY seq DESC LIMIT 1",
                params![run_id, KAPPA_LABEL_KIND, base, glob],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("kappa: label read failed: {e}"))?
            .ok_or_else(|| "kappa: label read failed: count and rows disagree".to_string())?;
        if latest_payload == payload {
            return Ok(KappaLabelReceipt {
                created: false,
                seq: latest_seq,
                supersession: existing,
            });
        }
    }
    let key = if existing == 0 {
        base
    } else {
        format!("{base}:{existing}")
    };
    let (created, seq) =
        super::session_log::append(tx.tx(), run_id, KAPPA_LABEL_KIND, &payload, &key, now)
            .map_err(|e| format!("kappa: label write failed: {e}"))?;
    if created {
        super::audit_write(
            tx.tx(),
            run_id,
            AUDIT_KAPPA_LABEL,
            crate::audit::AuditStatus::Ok,
            &format!(
                "digest={digest} slot={slot} seq={seq} supersession={existing} partition={}",
                partition.as_str()
            ),
        );
    }
    Ok(KappaLabelReceipt {
        created,
        seq,
        supersession: existing,
    })
}

/// One tuple as a rater sees it: the mined content (masked through the
/// read seam, the machine's governed truth ABSENT by type), the frozen
/// partition, and — never anyone else's — the rater's own latest label.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct QueueTuple {
    pub run_id: i64,
    pub seq: i64,
    pub digest: String,
    pub phase: String,
    pub model_proposal: String,
    pub partition: Partition,
    pub my_label: Option<String>,
    pub my_label_seq: Option<i64>,
}

/// The rater's own latest label on a tuple, if any: the max-seq
/// `kappa_label` row for (run, digest, slot). A superseded label row is
/// history, never the answer.
fn own_latest_label(
    conn: &rusqlite::Connection,
    run_id: i64,
    digest: &str,
    slot: u8,
) -> Result<Option<(String, i64)>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT seq, payload_json FROM agent_session_events
              WHERE run_id = ?1 AND kind = ?2 ORDER BY seq",
        )
        .map_err(|e| format!("kappa: label read failed: {e}"))?;
    let rows = stmt
        .query_map(params![run_id, KAPPA_LABEL_KIND], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| format!("kappa: label read failed: {e}"))?;
    let mut latest: Option<(String, i64)> = None;
    for row in rows {
        let (seq, payload_json) = row.map_err(|e| format!("kappa: label read failed: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| format!("kappa: stored label unreadable: {e}"))?;
        let payload = parse_kappa_label(&value)?;
        if payload.digest == digest && payload.rater_slot == slot {
            latest = Some((payload.label, seq));
        }
    }
    Ok(latest)
}

/// The rater's own assignment queue: tuples assigned to THIS slot (both
/// partitions — the round collects on train and holdout alike), oldest
/// first, bounded by the caller's page. The machine's answer is not in
/// the rows; another rater's answer is not in the reader.
pub(crate) fn rater_queue(
    conn: &rusqlite::Connection,
    slot: u8,
    limit: usize,
) -> Result<Vec<QueueTuple>, String> {
    if slot >= RATER_SLOTS {
        return Err(format!("kappa: rater_slot unknown: {slot}"));
    }
    let mut stmt = conn
        .prepare(
            "SELECT run_id, seq, payload_json FROM agent_session_events
              WHERE kind = ?1 ORDER BY run_id, seq",
        )
        .map_err(|e| format!("kappa: queue read failed: {e}"))?;
    let rows = stmt
        .query_map(
            params![super::reflection::REFLECTION_DISAGREEMENT_KIND],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(|e| format!("kappa: queue read failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        if out.len() >= limit {
            break;
        }
        let (run_id, seq, payload_json) =
            row.map_err(|e| format!("kappa: queue read failed: {e}"))?;
        let value: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| format!("kappa: stored tuple unreadable: {e}"))?;
        let digest = value
            .get("input_digest")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "kappa: stored tuple unreadable: missing input_digest".to_string())?
            .to_string();
        if !assignment_for(run_id, &digest, slot) {
            continue;
        }
        let phase = value
            .get("phase")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let proposal_raw = value
            .get("model_proposal")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        // The read seam runs as a synthetic scope-less reader: PII
        // masking is unconditional, no caller's clearance can bypass it.
        let model_proposal = cap_chars(
            &super::reflection::sanitize_seam(proposal_raw),
            super::reflection::MODEL_PROPOSAL_CAP,
        );
        let partition = super::reflection::partition_for_run(run_id);
        let own = own_latest_label(conn, run_id, &digest, slot)?;
        let (my_label, my_label_seq) = own.map_or((None, None), |(l, s)| (Some(l), Some(s)));
        out.push(QueueTuple {
            run_id,
            seq,
            digest,
            phase,
            model_proposal,
            partition,
            my_label,
            my_label_seq,
        });
    }
    Ok(out)
}

/// One agreement cell: a slot pair's κ AND Gwet's AC1 AND the raw
/// agreement AND the prevalence flag over the tuples both slots labeled
/// inside one (domain, partition) group — the preregistered metric SUITE,
/// emitted together, never one number alone (KM-2). A degenerate pair
/// names itself — `NO_KAPPA` in the failed metrics' own fields plus each
/// function's own refusal string — and never pretends to agree.
/// `meets_bar` is data for the operator's read, never a gate input.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct KappaCell {
    pub domain: String,
    pub partition: String,
    pub slot_a: u8,
    pub slot_b: u8,
    pub n_joint: usize,
    pub kappa_units: i32,
    pub kappa_note: Option<String>,
    pub meets_bar: bool,
    pub ac1_units: i32,
    pub ac1_note: Option<String>,
    pub raw_agreement_units: i32,
    pub prevalence_flag: Option<String>,
}

/// The κ report: every slot pair × every (domain, partition) group over
/// the labeled set, latest-wins (superseded rows are history, never
/// votes). Deterministic cell order: domain, partition, slot pair. Every
/// cell carries the preregistered metric SUITE together — κ, Gwet's AC1,
/// the raw agreement, the prevalence flag — never one number alone (KM-2);
/// the sign-form reads these cells side by side when the operator signs.
pub(crate) fn kappa_report(conn: &rusqlite::Connection) -> Result<Vec<KappaCell>, String> {
    // Latest label per (run, digest, slot).
    let mut latest: std::collections::BTreeMap<(i64, String, u8), (String, i64)> =
        std::collections::BTreeMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT run_id, seq, payload_json FROM agent_session_events
                  WHERE kind = ?1 ORDER BY run_id, seq",
            )
            .map_err(|e| format!("kappa: report read failed: {e}"))?;
        let rows = stmt
            .query_map(params![KAPPA_LABEL_KIND], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("kappa: report read failed: {e}"))?;
        for row in rows {
            let (run_id, seq, payload_json) =
                row.map_err(|e| format!("kappa: report read failed: {e}"))?;
            let value: serde_json::Value = serde_json::from_str(&payload_json)
                .map_err(|e| format!("kappa: stored label unreadable: {e}"))?;
            let payload = parse_kappa_label(&value)?;
            let key = (run_id, payload.digest.clone(), payload.rater_slot);
            latest
                .entry(key)
                .and_modify(|cur| {
                    if seq >= cur.1 {
                        cur.0 = payload.label.clone();
                        cur.1 = seq;
                    }
                })
                .or_insert_with(|| (payload.label, seq));
        }
    }
    // Domain per run (total: an unattributable run names itself).
    let mut domains: std::collections::BTreeMap<i64, String> = std::collections::BTreeMap::new();
    for (run_id, _, _) in latest.keys() {
        if domains.contains_key(run_id) {
            continue;
        }
        let domain: Option<String> = conn
            .query_row(
                "SELECT domain FROM workflow_runs WHERE id = ?1",
                params![run_id],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(other),
            })
            .map_err(|e| format!("kappa: report domain read failed: {e}"))?;
        domains.insert(
            *run_id,
            domain.unwrap_or_else(|| "(unattributed)".to_string()),
        );
    }
    // Group tuples: (domain, partition) → set of (run, digest).
    let mut groups: std::collections::BTreeMap<(String, String), Vec<(i64, String)>> =
        std::collections::BTreeMap::new();
    for (run_id, digest, _) in latest.keys() {
        let partition = super::reflection::partition_for_run(*run_id);
        groups
            .entry((domains[run_id].clone(), partition.as_str().to_string()))
            .or_default()
            .push((*run_id, digest.clone()));
    }
    let mut cells = Vec::new();
    for ((domain, partition), mut tuples) in groups {
        tuples.sort();
        tuples.dedup();
        for a in 0..RATER_SLOTS {
            for b in (a + 1)..RATER_SLOTS {
                let mut va: Vec<String> = Vec::new();
                let mut vb: Vec<String> = Vec::new();
                for (run_id, digest) in &tuples {
                    let la = latest.get(&(*run_id, digest.clone(), a));
                    let lb = latest.get(&(*run_id, digest.clone(), b));
                    if let (Some(la), Some(lb)) = (la, lb) {
                        va.push(la.0.clone());
                        vb.push(lb.0.clone());
                    }
                }
                let n_joint = va.len();
                let (kappa_units, kappa_note, meets) = cohen_kappa_units(&va, &vb)
                    .map(|units| (units, None, units >= KAPPA_BAR_UNITS))
                    .unwrap_or_else(|reason| (NO_KAPPA, Some(reason), false));
                let (ac1_units, ac1_note) = gwet_ac1_units(&va, &vb)
                    .map(|units| (units, None))
                    .unwrap_or_else(|reason| (NO_KAPPA, Some(reason)));
                // The raw agreement the two metrics observe: NO_KAPPA when
                // nothing is jointly labeled (absence is never dressed up
                // as 0% or 100% — the K-1 posture).
                let raw_agreement_units = if n_joint == 0 {
                    NO_KAPPA
                } else {
                    let agrees = va.iter().zip(&vb).filter(|(a, b)| a == b).count();
                    ((agrees as f64 / n_joint as f64) * 10_000.0).round() as i32
                };
                cells.push(KappaCell {
                    domain: domain.clone(),
                    partition: partition.clone(),
                    slot_a: a,
                    slot_b: b,
                    n_joint,
                    kappa_units,
                    kappa_note,
                    meets_bar: meets,
                    ac1_units,
                    ac1_note,
                    raw_agreement_units,
                    prevalence_flag: prevalence_flag(&va, &vb).map(str::to_string),
                });
            }
        }
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::run_migration;
    use crate::register_sqlite_vec::register_sqlite_vec;
    use rusqlite::Connection;

    fn db() -> Connection {
        register_sqlite_vec();
        let mut conn = Connection::open_in_memory().unwrap();
        run_migration(&mut conn, 1).unwrap();
        conn
    }

    fn tx(conn: &mut Connection) -> WorkflowTx<'_> {
        WorkflowTx::begin(conn).unwrap()
    }

    fn hex_digest(seed: &str) -> String {
        crate::audit::hash(seed)
    }

    /// Seed one disagreement tuple under `run_id` (the row the GDL close
    /// seam would have mined), plus the workflow_runs row the report's
    /// domain join reads.
    fn seed_tuple(conn: &Connection, run_id: i64, domain: &str, seed: &str, phase: &str) -> String {
        conn.execute(
            "INSERT INTO workflow_runs(id, domain, kind, state_json, state_revision, status, created_at, updated_at)
             VALUES (?1, ?2, 'troubleshoot', '{}', 0, 'active', 1, 1)
             ON CONFLICT(id) DO NOTHING",
            params![run_id, domain],
        )
        .unwrap();
        let digest = hex_digest(seed);
        let payload = serde_json::json!({
            "input_digest": digest,
            "model_proposal": format!("proposal {seed}"),
            "governed_truth": "L9 no search hit",
            "phase": phase,
        })
        .to_string();
        conn.execute(
            "INSERT INTO agent_session_events(run_id, seq, idempotency_key, kind, payload_json, created_at)
             VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM agent_session_events WHERE run_id = ?1), ?2, ?3, ?4, 1)",
            params![run_id, format!("run{run_id}:seed:{seed}"), super::super::reflection::REFLECTION_DISAGREEMENT_KIND, payload],
        )
        .unwrap();
        digest
    }

    fn submit(
        conn: &mut Connection,
        run_id: i64,
        digest: &str,
        label: &str,
        slot: u8,
    ) -> KappaLabelReceipt {
        let mut wtx = tx(conn);
        let receipt = write_label(&mut wtx, run_id, digest, label, slot, 100).unwrap();
        wtx.commit().unwrap();
        receipt
    }

    #[test]
    fn kappa_units_goldens_survive_the_promotion() {
        let v: fn(&[&str]) -> Vec<String> = |xs| xs.iter().map(|s| s.to_string()).collect();
        // Perfect agreement: κ = 1 → 10000.
        let a = v(&["x", "y", "x"]);
        let b = v(&["x", "y", "x"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 10_000);
        // The classic hand case: the promoted implementation keeps the
        // hand-computed 1/4 → 2500.
        let a = v(&["y", "y", "n", "y", "n", "y"]);
        let b = v(&["y", "y", "y", "y", "n", "n"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 2_500);
        // Chance agreement → 0: observed equals expected.
        let a = v(&["x", "y", "x", "y"]);
        let b = v(&["x", "y", "y", "x"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 0);
        // Degenerate: same constant marginal → named error, never NaN.
        let a = v(&["x", "x", "x"]);
        let b = v(&["x", "x", "x"]);
        assert!(cohen_kappa_units(&a, &b).is_err());
        // Degenerate: opposite constants (observed 0, expected 0) → κ = 0.
        let a = v(&["x", "x"]);
        let b = v(&["y", "y"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 0);
        // Perfect disagreement with balanced marginals → κ = −1 → −10000.
        let a = v(&["x", "x", "y", "y"]);
        let b = v(&["y", "y", "x", "x"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), -10_000);
        // NO_KAPPA is the -1 sentinel … which collides with the perfect
        // disagreement value in units; the sentinel is a field default,
        // never a computed κ, so the vocabulary stays honest.
        assert_eq!(NO_KAPPA, -1);
        // Named refusals.
        assert!(cohen_kappa_units(&[], &[]).is_err());
        assert!(cohen_kappa_units(&["x".into()], &["x".into(), "y".into()]).is_err());
    }

    /// The published prevalence-paradox fixture (KM-1): the classic Gwet-
    /// literature illustration — n=100, both raters 90% "yes" marginals,
    /// table 82 both-yes / 8 A-yes-B-no / 8 A-no-B-yes / 2 both-no → raw
    /// agreement 84%, published κ = 0.11, AC1 = 0.80. The implementation is
    /// pinned to THESE numbers, not invented ones. Exact ten-thousandths
    /// (the preregistration doc's §3 arithmetic): Pe(κ) = 0.9·0.9 + 0.1·0.1
    /// = 0.82, κ = 0.02/0.18 = 1/9 → 1111; π_yes = 0.9, π_no = 0.1, Pe(γ) =
    /// (0.09 + 0.09)/(2−1) = 0.18, AC1 = 0.66/0.82 = 33/41 → 8049. A
    /// formula that misses either number on this table is wrong, whatever
    /// it claims (the transcribed Pe = 2γ(1−γ) reading yields 7730 — the
    /// fixture is the arbiter).
    #[test]
    fn gwet_ac1_matches_the_published_paradox_example() {
        let mut a: Vec<String> = Vec::with_capacity(100);
        let mut b: Vec<String> = Vec::with_capacity(100);
        for i in 0..100 {
            let (la, lb) = match i {
                0..=81 => ("yes", "yes"), // 82 both yes
                82..=89 => ("yes", "no"), // 8 A yes / B no
                90..=97 => ("no", "yes"), // 8 A no / B yes
                _ => ("no", "no"),        // 2 both no — raw agreement 84%
            };
            a.push(la.into());
            b.push(lb.into());
        }
        // The κ arm of the same fixture (the published 0.11): the EXISTING
        // implementation already carries it — the pin binds both metrics to
        // one published table.
        assert_eq!(
            cohen_kappa_units(&a, &b).unwrap(),
            1_111,
            "the published κ 0.11 (1/9) on the paradox table"
        );
        // The AC1 arm (the published 0.80): 33/41 → 8049.
        assert_eq!(
            gwet_ac1_units(&a, &b).unwrap(),
            8_049,
            "the published AC1 0.80 (33/41) on the same table"
        );
    }

    /// Parallel goldens over the κ fn's own cases — same vectors, both
    /// metrics, so the shapes stay comparable (KM-5's behavioral arm) and
    /// the "different questions" claim is concrete. Hand arithmetic, all
    /// verified: a=[p,p,q] vs b=[p,q,q] (TWO distinct labels): observed
    /// 2/3, Pe(κ) = 4/9 → κ 4000; π_p = π_q = 0.5, Pe(γ) = 0.5/(2−1) →
    /// AC1 = 1/3 → 3333. The TRUE three-class case a=[p,p,r] vs b=[p,r,s]:
    /// observed 1/3, Pe(κ) = 1/3 → κ 0; π = (1/2, 1/3, 1/6), Σπ(1−π) =
    /// 11/18, Pe(γ) = (11/18)/(3−1) = 11/36 → AC1 = (1/36)/(25/36) = 1/25
    /// → 400 — the only case that
    /// exercises the q−1 divisor (at q = 2 it is 1, invisible to a mutant).
    /// Opposite constants: κ 0 (observed and expected both 0) but AC1 −1
    /// (Gwet's Pe(γ) = 0.5 is NOT 0) — the divergence is the point, both
    /// are recorded as data.
    #[test]
    fn gwet_ac1_goldens_track_the_kappa_goldens() {
        let v: fn(&[&str]) -> Vec<String> = |xs| xs.iter().map(|s| s.to_string()).collect();
        // Perfect agreement → 10000 for both.
        let a = v(&["x", "y", "x"]);
        assert_eq!(gwet_ac1_units(&a, &a).unwrap(), 10_000);
        // Chance agreement → 0 for both (observed 0.5 = Pe(γ) 0.5).
        let a = v(&["x", "y", "x", "y"]);
        let b = v(&["x", "y", "y", "x"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 0);
        assert_eq!(gwet_ac1_units(&a, &b).unwrap(), 0);
        // Perfect disagreement, balanced marginals → −10000 for both.
        let a = v(&["x", "x", "y", "y"]);
        let b = v(&["y", "y", "x", "x"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), -10_000);
        assert_eq!(gwet_ac1_units(&a, &b).unwrap(), -10_000);
        // The two-class divergence: same vectors, different questions.
        let a = v(&["p", "p", "q"]);
        let b = v(&["p", "q", "q"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 4_000);
        assert_eq!(gwet_ac1_units(&a, &b).unwrap(), 3_333);
        // The TRUE three-class case: the q−1 divisor is load-bearing here
        // (a mutant dropping it reds THIS arm and no other). Hand
        // arithmetic: observed 1/3, Pe(κ) = 1/3 → κ 0; π = (1/2, 1/3, 1/6),
        // Σπ(1−π) = 11/18, Pe(γ) = (11/18)/(3−1) = 11/36,
        // AC1 = (1/36)/(25/36) = 1/25 → 400.
        let a = v(&["p", "p", "r"]);
        let b = v(&["p", "r", "s"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 0);
        assert_eq!(gwet_ac1_units(&a, &b).unwrap(), 400);
        // Opposite constants: κ 0, AC1 −1 — AC1 names the total disagreement
        // κ's chance-model absorbs; the report shows both, never one.
        let a = v(&["x", "x"]);
        let b = v(&["y", "y"]);
        assert_eq!(cohen_kappa_units(&a, &b).unwrap(), 0);
        assert_eq!(gwet_ac1_units(&a, &b).unwrap(), -10_000);
    }

    /// The refusals match `cohen_kappa_units`' named-error shape — the
    /// degenerate pair names itself, never NaN (the degenerate-pair
    /// pattern's fn-level arm; the report cell carries the same strings in
    /// its notes). Every input κ refuses, AC1 refuses too, under its own
    /// `ac1:` prefix; the single-category case is AC1's own degenerate
    /// (the q−1 denominator), refused even where κ computes.
    #[test]
    fn gwet_ac1_refusals_match_cohen_kappa_named() {
        let v: fn(&[&str]) -> Vec<String> = |xs| xs.iter().map(|s| s.to_string()).collect();
        // Empty: the same refusal shape, AC1's own prefix.
        let err = gwet_ac1_units(&[], &[]).unwrap_err();
        assert_eq!(err, "ac1: no ratings supplied");
        // Length mismatch: the same refusal shape, AC1's own prefix.
        let err = gwet_ac1_units(&["x".into()], &["x".into(), "y".into()]).unwrap_err();
        assert_eq!(err, "ac1: rating length mismatch (1 vs 2)");
        // Single category (both raters constant, same label): κ refuses as
        // "expected agreement 1"; AC1 refuses as its own degenerate — the
        // q−1 denominator.
        let a = v(&["x", "x", "x"]);
        assert!(cohen_kappa_units(&a, &a).is_err());
        let err = gwet_ac1_units(&a, &a).unwrap_err();
        assert_eq!(err, "ac1: degenerate — a single category; AC1 is undefined");
        // The name carries the metric: no AC1 refusal answers as "kappa:".
        assert!(err.starts_with("ac1: "));
    }

    /// KM-5 (structural): `gwet_ac1_units`' signature is shape-identical to
    /// `cohen_kappa_units`' — same parameter list, same `Result<i32, String>`
    /// return — the comparability law read off the source itself, so a unit
    /// drift (f64 return, swapped params, a differing shape) reds here
    /// before it can make the report's two metrics incomparable.
    #[test]
    fn gwet_ac1_signature_matches_cohen_kappa_units_shape() {
        const SOURCE: &str = include_str!("kappa.rs");
        let sig = |name: &str| {
            let line = SOURCE
                .lines()
                .find(|l| l.contains(&format!("pub(crate) fn {name}(")))
                .unwrap_or_else(|| panic!("the source must declare pub(crate) fn {name}"));
            line.trim()
                .split_once(&format!("fn {name}"))
                .map(|(_, rest)| rest)
                .unwrap()
                .to_string()
        };
        assert_eq!(
            sig("cohen_kappa_units"),
            sig("gwet_ac1_units"),
            "the two metrics' signatures must be identical modulo the name \
             (the comparability law): (rater_a: &[String], rater_b: &[String]) -> Result<i32, String>"
        );
    }

    #[test]
    fn kappa_assignment_is_deterministic_and_blind() {
        let run = 7;
        let digest = hex_digest("case-a");
        let first = assignment_slots(run, &digest);
        // Pure in the tuple: repeated calls agree.
        for _ in 0..8 {
            assert_eq!(assignment_slots(run, &digest), first);
        }
        // The membership view agrees with the slot set.
        for slot in 0..RATER_SLOTS {
            assert_eq!(assignment_for(run, &digest, slot), first.contains(&slot));
        }
        // The slot derivation is pure in the subject and body-free: same
        // subject → same slot, and every derived slot stays in the
        // roster. (Two subjects MAY collide onto one slot — a hash, not a
        // registry; the operator picks raters whose slots differ, and the
        // queue echo makes the slot discoverable.)
        assert_eq!(
            slot_for_principal("user:rater-1"),
            slot_for_principal("user:rater-1")
        );
        assert!(slot_for_principal("user:rater-1") < RATER_SLOTS);
        assert!(slot_for_principal("user:rater-2") < RATER_SLOTS);
    }

    #[test]
    fn kappa_assignment_slots_are_disjoint_and_complete() {
        // Every tuple: exactly the required number of DISTINCT in-roster
        // slots; with the ratified two-slot roster the assignment is total
        // (the pair always shares the tuple set).
        for run in 0..64 {
            for n in 0..16 {
                let digest = hex_digest(format!("sweep-{run}-{n}").as_str());
                let slots = assignment_slots(run, &digest);
                assert_eq!(slots.len(), REQUIRED_RATERS_PER_TUPLE);
                let mut sorted = slots.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), slots.len(), "slots repeat for {run}/{digest}");
                assert!(slots.iter().all(|s| *s < RATER_SLOTS));
                for slot in 0..RATER_SLOTS {
                    assert!(
                        assignment_for(run, &digest, slot),
                        "two-slot roster is total: {run}/{digest}/slot {slot}"
                    );
                }
            }
        }
    }

    #[test]
    fn kappa_label_row_is_exactly_once_and_append_only() {
        let mut conn = db();
        let digest = seed_tuple(&conn, 11, "acme", "case-x", "C2");
        let first = submit(&mut conn, 11, &digest, "agree", 0);
        assert!(first.created);
        assert_eq!(first.supersession, 0);
        // The exactly-once replay: an uncommitted retry recomputes the
        // SAME key (the count is unchanged) and replays as the no-op
        // receipt, never a second row.
        let replay = submit(&mut conn, 11, &digest, "agree", 0);
        assert!(!replay.created);
        assert_eq!(replay.seq, first.seq);
        assert_eq!(replay.supersession, 1);
        // A correction appends a NEW row; the first row stays byte-for-byte.
        let correction = submit(&mut conn, 11, &digest, "disagree", 0);
        assert!(correction.created);
        assert_eq!(correction.supersession, 1);
        assert!(correction.seq > first.seq);
        // A re-submit of the ORIGINAL payload after the correction is a
        // NEW judgment event (the count has moved; the law's own
        // consequence): it appends, it never resurrects the old row.
        let again = submit(&mut conn, 11, &digest, "agree", 0);
        assert!(again.created);
        assert_eq!(again.supersession, 2);
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_session_events WHERE run_id = 11 AND kind = ?1",
                params![KAPPA_LABEL_KIND],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 3,
            "append-only: one base + one correction + one re-submit"
        );
    }

    #[test]
    fn kappa_label_refuses_unassigned_tuple() {
        let mut conn = db();
        // An absent tuple: no disagreement row carries the digest.
        let mut wtx = tx(&mut conn);
        let err = write_label(&mut wtx, 42, &hex_digest("ghost"), "agree", 0, 100).unwrap_err();
        assert_eq!(err, "label_assignment_absent");
        // A slot outside the roster refuses before any read.
        let err = write_label(&mut wtx, 42, &hex_digest("ghost"), "agree", 9, 100).unwrap_err();
        assert!(err.starts_with("kappa: rater_slot unknown"));
        wtx.commit().unwrap();
        // Nothing landed.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_session_events WHERE kind = ?1",
                params![KAPPA_LABEL_KIND],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "refusals write nothing");
    }

    #[test]
    fn kappa_label_digest_only_never_raw_text() {
        let mut conn = db();
        let digest = seed_tuple(&conn, 12, "acme", "case-y", "C3");
        submit(&mut conn, 12, &digest, "uncertain", 1);
        let payload_json: String = conn
            .query_row(
                "SELECT payload_json FROM agent_session_events WHERE run_id = 12 AND kind = ?1",
                params![KAPPA_LABEL_KIND],
                |r| r.get(0),
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&payload_json).unwrap();
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        assert_eq!(keys.len(), 4);
        for key in ["digest", "label", "rater_slot", "partition"] {
            assert!(keys.contains(&key), "missing {key}: {payload_json}");
        }
        // The digest is the stored reference — 64 hex chars, and the raw
        // case text ("case-y") appears nowhere in the payload.
        assert_eq!(value["digest"], serde_json::json!(digest));
        assert_eq!(digest.len(), 64);
        assert!(
            !payload_json.contains("case-y"),
            "raw seed text leaked: {payload_json}"
        );
        assert!(
            !payload_json.contains("governed"),
            "the machine's answer never rides a label row"
        );
    }

    #[test]
    fn kappa_report_separates_the_frozen_partition() {
        // Find one train run and one holdout run (the split is frozen).
        let mut train_run = None;
        let mut holdout_run = None;
        for r in 0..10_000 {
            match super::super::reflection::partition_for_run(r) {
                Partition::Train => {
                    if train_run.is_none() {
                        train_run = Some(r);
                    }
                }
                Partition::Holdout => {
                    if holdout_run.is_none() {
                        holdout_run = Some(r);
                    }
                }
            }
            if train_run.is_some() && holdout_run.is_some() {
                break;
            }
        }
        let (train_run, holdout_run) = (
            train_run.expect("a train run exists in the sweep"),
            holdout_run.expect("a holdout run exists in the sweep"),
        );
        assert_ne!(train_run, holdout_run);
        let mut conn = db();
        // Each partition gets the same non-degenerate pair of tuples:
        // agree/agree and agree/disagree → κ = 0 inside the cell.
        let t1 = seed_tuple(&conn, train_run, "acme", "case-train-1", "C2");
        let t2 = seed_tuple(&conn, train_run, "acme", "case-train-2", "C3");
        submit(&mut conn, train_run, &t1, "agree", 0);
        submit(&mut conn, train_run, &t1, "agree", 1);
        submit(&mut conn, train_run, &t2, "agree", 0);
        submit(&mut conn, train_run, &t2, "disagree", 1);
        let h1 = seed_tuple(&conn, holdout_run, "acme", "case-hold-1", "C2");
        let h2 = seed_tuple(&conn, holdout_run, "acme", "case-hold-2", "C3");
        submit(&mut conn, holdout_run, &h1, "agree", 0);
        submit(&mut conn, holdout_run, &h1, "agree", 1);
        submit(&mut conn, holdout_run, &h2, "agree", 0);
        submit(&mut conn, holdout_run, &h2, "disagree", 1);
        let cells = kappa_report(&conn).unwrap();
        // One cell per partition (one pair over one domain), never merged.
        let acme: Vec<&KappaCell> = cells.iter().filter(|c| c.domain == "acme").collect();
        assert_eq!(
            acme.len(),
            2,
            "train and holdout render as separate cells: {acme:?}"
        );
        assert!(acme.iter().any(|c| c.partition == "train"));
        assert!(acme.iter().any(|c| c.partition == "holdout"));
        for cell in &acme {
            assert_eq!(
                cell.n_joint, 2,
                "both tuples of the cell are jointly labeled"
            );
            assert_eq!(cell.kappa_units, 0, "agree/agree + agree/disagree → κ = 0");
            assert!(!cell.meets_bar);
        }
    }

    #[test]
    fn kappa_report_reads_latest_supersession_only() {
        let mut conn = db();
        // Three joint tuples: slot0 = [a, a, d] vs slot1 = [a, d, d]
        // (one differing position): observed 2/3, expected 4/9 → κ = 4000.
        let d1 = seed_tuple(&conn, 21, "acme", "case-z1", "C2");
        let d2 = seed_tuple(&conn, 21, "acme", "case-z2", "C2");
        let d3 = seed_tuple(&conn, 21, "acme", "case-z3", "C2");
        submit(&mut conn, 21, &d1, "agree", 0);
        submit(&mut conn, 21, &d1, "agree", 1);
        submit(&mut conn, 21, &d2, "agree", 0);
        submit(&mut conn, 21, &d2, "disagree", 1);
        submit(&mut conn, 21, &d3, "disagree", 0);
        submit(&mut conn, 21, &d3, "disagree", 1);
        let before = kappa_report(&conn).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].kappa_units, 4_000);
        assert_eq!(before[0].n_joint, 3);
        // Supersede slot 0's vote on the FIRST tuple: latest-wins makes
        // slot0 = [d, a, d] vs slot1 = [a, d, d] → κ = −5000. Counting
        // the superseded row instead would keep 2500 — the flip is the
        // proof.
        submit(&mut conn, 21, &d1, "disagree", 0);
        let cells = kappa_report(&conn).unwrap();
        assert_eq!(cells.len(), 1);
        let cell = &cells[0];
        assert_eq!(cell.n_joint, 3);
        assert_eq!(cell.kappa_units, -5_000, "the latest rows are the votes");
        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_session_events WHERE run_id = 21 AND kind = ?1",
                params![KAPPA_LABEL_KIND],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 7, "six first labels + one supersession row");
    }

    #[test]
    fn fuzz_corpus_replays_kappa_label_parser() {
        let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir.push("crates/brain-fuzz/corpus/kappa");
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("corpus dir: {e}"));
        let mut count = 0;
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let text = String::from_utf8_lossy(&bytes);
            let v: serde_json::Value =
                serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
            match parse_kappa_label(&v) {
                Ok(payload) => {
                    assert!(
                        crate::workflow::kappa::LABEL_VOCABULARY.contains(&payload.label.as_str()),
                        "a parsed label stays inside the closed vocabulary"
                    );
                }
                Err(e) => assert!(e.starts_with("kappa: "), "unnamed error: {e}"),
            }
            // The assignment fold is total over the same bytes.
            let run_id = v
                .get("run_id")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let digest = v
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let slots = assignment_slots(run_id, digest);
            assert_eq!(slots.len(), REQUIRED_RATERS_PER_TUPLE);
            assert!(slots.iter().all(|s| *s < RATER_SLOTS));
            count += 1;
        }
        assert!(count >= 10, "the κ corpus must stay populated");
    }

    #[test]
    fn kappa_report_on_empty_corpus_emits_zero_cells_never_a_number() {
        // K-1: at zero labels the report emits NO_KAPPA cells or zero cells —
        // never a fabricated number. S0 measured 0 labels: zero cells here.
        let conn = db();
        let cells = kappa_report(&conn).unwrap();
        assert!(
            cells.is_empty(),
            "an empty corpus names itself with no cells"
        );
        assert!(
            cells.iter().all(|c| c.kappa_units == NO_KAPPA),
            "no measured value may appear at n=0"
        );
    }

    #[test]
    fn kappa_degenerate_pair_names_itself() {
        let mut conn = db();
        let digest = seed_tuple(&conn, 31, "acme", "case-d", "C2");
        // Only slot 0 labels: the pair is degenerate and must say so.
        submit(&mut conn, 31, &digest, "agree", 0);
        let cells = kappa_report(&conn).unwrap();
        assert_eq!(cells.len(), 1);
        let cell = &cells[0];
        assert_eq!(cell.n_joint, 0);
        assert_eq!(cell.kappa_units, NO_KAPPA);
        let note = cell.kappa_note.as_deref().unwrap();
        assert!(
            note.contains("no ratings supplied"),
            "the κ fn's own refusal: {note}"
        );
        assert!(!cell.meets_bar);
    }

    /// The degenerate pair names BOTH metrics (KM-2's degenerate arm): the
    /// AC1 field carries NO_KAPPA plus its own refusal string, the raw
    /// agreement carries NO_KAPPA (absence is never dressed up as a
    /// percentage), and no prevalence is claimed (absence is not skew).
    #[test]
    fn kappa_degenerate_pair_names_both_metrics() {
        let mut conn = db();
        let digest = seed_tuple(&conn, 32, "acme", "case-d2", "C2");
        submit(&mut conn, 32, &digest, "agree", 0);
        let cells = kappa_report(&conn).unwrap();
        assert_eq!(cells.len(), 1);
        let cell = &cells[0];
        assert_eq!(cell.n_joint, 0);
        assert_eq!(cell.ac1_units, NO_KAPPA);
        let note = cell.ac1_note.as_deref().unwrap();
        assert!(
            note.contains("no ratings supplied"),
            "the AC1 fn's own refusal: {note}"
        );
        assert!(
            note.starts_with("ac1: "),
            "the note names its metric: {note}"
        );
        assert_eq!(cell.raw_agreement_units, NO_KAPPA);
        assert_eq!(cell.prevalence_flag, None);
    }

    /// KM-3: the prevalence flag fires on the skewed cell (9:1) and stays
    /// silent on the balanced one — and the skewed cell IS the paradox on
    /// display: raw agreement 90%, κ 0, AC1 8895, flagged. Hand arithmetic
    /// for the skewed cell (9 agree/agree + 1 agree/disagree, slot0 all
    /// agree, slot1 9 agree 1 disagree): observed 9/10; Pe(κ) = 1.0·0.9 =
    /// 0.9 → κ 0; π_agree = 0.95, π_disagree = 0.05, Pe(γ) = 0.095 → AC1 =
    /// 0.805/0.905 → 8895; modal pooled prevalence 0.95 > 0.80 → flagged.
    /// The balanced cell (5 agree/agree + 5 disagree/disagree): raw 10000,
    /// marginals 50/50 → modal 0.5, inside the band → silent.
    #[test]
    fn kappa_report_flags_the_skewed_cell_and_not_the_balanced_one() {
        let mut conn = db();
        // The skewed cell: 10 joint tuples, 9:1 — the paradox shape.
        for i in 0..10 {
            let digest = seed_tuple(&conn, 41, "skewed", &format!("case-s{i}"), "C2");
            submit(&mut conn, 41, &digest, "agree", 0);
            submit(
                &mut conn,
                41,
                &digest,
                if i < 9 { "agree" } else { "disagree" },
                1,
            );
        }
        // The balanced cell: 10 joint tuples, 50/50.
        for i in 0..10 {
            let digest = seed_tuple(&conn, 42, "balanced", &format!("case-b{i}"), "C2");
            let label = if i < 5 { "agree" } else { "disagree" };
            submit(&mut conn, 42, &digest, label, 0);
            submit(&mut conn, 42, &digest, label, 1);
        }
        let cells = kappa_report(&conn).unwrap();
        assert_eq!(cells.len(), 2, "two domains, one cell each");
        let skewed = cells.iter().find(|c| c.domain == "skewed").unwrap();
        let balanced = cells.iter().find(|c| c.domain == "balanced").unwrap();
        // The paradox on display: high raw agreement, collapsed κ, stable
        // AC1 — all three recorded together, flagged.
        assert_eq!(skewed.raw_agreement_units, 9_000);
        assert_eq!(skewed.kappa_units, 0);
        assert_eq!(skewed.ac1_units, 8_895);
        assert_eq!(
            skewed.prevalence_flag.as_deref(),
            Some(PREVALENCE_SKEWED),
            "modal prevalence 0.95 leaves the band"
        );
        // The balanced cell: the flag stays silent.
        assert_eq!(balanced.raw_agreement_units, 10_000);
        assert_eq!(balanced.kappa_units, 10_000);
        assert_eq!(balanced.ac1_units, 10_000);
        assert_eq!(balanced.prevalence_flag, None);
        // KM-2, behavioral arm: EVERY cell emitted the whole suite — both
        // metrics, the raw agreement, and the flag field, populated or
        // honestly refusing.
        for cell in &cells {
            let _ = (cell.kappa_units, cell.ac1_units, cell.raw_agreement_units);
            assert!(
                cell.kappa_note.is_some() || cell.kappa_units != NO_KAPPA,
                "no silent κ"
            );
            assert!(
                cell.ac1_note.is_some() || cell.ac1_units != NO_KAPPA,
                "no silent AC1"
            );
        }
    }

    /// KM-2, structural arm: the cell's TYPE carries the whole suite and
    /// the report handler's JSON map emits every one of its keys — read
    /// off the source, so a key dropped from the wire reds here.
    #[test]
    fn kappa_cell_type_and_wire_carry_the_whole_suite() {
        const CORE: &str = include_str!("kappa.rs");
        const HANDLER: &str = include_str!("../handlers/kappa.rs");
        for field in [
            "kappa_units",
            "ac1_units",
            "raw_agreement_units",
            "prevalence_flag",
        ] {
            assert!(
                CORE.contains(&format!("pub {field}: ")),
                "KappaCell must declare {field}"
            );
            assert!(
                HANDLER.contains(&format!("\"{field}\"")),
                "the report wire must emit {field}"
            );
        }
    }
}
