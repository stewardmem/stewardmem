//! The journey projection (IMPROVE15) — the customer-facing read of the chain.
//!
//! THE PROJECTION LAW (the round's constitution): same chain state →
//! byte-identical journey. No journey table, no cache, no parallel log — a
//! projection that could drift from the audit chain is the "false coverage"
//! defect (the III-6 lesson at customer scale). This module is a PURE fn over
//! rows its caller already fetched: no clock, no pool, no write, no LLM. The
//! determinism pin (`tests/journey_pins.rs`) compares two renders byte for
//! byte and re-sorts the input rows to prove the caller's order never leaks —
//! the projection itself re-sorts every input into source order, so the
//! caller's order is irrelevant BY CONSTRUCTION.
//!
//! The output is the closed customer-event vocabulary the plan's §2 table
//! names — each chain fact maps to ONE customer sentence. Every actor renders
//! as a ROLE (`a supervisor`, `the privacy office`, `the operator`, `the
//! system`) through the total [`role_of_actor`] mapping: a roster name, a
//! masked subject, and the literal `api` — the audit-filter round's three
//! measured actor forms — each map INTO a role and none ever passes through.
//! The planted-name control proves the render-scan catches a passthrough.
//!
//! Inputs, all chain-provable facts (no derived state, no cache):
//! - the run row (`workflow_runs`): the case opened, its current state;
//! - the GDL journal's `control:gdl` checkpoints (`agent_session_events`):
//!   each carries the case snapshot as of that store, so the phase ordinals,
//!   the triage escalation, the CG-05 split and the CG-04 re-examination are
//!   DATED transitions read off the journal — the parse extracts only the
//!   closed fields (phase names, the split count, artifact presence); no
//!   ticket or artifact text ever leaves the parse;
//! - the outbox `case/decision` rows (the closed five-key grammar): the
//!   delivered hand-off decision is the CG-07 hand-down act;
//! - the approved proposals ⋈ the HITL ledger: the digest-bound review acts
//!   (the receipt digest is the same `review_digest` the approve bound);
//! - the domain's legal holds and completed DSARs (the client's own data
//!   acts, dated from their rows).
//!
//! The appeal kinds (`appeal_received` / `appeal_decided`) join this
//! vocabulary in C-d — the const list and the enum move together, and the
//! sentence table stays closed.

use serde::{Deserialize, Serialize};

// ── the closed kind vocabulary ─────────────────────────────────────────────

/// The closed customer-event vocabulary (the plan's §2 table, in order).
/// C-d appends the appeal kinds; widening is a dated act, never an accident.
pub const JOURNEY_KINDS: &[&str] = &[
    "case_opened",
    "phase_advanced",
    "triage_escalated",
    "case_split",
    "reexamined",
    "resolution_approved",
    "handdown_executed",
    "hold_added",
    "hold_released",
    "dsar_completed",
    "appeal_received",
    "appeal_decided",
];

/// One journey event kind. `wire_kind` is the string that crosses the wire
/// and the key the sentence table is closed against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JourneyKind {
    CaseOpened,
    PhaseAdvanced,
    TriageEscalated,
    CaseSplit,
    Reexamined,
    ResolutionApproved,
    HanddownExecuted,
    HoldAdded,
    HoldReleased,
    DsarCompleted,
    AppealReceived,
    AppealDecided,
}

impl JourneyKind {
    pub fn wire_kind(self) -> &'static str {
        match self {
            Self::CaseOpened => "case_opened",
            Self::PhaseAdvanced => "phase_advanced",
            Self::TriageEscalated => "triage_escalated",
            Self::CaseSplit => "case_split",
            Self::Reexamined => "reexamined",
            Self::ResolutionApproved => "resolution_approved",
            Self::HanddownExecuted => "handdown_executed",
            Self::HoldAdded => "hold_added",
            Self::HoldReleased => "hold_released",
            Self::DsarCompleted => "dsar_completed",
            Self::AppealReceived => "appeal_received",
            Self::AppealDecided => "appeal_decided",
        }
    }
}

// ── the roles-not-names mapping ────────────────────────────────────────────

/// The act an actor performed — the ROLE comes from the act's class, never
/// from the identity (the propose/dispose law: the disposal is a human act,
/// and the human's role is what the customer sees).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorAct {
    /// a digest-bound disposal on the review queue (the supervisor's act).
    Approve,
    /// a privacy instrument (a legal hold, a DSAR completion) — the privacy
    /// office's act.
    Privacy,
    /// a machine act (the kernel's own machinery).
    System,
}

/// The three measured actor forms (the audit-filter round): a roster
/// principal's label, a masked subject (`mask_sub`'s 12-hex form), and the
/// literal `api` on infra. The classifier is the mapping's input law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorForm {
    /// a named roster principal (or any other identity label).
    Roster,
    /// a masked subject — `mask_sub` renders 12 hex characters.
    Masked,
    /// the infra literal (or an absent actor).
    System,
}

/// Which of the three measured actor forms a label is. Anything that is not
/// `api`/empty/loopback and not 12 hex characters is a Roster form — the
/// mapping stays total regardless, and nothing is ever classified INTO a name.
pub fn classify_actor(actor: &str) -> ActorForm {
    if actor.is_empty() || actor == "api" || actor == "loopback" {
        return ActorForm::System;
    }
    if actor.len() == 12 && actor.chars().all(|c| c.is_ascii_hexdigit()) {
        return ActorForm::Masked;
    }
    ActorForm::Roster
}

/// THE ROLES-NOT-NAMES MAPPING — total over every actor form and every act:
/// the customer event's actor is always one of the four fixed roles, and the
/// input label never passes through. A roster name on an approve act is `a
/// supervisor` (the act decides); on a privacy act, `the privacy office`; a
/// masked subject is still a human act; `api`/loopback/absent is `the
/// system`. The planted-name pin holds the "never passes through" side.
pub fn role_of_actor(actor: &str, act: ActorAct) -> &'static str {
    match classify_actor(actor) {
        ActorForm::System => "the system",
        ActorForm::Masked | ActorForm::Roster => match act {
            ActorAct::Approve => "a supervisor",
            ActorAct::Privacy => "the privacy office",
            ActorAct::System => "the operator",
        },
    }
}

// ── the closed sentence table (the plan's §2) ─────────────────────────────

/// THE customer sentence for a journey kind. The table is CLOSED: one
/// sentence per kind, the receipts and counts interpolated where the
/// vocabulary names them, and nothing else — no scores, no free text, no
/// infra. The `journey_sentences_are_the_closed_vocabulary_table` pin holds
/// every row to the plan's §2 wording.
pub fn sentence_for(
    kind: JourneyKind,
    phase: Option<&str>,
    count: Option<i64>,
    digest: Option<&str>,
) -> String {
    match kind {
        JourneyKind::CaseOpened => "Your case was opened.".to_string(),
        JourneyKind::PhaseAdvanced => {
            format!("Your case moved to {}.", phase.unwrap_or("the next phase"))
        }
        JourneyKind::TriageEscalated => "A specialist is reviewing your case.".to_string(),
        JourneyKind::CaseSplit => format!(
            "Your case was split into {} linked cases.",
            count.unwrap_or(0)
        ),
        JourneyKind::Reexamined => "An earlier conclusion was re-examined.".to_string(),
        JourneyKind::ResolutionApproved => format!(
            "A supervisor approved the resolution. Receipt: {}.",
            digest.unwrap_or("")
        ),
        JourneyKind::HanddownExecuted => "The verified procedure was applied.".to_string(),
        JourneyKind::HoldAdded => "A legal hold affects your data.".to_string(),
        JourneyKind::HoldReleased => "A legal hold on your data was released.".to_string(),
        JourneyKind::DsarCompleted => match digest {
            Some(d) => format!("Your data request completed. Certificate: {d}."),
            None => "Your data request completed.".to_string(),
        },
        JourneyKind::AppealReceived => "Your appeal was received.".to_string(),
        JourneyKind::AppealDecided => "Your appeal was decided.".to_string(),
    }
}

// ── the appeal's declared review clock (the sla_seconds pattern) ──────────
//
// THE CONST TABLE (the tier-table pattern): one appeal class today — the
// customer appeal through the portal. Widening is a dated act, never a
// tweak. An appeal without a clock is a suggestion; this makes it a
// commitment the chain can prove was kept or missed.

/// The customer appeal's declared review clock, in seconds (seven days).
pub const APPEAL_REVIEW_CLOCK_SECS: i64 = 7 * 24 * 3600;

/// The clock's deadline: a pure fn over the row's own `created_at` — the
/// commitment the chain can prove.
pub fn appeal_clock_deadline(created_at: i64) -> i64 {
    created_at + APPEAL_REVIEW_CLOCK_SECS
}

/// The clock's verdict, pure over chain-provable facts: the proposal row's
/// `created_at`, the HITL ledger's decision `ts` (None = still open), and
/// the caller's now (only used to name an OPEN breach — a pending appeal
/// past its deadline is already a missed commitment the moment the deadline
/// passes, decided or not).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppealClock {
    /// disposed before the deadline — the commitment kept.
    Kept,
    /// disposed after the deadline — the commitment missed (the alert fires
    /// through the EXISTING webhook path; the audit row carries the verdict).
    Missed,
    /// still open past the deadline — missed and un-decided.
    MissedOpen,
    /// inside the window, undecided — the commitment still keepable.
    Pending,
}

impl AppealClock {
    pub fn kept_or_missed(created_at: i64, decided_at: Option<i64>, now: i64) -> Self {
        let deadline = appeal_clock_deadline(created_at);
        match decided_at {
            Some(ts) if ts <= deadline => Self::Kept,
            Some(_) => Self::Missed,
            None if now > deadline => Self::MissedOpen,
            None => Self::Pending,
        }
    }
}

// ── the closed journal extraction ──────────────────────────────────────────
//
// Each `control:gdl` journal row's payload is the WHOLE Checkpoint JSON —
// including the case's ticket verbatim and every artifact. The projection
// parses ONLY the closed fields below (`serde` ignores the rest), so no
// artifact or ticket text can reach a customer event: the extraction type
// has no field that could carry one.

#[derive(Debug, Deserialize)]
struct JournalSnapshot {
    #[serde(default)]
    case: JournalCase,
}

#[derive(Debug, Default, Deserialize)]
struct JournalCase {
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    triage: Option<JournalTriage>,
    #[serde(default)]
    decomposition: Option<JournalDecomposition>,
    /// presence is the fact (CG-04); the pivot text is never read.
    #[serde(default)]
    rehypothesis: Option<JournalMarker>,
}

/// The triage verdict — `defer`/`unknown` is the escalation the vocabulary
/// names ("A specialist is reviewing your case."). The P-class, acuity and
/// ESI fields are internal scores and are deliberately absent here (the
/// curation scan reds on any score in a customer event).
#[derive(Debug, Deserialize)]
struct JournalTriage {
    #[serde(default)]
    verdict: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JournalDecomposition {
    #[serde(default)]
    issues: Vec<serde_json::Value>,
}

/// A presence-only marker: the key exists, the content is unread.
#[derive(Debug, Deserialize)]
struct JournalMarker {}

/// The `case/decision` lineage row's closed five-key grammar
/// (`append_case_decision`): kind, decision_ref, subject, outcome, at. The
/// projection reads only kind + outcome — refs and subjects are the
/// operator's pointers, never customer words.
#[derive(Debug, Deserialize)]
struct CaseDecision {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    outcome: Option<String>,
}

// ── the caller-read inputs (plain rows; the fn stays pure) ────────────────

/// One case run's chain rows, exactly as the caller fetched them.
pub struct JourneyRun<'a> {
    pub run_id: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub status: &'a str,
    pub state_json: &'a str,
}

/// One digest-bound disposal: the approved proposal ⋈ its HITL ledger row.
/// `digest` is the SAME `review_digest` the approve bound (the receipt the
/// §2 vocabulary names); `actor` is the ledger's actor label — the mapping
/// renders it as a role.
pub struct ApprovalFact {
    pub proposal_id: i64,
    pub decided_at: i64,
    pub digest: String,
    pub actor: String,
}

/// One legal hold on the client's domain. `actor` is the hold row's own
/// actor label (`held_by`) — the mapping renders it as a role.
pub struct HoldFact {
    pub id: i64,
    pub held_at: i64,
    pub released_at: Option<i64>,
    pub actor: String,
}

/// One completed data request. `certificate_digest` is the certificate's
/// digest where the ledger carries one — the receipt the §2 vocabulary names.
/// `actor` is the completion's actor label, rendered as a role.
pub struct DsarFact {
    pub id: i64,
    pub completed_at: i64,
    pub certificate_digest: Option<String>,
    pub actor: String,
}

/// One appeal: the proposal row ⋈ (optionally) its HITL disposal row. The
/// received event rides `created_at`; the decided event rides the ledger's
/// `ts` + its decision word (approve|reject — both are disposals the
/// customer sees as "decided", never a reversal).
pub struct AppealFact {
    pub proposal_id: i64,
    pub created_at: i64,
    pub decided_at: Option<i64>,
}

/// The projection's whole input: one case run + the chain rows and the
/// domain-level acts that belong to the same client. Rows in, events out —
/// the fn owns no state.
pub struct JourneyChain<'a> {
    pub run: JourneyRun<'a>,
    /// the run's `control:gdl` journal rows, verbatim — (seq, created_at,
    /// payload_json). Caller-fetched; the projection re-sorts by seq.
    pub checkpoints: &'a [(i64, i64, String)],
    /// the run's outbox `case/decision` rows, verbatim — (id, created_at,
    /// payload_json). Caller-fetched; the projection re-sorts by id.
    pub decisions: &'a [(i64, i64, String)],
    pub approvals: &'a [ApprovalFact],
    pub holds: &'a [HoldFact],
    pub dsars: &'a [DsarFact],
    /// the domain's appeals (kind='appeal' proposals ⋈ their disposals).
    pub appeals: &'a [AppealFact],
}

/// One customer event. Every field is a chain-provable fact or a fixed
/// vocabulary word; the wire shape is additive and closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JourneyEvent {
    /// the closed vocabulary key (`JOURNEY_KINDS`).
    pub kind: &'static str,
    /// the case run the event belongs to; `0` for a domain-level act (a hold
    /// or a DSAR — data-level facts, not case facts).
    pub case_id: i64,
    /// epoch seconds, from the source chain row.
    pub at: i64,
    /// the source row's own monotonic id/seq — the provenance anchor a
    /// reviewer can follow back to the chain row.
    pub seq: i64,
    /// THE customer sentence (the closed §2 table).
    pub text: String,
    /// the ROLE (never a name, never `api`).
    pub actor: &'static str,
    /// the receipt where the vocabulary names one (the digest the approve
    /// bound / the DSAR certificate's digest).
    pub digest: Option<String>,
    /// the linked-case count where the vocabulary names one (CG-05).
    pub count: Option<i64>,
    /// the phase name where the vocabulary names one.
    pub phase: Option<&'static str>,
}

// ── the projection ─────────────────────────────────────────────────────────

/// Project one case run + its client's data acts into the customer journey.
/// PURE: rows in, events out, sorted by a TOTAL key — `(at, kind, case_id,
/// seq, text)` — and every input re-sorted into its own source order first,
/// so the caller's row order can never leak (the determinism pin reverses
/// its inputs to prove it).
pub fn project(chain: &JourneyChain) -> Vec<JourneyEvent> {
    let run = &chain.run;
    let mut events: Vec<JourneyEvent> = Vec::new();

    // The case opened — the run row is the chain fact.
    events.push(JourneyEvent {
        kind: JourneyKind::CaseOpened.wire_kind(),
        case_id: run.run_id,
        at: run.created_at,
        seq: run.run_id,
        text: sentence_for(JourneyKind::CaseOpened, None, None, None),
        actor: "the operator",
        digest: None,
        count: None,
        phase: None,
    });

    // The phase ordinals + the CG-04/CG-05 facts, read off the journal's
    // snapshots: each `control:gdl` row carries the case AS OF that store,
    // so a changed phase/artifact is a dated transition. The rows are walked
    // in SEQ order (re-sorted here), and adjacency-deduplicated — a phase
    // that recurs (the CG-04 re-entry) is a real transition; a snapshot that
    // repeats the last row's phase is not.
    let mut journal: Vec<&(i64, i64, String)> = chain.checkpoints.iter().collect();
    journal.sort_by_key(|(seq, _, _)| *seq);
    let mut last_phase: Option<String> = None;
    let mut seen_split = false;
    let mut seen_rehypo = false;
    let mut seen_escalated = false;
    for (seq, created_at, payload) in journal {
        let Ok(snap) = serde_json::from_str::<JournalSnapshot>(payload) else {
            // an unparsable journal row contributes nothing — the projection
            // never invents an event a chain row does not prove.
            continue;
        };
        if let Some(phase) = &snap.case.phase
            && last_phase.as_deref() != Some(phase.as_str())
            && let Some(wire) = phase_wire(phase)
        {
            last_phase = Some(phase.clone());
            events.push(JourneyEvent {
                kind: JourneyKind::PhaseAdvanced.wire_kind(),
                case_id: run.run_id,
                at: *created_at,
                seq: *seq,
                text: sentence_for(JourneyKind::PhaseAdvanced, Some(wire), None, None),
                actor: "the operator",
                digest: None,
                count: None,
                phase: Some(wire),
            });
        }
        // The triage escalation (defer/unknown escalates with the bundle —
        // the T4/T5 law): one event, on the first snapshot that carries it.
        if !seen_escalated
            && let Some(triage) = &snap.case.triage
            && matches!(triage.verdict.as_deref(), Some("defer") | Some("unknown"))
        {
            seen_escalated = true;
            events.push(JourneyEvent {
                kind: JourneyKind::TriageEscalated.wire_kind(),
                case_id: run.run_id,
                at: *created_at,
                seq: *seq,
                text: sentence_for(JourneyKind::TriageEscalated, None, None, None),
                actor: "a supervisor",
                digest: None,
                count: None,
                phase: None,
            });
        }
        // CG-05: the split-pending marker — one event with the linked count.
        if !seen_split && let Some(decomp) = &snap.case.decomposition {
            seen_split = true;
            let count = decomp.issues.len() as i64;
            events.push(JourneyEvent {
                kind: JourneyKind::CaseSplit.wire_kind(),
                case_id: run.run_id,
                at: *created_at,
                seq: *seq,
                text: sentence_for(JourneyKind::CaseSplit, None, Some(count), None),
                actor: "a supervisor",
                digest: None,
                count: Some(count),
                phase: None,
            });
        }
        // CG-04: the rehypothesis proposal — one event, first appearance.
        if !seen_rehypo && snap.case.rehypothesis.is_some() {
            seen_rehypo = true;
            events.push(JourneyEvent {
                kind: JourneyKind::Reexamined.wire_kind(),
                case_id: run.run_id,
                at: *created_at,
                seq: *seq,
                text: sentence_for(JourneyKind::Reexamined, None, None, None),
                actor: "a supervisor",
                digest: None,
                count: None,
                phase: None,
            });
        }
    }

    // A run with no journal rows still renders its CURRENT phase — from the
    // run row's state (a chain row), at the run row's updated_at. The event
    // stays a projection of stored bytes, never a guess at a timeline.
    if last_phase.is_none()
        && let Ok(state) = serde_json::from_str::<JournalSnapshot>(run.state_json)
    {
        if let Some(phase) = state.case.phase
            && let Some(wire) = phase_wire(&phase)
        {
            events.push(JourneyEvent {
                kind: JourneyKind::PhaseAdvanced.wire_kind(),
                case_id: run.run_id,
                at: run.updated_at,
                seq: run.run_id,
                text: sentence_for(JourneyKind::PhaseAdvanced, Some(wire), None, None),
                actor: "the operator",
                digest: None,
                count: None,
                phase: Some(wire),
            });
        }
        // the same fallback carries the state-level facts the journal
        // would have dated: escalation / split / rehypothesis, at
        // updated_at — still stored bytes, still one event each.
        if let Some(triage) = state.case.triage
            && matches!(triage.verdict.as_deref(), Some("defer") | Some("unknown"))
        {
            events.push(JourneyEvent {
                kind: JourneyKind::TriageEscalated.wire_kind(),
                case_id: run.run_id,
                at: run.updated_at,
                seq: run.run_id,
                text: sentence_for(JourneyKind::TriageEscalated, None, None, None),
                actor: "a supervisor",
                digest: None,
                count: None,
                phase: None,
            });
        }
        if let Some(decomp) = state.case.decomposition {
            let count = decomp.issues.len() as i64;
            events.push(JourneyEvent {
                kind: JourneyKind::CaseSplit.wire_kind(),
                case_id: run.run_id,
                at: run.updated_at,
                seq: run.run_id,
                text: sentence_for(JourneyKind::CaseSplit, None, Some(count), None),
                actor: "a supervisor",
                digest: None,
                count: Some(count),
                phase: None,
            });
        }
        if state.case.rehypothesis.is_some() {
            events.push(JourneyEvent {
                kind: JourneyKind::Reexamined.wire_kind(),
                case_id: run.run_id,
                at: run.updated_at,
                seq: run.run_id,
                text: sentence_for(JourneyKind::Reexamined, None, None, None),
                actor: "a supervisor",
                digest: None,
                count: None,
                phase: None,
            });
        }
    }

    // The CG-07 hand-down: the run's `case/decision` row whose closed grammar
    // names the handoff transition DELIVERED — the verified procedure was
    // applied. Rows walked in id order; one event per delivered decision.
    let mut decisions: Vec<&(i64, i64, String)> = chain.decisions.iter().collect();
    decisions.sort_by_key(|(id, _, _)| *id);
    for (id, created_at, payload) in decisions {
        let Ok(d) = serde_json::from_str::<CaseDecision>(payload) else {
            continue;
        };
        if d.kind.as_deref() == Some("handoff_transition")
            && d.outcome.as_deref() == Some("delivered")
        {
            events.push(JourneyEvent {
                kind: JourneyKind::HanddownExecuted.wire_kind(),
                case_id: run.run_id,
                at: *created_at,
                seq: *id,
                text: sentence_for(JourneyKind::HanddownExecuted, None, None, None),
                actor: "the operator",
                digest: None,
                count: None,
                phase: None,
            });
        }
    }

    // The digest-bound review acts: the approved proposals ⋈ the HITL ledger.
    // The receipt digest rides verbatim; the actor renders as a role.
    let mut approvals: Vec<&ApprovalFact> = chain.approvals.iter().collect();
    approvals.sort_by_key(|a| (a.decided_at, a.proposal_id));
    for approval in approvals {
        events.push(JourneyEvent {
            kind: JourneyKind::ResolutionApproved.wire_kind(),
            case_id: run.run_id,
            at: approval.decided_at,
            seq: approval.proposal_id,
            text: sentence_for(
                JourneyKind::ResolutionApproved,
                None,
                None,
                Some(&approval.digest),
            ),
            actor: role_of_actor(&approval.actor, ActorAct::Approve),
            digest: Some(approval.digest.clone()),
            count: None,
            phase: None,
        });
    }

    // The client's data acts + the appeals ride the DOMAIN half (one fn, so
    // the case timeline and the response's domain_events can never disagree):
    events.extend(project_domain(chain.appeals, chain.holds, chain.dsars));

    // The total sort key: (at, kind, case_id, seq, text). Deterministic over
    // any input row order; ties fall to the rendered text, which is a pure
    // fn of the closed table.
    events.sort_by(|a, b| {
        (a.at, a.kind, a.case_id, a.seq, a.text.as_str()).cmp(&(
            b.at,
            b.kind,
            b.case_id,
            b.seq,
            b.text.as_str(),
        ))
    });
    events
}

/// The DOMAIN half of the projection: the client's own acts (legal holds,
/// completed DSARs, the appeals) — the events that belong to the CLIENT, not
/// to one case. ONE fn so a case timeline's copy and the response's
/// domain_events copy can never disagree (the projection law: one
/// projection). Sorted by the same total key as the case half.
pub fn project_domain(
    appeals: &[AppealFact],
    holds: &[HoldFact],
    dsars: &[DsarFact],
) -> Vec<JourneyEvent> {
    let mut events: Vec<JourneyEvent> = Vec::new();
    let mut holds: Vec<&HoldFact> = holds.iter().collect();
    holds.sort_by_key(|h| (h.held_at, h.id));
    for hold in holds {
        events.push(JourneyEvent {
            kind: JourneyKind::HoldAdded.wire_kind(),
            case_id: 0,
            at: hold.held_at,
            seq: hold.id,
            text: sentence_for(JourneyKind::HoldAdded, None, None, None),
            actor: role_of_actor(&hold.actor, ActorAct::Privacy),
            digest: None,
            count: None,
            phase: None,
        });
        if let Some(released_at) = hold.released_at {
            events.push(JourneyEvent {
                kind: JourneyKind::HoldReleased.wire_kind(),
                case_id: 0,
                at: released_at,
                seq: hold.id,
                text: sentence_for(JourneyKind::HoldReleased, None, None, None),
                actor: role_of_actor(&hold.actor, ActorAct::Privacy),
                digest: None,
                count: None,
                phase: None,
            });
        }
    }
    let mut dsars: Vec<&DsarFact> = dsars.iter().collect();
    dsars.sort_by_key(|d| (d.completed_at, d.id));
    for dsar in dsars {
        events.push(JourneyEvent {
            kind: JourneyKind::DsarCompleted.wire_kind(),
            case_id: 0,
            at: dsar.completed_at,
            seq: dsar.id,
            text: sentence_for(
                JourneyKind::DsarCompleted,
                None,
                None,
                dsar.certificate_digest.as_deref(),
            ),
            actor: role_of_actor(&dsar.actor, ActorAct::Privacy),
            digest: dsar.certificate_digest.clone(),
            count: None,
            phase: None,
        });
    }
    // The appeals (the round's centerpiece): received at the filing, decided
    // at the HITL disposal — "received." / "decided.", roles not names, no
    // reversal rendered (the decision is the fact; what it decided is the
    // operator's side of the provenance pair).
    let mut appeals: Vec<&AppealFact> = appeals.iter().collect();
    appeals.sort_by_key(|a| (a.created_at, a.proposal_id));
    for appeal in appeals {
        events.push(JourneyEvent {
            kind: JourneyKind::AppealReceived.wire_kind(),
            case_id: 0,
            at: appeal.created_at,
            seq: appeal.proposal_id,
            text: sentence_for(JourneyKind::AppealReceived, None, None, None),
            actor: "the system",
            digest: None,
            count: None,
            phase: None,
        });
        if let Some(decided_at) = appeal.decided_at {
            events.push(JourneyEvent {
                kind: JourneyKind::AppealDecided.wire_kind(),
                case_id: 0,
                at: decided_at,
                seq: appeal.proposal_id,
                text: sentence_for(JourneyKind::AppealDecided, None, None, None),
                actor: "the privacy office",
                digest: None,
                count: None,
                phase: None,
            });
        }
    }
    events.sort_by(|a, b| {
        (a.at, a.kind, a.case_id, a.seq, a.text.as_str()).cmp(&(
            b.at,
            b.kind,
            b.case_id,
            b.seq,
            b.text.as_str(),
        ))
    });
    events
}

/// The phase name the vocabulary renders. The journal stores the engine's
/// phase names; the seven GDL phases cross verbatim (they are the customer's
/// words in the plan's own examples — `intake`/`triage`/…), anything else is
/// refused: the projection never renders a phase word it does not know.
fn phase_wire(phase: &str) -> Option<&'static str> {
    match phase {
        "intake" => Some("intake"),
        "triage" => Some("triage"),
        "hypothesize" => Some("hypothesize"),
        "plan" => Some("plan"),
        "act" => Some("act"),
        "verify" => Some("verify"),
        "handoff" => Some("handoff"),
        _ => None,
    }
}
