//! The cascade coordinator (T1) — round 17 S1.
//!
//! The tiered cascade's route tier: one guided AFM turn per decision, the
//! decision landing as a TYPED VALUE (TC-2) the kernel validates — no model
//! in the gate. The tier boundaries and the input contract are S0's
//! measured design of record (`IMPROVE17_EVIDENCE_2026-11-13.md` §3.3,
//! plans repo): the summary is CONTENT-BEARING (hit count + the abstain
//! signal + top-3 score-and-snippet lines) because the S0 base run measured
//! the title-only shape failing into echo-misroutes, and the abstain signal
//! rides every summary because S0 measured the junk-hit abstain miss.
//!
//! Laws (by construction, each pinned):
//! - TC-2: the route decision is a typed four-case value; the AFM tier's
//!   guided generation constrains the SHAPE (a guided object through the
//!   round-14 wire-schema turn — no new socket); the VALUE is validated
//!   HERE, kernel-side. Out-of-roster → `None` → the preregistered
//!   fallback (`synthesize` — the safe default that goes and reads, the
//!   writer's kernel gate judges the evidence).
//! - TC-6 (the AFM half): one session = one tier. The coordinator IS the
//!   AFM tier — a fresh session per classification and never a `model`
//!   field on the wire (absent = `afm3-core` by the bridge's own law; a
//!   sent field would be the mid-session-swap shape the bridge refuses as
//!   `model_locked`). A mid-session swap therefore cannot even be
//!   expressed by this module.
//! - TC-7: every classification writes its audit row — the model asked and
//!   served, the raw guided value, the roster verdict, confidence, tokens,
//!   duration — through the EXISTING `audit::record` door under the NEW
//!   `AuditKind::Cascade` kind (the `DocCorrection` precedent: one variant
//!   with its `as_str` arm; the chain stores hashes, so no query text
//!   rides a row).
//! - TC-1: the coordinator writes NO memory and NO proposal. The decision
//!   is a returned value; what the CALLER does with it is the caller's
//!   governed lane.

use crate::agentloop::bridge_afm::{BridgeEvent, BridgeTurn};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// The route decision as a typed value (TC-2): exhaustive, compile-checked
/// at every switch site, each case carrying its wire name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteDecision {
    AnswerFromMemory,
    ReadMore,
    Synthesize,
    EscalateFrontier,
}

impl RouteDecision {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RouteDecision::AnswerFromMemory => "answer_from_memory",
            RouteDecision::ReadMore => "read_more",
            RouteDecision::Synthesize => "synthesize",
            RouteDecision::EscalateFrontier => "escalate_frontier",
        }
    }

    /// The kernel-side roster validation (TC-2): the guided generation
    /// constrains the shape, this constrains the value. Case-sensitive by
    /// design — the wire name is a literal, and a case-drift is exactly the
    /// out-of-roster class the validation exists to refuse.
    pub(crate) fn from_wire(raw: &str) -> Option<Self> {
        match raw {
            "answer_from_memory" => Some(Self::AnswerFromMemory),
            "read_more" => Some(Self::ReadMore),
            "synthesize" => Some(Self::Synthesize),
            "escalate_frontier" => Some(Self::EscalateFrontier),
            _ => None,
        }
    }
}

/// The preregistered fallback for an invalid route: `synthesize` — the safe
/// default that goes and reads (more evidence, and the writer's kernel gate
/// judges it), never a silent guess from the route tier.
pub(crate) fn fallback() -> RouteDecision {
    RouteDecision::Synthesize
}

/// The S0 oracle route as a pure unit (arm D's law, replayed by the pins):
/// an empty relevant set IS the ground-truth judgment that nothing in the
/// memory answers — the optimal zero-token route is the honest abstention,
/// whether or not best-effort recall surfaced junk hits; a top hit whose
/// content spans every relevant doc IS the answer (evidence-as-answer);
/// otherwise the writer composes. `read_more` is never chosen — the oracle
/// bounds the functional split {answer-from-memory, synthesize}.
pub(crate) fn oracle_route(relevant_is_empty: bool, top1_verifies_all_gold: bool) -> RouteDecision {
    if relevant_is_empty || top1_verifies_all_gold {
        RouteDecision::AnswerFromMemory
    } else {
        RouteDecision::Synthesize
    }
}

/// The guided route object's wire schema (round 14 Arm A): a guided OBJECT
/// with a route string and a confidence number. The description names the
/// roster AND forbids the echo classes S0 recorded (memory titles, hit
/// text) — the description is guidance; the VALUE is kernel-validated.
pub(crate) const ROUTE_SCHEMA: &str = r#"{"type":"object","properties":{"route":{"type":"string","description":"The route value: exactly one of the four literal strings answer_from_memory, read_more, synthesize, escalate_frontier — never a memory title, a hit text, or any other word"},"confidence":{"type":"number","description":"0.0 to 1.0"}},"required":["route","confidence"]}"#;

/// The router contract — the AFM tier's instructions for a classification
/// turn. Short, roster-explicit, and echo-hostile by the same recorded
/// classes.
pub(crate) const ROUTER_CONTRACT: &str = "You are a routing classifier for a memory \
assistant. You see a query and a summary of what deterministic recall found. \
Classify the query into EXACTLY one route value. Allowed route values are \
ONLY: answer_from_memory, read_more, synthesize, escalate_frontier. \
answer_from_memory: the recall summary already contains the answer (answer \
directly from it, no big model). read_more: the summary suggests the memory \
has more that should be retrieved before answering. synthesize: the answer \
must be composed from the retrieved evidence by a writer model. \
escalate_frontier: the query needs more intelligence than a local writer. \
No other value is legal.";

/// One top hit as the summary sees it: its score and its content (already
/// sanitized by the recall path — the coordinator never re-reads raw rows).
pub(crate) type SummaryHit = (f64, String);

/// The preregistered summary shape (S0's measured contract): the query, the
/// hit count, the abstain signal (the recall decision — `ok` vs
/// `low_confidence`), the roster line, and the top three hits each with its
/// score and a ≤10-word content snippet. A SUMMARY, never the evidence: the
/// evidence the writer sees is the caller's fenced lane, not this.
pub(crate) fn build_route_summary(
    query: &str,
    recall_decision: &str,
    hits: &[SummaryHit],
) -> String {
    let mut s = format!(
        "Query: {query}\nRecall summary: {} hits, decision: {recall_decision}. Top hits:",
        hits.len()
    );
    for (i, (score, content)) in hits.iter().take(3).enumerate() {
        let snippet: String = content
            .split_whitespace()
            .take(10)
            .collect::<Vec<_>>()
            .join(" ");
        s.push_str(&format!("\n{}. (score {score:.2}) {snippet}", i));
    }
    s.push_str(
        "\n\nChoose route from: answer_from_memory, read_more, synthesize, \
         escalate_frontier.",
    );
    s
}

/// One classification's outcome: the typed decision, the raw guided string,
/// the roster verdict, the confidence, the bridge's own token counts, and
/// the wall duration.
#[derive(Debug, Clone)]
pub(crate) struct RouteCall {
    pub(crate) decision: RouteDecision,
    pub(crate) raw: String,
    pub(crate) roster_ok: bool,
    pub(crate) confidence: f64,
    pub(crate) input_tokens: i64,
    pub(crate) output_tokens: i64,
    pub(crate) duration_ms: u128,
}

/// One classification against the live bridge child. A FRESH session per
/// call (never a reused id — the session-lock law's AFM half) and never a
/// `model` field (the coordinator IS the AFM tier). The guided turn rides
/// the round-14 wire-schema field; the parse and the roster validation are
/// kernel-side (no model in the gate).
pub(crate) async fn classify_route(
    bridge: &Arc<crate::agentloop::bridge_afm::AfmBridge>,
    query: &str,
    recall_decision: &str,
    hits: &[SummaryHit],
) -> Result<RouteCall, crate::agentloop::bridge_afm::BridgeError> {
    let summary = build_route_summary(query, recall_decision, hits);
    let session = format!("cascade-route-{}", uuid::Uuid::new_v4());
    let started = Instant::now();
    let (_id, mut rx) = bridge
        .turn(BridgeTurn {
            session,
            prompt: summary,
            instructions: Some(ROUTER_CONTRACT.to_owned()),
            max_response_tokens: Some(128),
            history: None,
            // ONE SESSION = ONE TIER (TC-6, the AFM half): the field stays
            // absent — the bridge resolves absent as afm3-core and a sent
            // field would be the mid-session-swap shape.
            model: None,
            schema: Some(ROUTE_SCHEMA.to_owned()),
        })
        .await?;
    let mut final_event: Option<BridgeEvent> = None;
    while let Some(event) = rx.recv().await {
        let terminal = !matches!(event, BridgeEvent::Delta { .. });
        if terminal {
            final_event = Some(event);
            break;
        }
    }
    let duration_ms = started.elapsed().as_millis();
    match final_event {
        Some(BridgeEvent::Final {
            text,
            input_tokens,
            output_tokens,
            ..
        }) => {
            let (raw, confidence, decision, roster_ok) = parse_route_call(&text);
            Ok(RouteCall {
                decision,
                raw,
                roster_ok,
                confidence,
                input_tokens,
                output_tokens,
                duration_ms,
            })
        }
        Some(BridgeEvent::Failed { code, message, .. }) => Err(
            crate::agentloop::bridge_afm::BridgeError::Protocol(format!(
                "route turn failed: {code}{}",
                message.map(|m| format!(" — {m}")).unwrap_or_default()
            )),
        ),
        Some(BridgeEvent::Delta { .. }) => unreachable!("terminal guarded above"),
        None => Err(crate::agentloop::bridge_afm::BridgeError::Protocol(
            "route turn ended without a terminal event".into(),
        )),
    }
}

/// The guided object's kernel-side parse: the JSON object's `route` string
/// validated against the roster, `confidence` carried as reported (absent
/// or malformed → 0.0 — the honesty default). The raw string rides the
/// result and the audit row either way — an unparseable or out-of-roster
/// guided value is RECORDED, never swallowed (the caller applies
/// [`fallback`]).
pub(crate) fn parse_route_call(text: &str) -> (String, f64, RouteDecision, bool) {
    let parsed: Option<Value> = serde_json::from_str(text).ok();
    let raw = parsed
        .as_ref()
        .and_then(|v| v["route"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| text.to_string());
    let confidence = parsed
        .as_ref()
        .and_then(|v| v["confidence"].as_f64())
        .unwrap_or(0.0);
    match RouteDecision::from_wire(&raw) {
        Some(decision) => (raw, confidence, decision, true),
        None => (raw, confidence, fallback(), false),
    }
}

/// The audit row for one classification (TC-7's T1 half — the
/// `DocCorrection` per-call shape): the model asked and served, the raw
/// guided value, the roster verdict, confidence, tokens, duration, and the
/// query's sha256 as the target. The chain stores hashes — no query text
/// rides a row. Returns the row id (best-effort, the `audit::record`
/// contract).
pub(crate) fn audit_route_call(
    conn: &rusqlite::Connection,
    call: &RouteCall,
    query_sha256: &str,
) -> Option<i64> {
    let detail = serde_json::json!({
        "tier": "T1",
        "model": "afm3-core",
        "decision": call.decision.as_str(),
        "raw": call.raw,
        "roster_ok": call.roster_ok,
        "confidence": call.confidence,
        "input_tokens": call.input_tokens,
        "output_tokens": call.output_tokens,
        "duration_ms": call.duration_ms,
        "guided": true,
    })
    .to_string();
    let status = if call.roster_ok {
        crate::audit::AuditStatus::Ok
    } else {
        crate::audit::AuditStatus::Error
    };
    crate::audit::record(
        conn,
        crate::audit::AuditKind::Cascade,
        "cascade-coordinator",
        query_sha256,
        status,
        &detail,
    )
}

#[cfg(test)]
pub(crate) mod tests {
    //! The TC-2 + S0-summary behavioral pins, born red against the pre-S1
    //! tree by ABSENCE (the recorded transcript: E0432 unresolved module —
    //! the honest compile-form red for a module that must exist before its
    //! behavior can be asserted; the TC-8 file-absence precedent). The
    //! module is `pub(crate)` by the §0.1.3 in-crate law (the coordinator
    //! lives where the bridge trait is visible), so these pins live
    //! in-module rather than in a test binary.

    use super::*;

    #[test]
    fn tc2_from_wire_accepts_exactly_the_four_case_roster() {
        assert_eq!(
            RouteDecision::from_wire("answer_from_memory"),
            Some(RouteDecision::AnswerFromMemory)
        );
        assert_eq!(
            RouteDecision::from_wire("read_more"),
            Some(RouteDecision::ReadMore)
        );
        assert_eq!(
            RouteDecision::from_wire("synthesize"),
            Some(RouteDecision::Synthesize)
        );
        assert_eq!(
            RouteDecision::from_wire("escalate_frontier"),
            Some(RouteDecision::EscalateFrontier)
        );
    }

    #[test]
    fn tc2_from_wire_refuses_out_of_roster_values() {
        // The S0 smoke run's live classes, replayed by name: the model
        // echoed a memory title and invented a route value — the
        // kernel-side roster validation refuses both (no model in the
        // gate).
        for raw in [
            "abstain",
            "eval-gold-10",
            "vxrail lcm upgrades",
            "",
            "ANSWER_FROM_MEMORY",
        ] {
            assert!(
                RouteDecision::from_wire(raw).is_none(),
                "out-of-roster route value {raw:?} must be refused"
            );
        }
    }

    #[test]
    fn tc2_the_roster_is_exhaustive_over_the_enum() {
        // The typed value's four cases each carry their wire name — an
        // exhaustive match here fails to compile when a case is added
        // without its wire name (the compile-checked shape the plan names).
        let all = [
            RouteDecision::AnswerFromMemory,
            RouteDecision::ReadMore,
            RouteDecision::Synthesize,
            RouteDecision::EscalateFrontier,
        ];
        for d in all {
            assert!(!d.as_str().is_empty(), "every case carries its wire name");
            assert_eq!(RouteDecision::from_wire(d.as_str()), Some(d), "round-trip");
        }
    }

    #[test]
    fn tc2_invalid_route_falls_back_to_synthesize() {
        // The preregistered fallback: the safe default that goes and reads —
        // the writer's kernel gate judges the evidence, never the route.
        assert_eq!(fallback(), RouteDecision::Synthesize);
    }

    #[test]
    fn tc2_oracle_replays_the_s0_semantics() {
        // The S0 oracle law (the harness's fixed form, replayed as a unit):
        // an empty relevant set → answer_from_memory (the ground-truth
        // abstention — junk hits never change it, the S0 base-phase
        // lesson); top-1 span-verifying every relevant doc →
        // answer_from_memory; otherwise synthesize. read_more is never
        // chosen.
        assert_eq!(oracle_route(true, false), RouteDecision::AnswerFromMemory);
        assert_eq!(oracle_route(false, true), RouteDecision::AnswerFromMemory);
        assert_eq!(oracle_route(false, false), RouteDecision::Synthesize);
    }

    #[test]
    fn s0_summary_is_content_bearing() {
        // The preregistered summary shape (S0 §3.3): the query, the hit
        // count, the abstain signal (the recall decision), and the top hits
        // each with score + a content snippet — never title-only.
        let hits: Vec<SummaryHit> = vec![
            (
                0.82,
                "vSAN storage policies set FTT failures to tolerate and FTM per VM.".into(),
            ),
            (
                0.71,
                "PowerFlex protection domains map fault sets to SDS pools.".into(),
            ),
        ];
        let s = build_route_summary("vSAN policy failures to tolerate", "ok", &hits);
        assert!(s.contains("Query: vSAN policy failures to tolerate"));
        assert!(s.contains("2 hits"), "the hit count rides the summary");
        assert!(
            s.contains("decision: ok"),
            "the abstain signal rides the summary"
        );
        assert!(
            s.contains("vSAN storage policies set FTT"),
            "the top hit's CONTENT snippet rides the summary (the S0 lesson: \
             title-only summaries made the classifier echo memory ids)"
        );
        assert!(
            s.contains("answer_from_memory") && s.contains("escalate_frontier"),
            "the roster rides the summary text"
        );
    }

    #[test]
    fn s0_summary_bounds_snippets() {
        // The snippet is bounded (≤10 words) — the summary stays a summary.
        let long = "word one two three four five six seven eight nine ten eleven twelve";
        let s = build_route_summary("q", "ok", &[(0.9, long.to_string())]);
        assert!(!s.contains("eleven"), "the snippet must stop at ten words");
    }

    #[test]
    fn s0_route_schema_carries_the_guided_object_shape() {
        // The round-14 wire: a guided OBJECT with route + confidence, both
        // required.
        let v: Value = serde_json::from_str(ROUTE_SCHEMA).expect("valid JSON");
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["route"]["type"], "string");
        assert_eq!(v["properties"]["confidence"]["type"], "number");
        let req = v["required"].as_array().expect("required array");
        let names: Vec<&str> = req.iter().filter_map(|x| x.as_str()).collect();
        assert!(names.contains(&"route") && names.contains(&"confidence"));
    }

    #[test]
    fn tc2_parse_route_call_validates_kernel_side() {
        // The guided object parses; the raw string and the roster verdict
        // ride the result either way (an unparseable value is RECORDED,
        // never swallowed).
        let (raw, conf, d, ok) = parse_route_call(r#"{"route":"synthesize","confidence":0.9}"#);
        assert_eq!(raw, "synthesize");
        assert!((conf - 0.9).abs() < 1e-9);
        assert_eq!(d, RouteDecision::Synthesize);
        assert!(ok);
        // The echo classes: recorded raw, fallback decision, roster=false.
        let (raw, conf, d, ok) = parse_route_call(r#"{"route":"eval-gold-10","confidence":0.5}"#);
        assert_eq!(raw, "eval-gold-10");
        assert!((conf - 0.5).abs() < 1e-9);
        assert_eq!(d, RouteDecision::Synthesize, "the preregistered fallback");
        assert!(!ok);
    }
}
