//! Versioned GDL journal contract. No lease recovery: only an owner that has
//! finished an exchange may explicitly pause. A crash retains its claim.
//! rusqlite rollback-on-drop verified via Context7 2026-09-17 (upstream docs).

use super::{GdlCase, GdlOutcome, GdlPhase, LoopError, MAX_PHASE_ATTEMPTS, session_log};
use crate::workflow::{state, tx::WorkflowTx};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

pub(super) fn persist_error(e: impl std::fmt::Display) -> LoopError {
    LoopError::Persist(e.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Checkpoint {
    version: u32,
    run: i64,
    domain: String,
    policy: String,
    pub episode: String,
    pub revision: i64,
    pub case: GdlCase,
    pub next_phase: Option<GdlPhase>,
    /// Completed failed attempts in next_phase, not the in-flight attempt.
    pub attempt: u32,
    pub errors: Vec<String>,
    pub phases: u32,
    pub exchange: Option<i64>,
    pub terminal: Option<GdlOutcome>,
    status: String,
}

impl Checkpoint {
    /// The run's domain, for the post-gate additive law checks that need
    /// it (the T7 must-miss catalog) — the domain is checkpoint state
    /// (bound to the run row), deliberately not case state.
    pub(super) fn case_domain(&self) -> &str {
        &self.domain
    }

    fn key(&self) -> String {
        format!("control:gdl:{}", self.revision)
    }

    fn json(&self) -> Result<String, LoopError> {
        serde_json::to_string(self).map_err(persist_error)
    }

    fn store(&self, conn: &Connection) -> Result<(), LoopError> {
        session_log::append(
            conn,
            self.run,
            "control:gdl",
            &self.json()?,
            &self.key(),
            chrono::Utc::now().timestamp(),
        )
        .map_err(persist_error)?;
        Ok(())
    }

    fn validate(&self) -> Result<(), LoopError> {
        // CG-04 ordinal machine: `phases` is total accepted passes. Mid-flight
        // the pass count is the next phase's position plus 4 passes per
        // COMPLETED re-entry loop (Hypothesize→Plan→Act→Verify); the count
        // bounds the loops. At count == 0 this is exactly today's
        // `phases == next_phase as u32`.
        let position = self.next_phase.map_or(7, |p| p as u32);
        // A handed-down case skips the two L3-owned phases, but only
        // once it has left the second phase: before that jump its
        // pass count is the legacy relation, and from the execute
        // phase onward it runs two behind the position. At zero
        // skipped phases this is exactly the legacy relation.
        let skipped: i64 = if self.case.hand_down.is_some() && position >= 4 {
            2
        } else {
            0
        };
        let loops = (self.phases as i64) + skipped - (position as i64);
        let consistent =
            loops >= 0 && loops % 4 == 0 && loops / 4 <= self.case.rehypothesis_count as i64;
        let expected = super::expected_phases_for_case(&self.case);
        if self.version != 1
            || self.revision < 1
            || self.episode.is_empty()
            || self.episode.len() > 128
            || self.attempt > MAX_PHASE_ATTEMPTS
            || (self.terminal.is_none() && self.attempt == MAX_PHASE_ATTEMPTS)
            || self.phases > expected
            || self.errors.len() > 64
            || self.errors.iter().any(|s| s.len() > 16_384)
            || (self.attempt == 0) != self.errors.is_empty()
            || (self.terminal.is_none() && (self.next_phase.is_none() || !consistent))
            || self.exchange.is_some_and(|id| id <= 0)
            || self.status
                != if matches!(self.terminal, Some(GdlOutcome::Resolved { .. })) {
                    "resolved"
                } else {
                    "active"
                }
        {
            return Err(persist_error("malformed or unsupported GDL checkpoint"));
        }
        let terminal_matches = match &self.terminal {
            Some(GdlOutcome::Resolved {
                phases,
                verify,
                capture,
            }) => {
                let expected = super::expected_phases_for_case(&self.case);
                *phases == expected
                    && self.phases == expected
                    && self.next_phase.is_none()
                    && self.case.phase == GdlPhase::Handoff
                    && verify.pass
                    && self.case.verify.as_ref() == Some(verify)
                    && self.case.capture.as_ref() == Some(capture)
            }
            Some(GdlOutcome::VerifyFailed { at, bundle }) => {
                *at == GdlPhase::Verify
                    && self.case.phase == *at
                    && self.case.verify.as_ref().is_some_and(|v| !v.pass)
                    && *bundle == self.case.escalation_bundle()
            }
            Some(GdlOutcome::Escalated { at, bundle }) => {
                self.next_phase == Some(*at)
                    && self.case.phase == *at
                    && *bundle == self.case.escalation_bundle()
            }
            Some(GdlOutcome::Routed { at, reason }) => {
                self.next_phase == Some(*at)
                    && self.attempt == MAX_PHASE_ATTEMPTS
                    && *reason
                        == format!(
                            "gate exhausted after {} attempts: {}",
                            self.attempt,
                            self.errors.join("; ")
                        )
            }
            Some(GdlOutcome::Capped { at, reason }) => {
                self.next_phase == Some(*at) && matches!(reason.as_str(), "turn_cap" | "budget")
            }
            Some(GdlOutcome::Canceled) => self.next_phase.is_some(),
            Some(GdlOutcome::ProviderFailed { at, .. }) => {
                self.next_phase == Some(*at) && self.case.phase == *at
            }
            None => true,
        };
        if !terminal_matches {
            return Err(persist_error("GDL terminal/case mismatch"));
        }
        Ok(())
    }

    pub fn verify(&self, conn: &Connection, owner: &str) -> Result<(), LoopError> {
        session_log::verify_owner(conn, self.run, owner).map_err(persist_error)?;
        let row = read_run(conn, self.run)?;
        self.bind(&row)?;
        let stored = session_log::exact(conn, self.run, &self.key())
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL checkpoint absent"))?;
        if stored.kind != "control:gdl" || stored.payload_json != self.json()? {
            return Err(persist_error("GDL checkpoint changed"));
        }
        self.verify_link(conn)
    }

    fn verify_link(&self, conn: &Connection) -> Result<(), LoopError> {
        if self.revision == 1 {
            // CG-05: a seeded split child's initial checkpoint carries
            // the parent link on an otherwise-fresh case — an initial
            // claim, not a mutation (admit built it from the seeded
            // row; the link fields are the split's own).
            let mut fresh = GdlCase::fresh(&self.case.ticket);
            fresh.parent_case_id = self.case.parent_case_id;
            fresh.sibling_ids = self.case.sibling_ids.clone();
            if self.exchange.is_some()
                || self.terminal.is_some()
                || self.phases != 0
                || self.attempt != 0
                || self.case != fresh
                || self.next_phase != Some(GdlPhase::Intake)
            {
                return Err(persist_error("invalid initial GDL checkpoint"));
            }
            return Ok(());
        }
        if self.revision > 23 {
            return Err(persist_error(
                "GDL checkpoint revision exceeds episode bound",
            ));
        }
        let row = session_log::exact(conn, self.run, &format!("{}:gate", self.key()))
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL gate missing"))?;
        let gate: GateRecord = serde_json::from_str(&row.payload_json)
            .map_err(|_| persist_error("corrupt GDL gate"))?;
        if row.kind != "gdl_gate"
            || gate.version != 1
            || gate.episode != self.episode
            || gate.revision != self.revision - 1
            || gate.exchange != self.exchange
            || gate.terminal != self.terminal
        {
            return Err(persist_error("GDL gate/checkpoint mismatch"));
        }
        let prior_row =
            session_log::exact(conn, self.run, &format!("control:gdl:{}", gate.revision))
                .map_err(persist_error)?
                .ok_or_else(|| persist_error("GDL prior checkpoint missing"))?;
        let prior: Checkpoint = serde_json::from_str(&prior_row.payload_json)
            .map_err(|_| persist_error("corrupt prior GDL checkpoint"))?;
        prior.validate()?;
        if prior_row.kind != "control:gdl"
            || prior.revision != gate.revision
            || prior.run != self.run
            || prior.domain != self.domain
            || prior.policy != self.policy
            || prior.episode != self.episode
            || prior.terminal.is_some()
            || prior.next_phase != Some(gate.phase)
            || gate.attempt != prior.attempt + 1
        {
            return Err(persist_error("GDL transition predecessor mismatch"));
        }
        prior.verify_link(conn)?;
        let mut expected = prior.case.clone();
        expected.phase = gate.phase;
        if let Some(artifact) = &gate.artifact {
            super::apply(&mut expected, gate.phase, artifact);
        }
        // CG-04: the gate row is the transition's definition. The count
        // moves 0..=1 per transition (the admitted edge is the only
        // increment — a jump of 2+ is corrupt even if the stored case
        // matches); `expected` takes the gate's count so the full case
        // comparison binds the stored state to the transition.
        if gate.rehypothesis_count != prior.case.rehypothesis_count
            && gate.rehypothesis_count != prior.case.rehypothesis_count + 1
        {
            return Err(persist_error("GDL rehypothesis count jump is not one step"));
        }
        expected.rehypothesis_count = gate.rehypothesis_count;
        // CG-05 (the count's precedent): the gate row binds the
        // split children — the one field the split act (not `apply`)
        // sets, so the derivation takes it from the gate, never
        // invents it.
        expected.children = gate.split_children.clone();
        expected.hand_down = gate.hand_down.clone();
        let accepted = gate.verdict == "pass";
        // The narrow-loop injection is the one other non-`apply`
        // write: a passed second phase on a handed-down case carries
        // the sealed plan. The derivation re-derives it from the
        // source row through the mark; the digest must bind.
        if accepted
            && gate.phase == GdlPhase::Triage
            && let Some(mark) = &gate.hand_down
        {
            let snapshot = crate::workflow::hand_down::load_snapshot(conn, mark)
                .map_err(|e| persist_error(format!("hand-down snapshot: {e}")))?;
            expected.plan = snapshot.plan.clone();
            expected.verify_step = Some(snapshot.verify_step.clone());
            expected.dead_end = Some(snapshot.dead_end.clone());
        }
        // CG-04: `next_phase` is derived PHASE-BASED from the gate's
        // explicit target (the admitted `Verify → Hypothesize` edge, the
        // Handoff exit), not array-indexed from the pass count. A legacy
        // gate row (pre-field, serde-defaulted None) with an artifact
        // falls back to the legacy derivation — identical at count 0.
        let next_phase = if accepted {
            match gate.next_phase {
                Some(target) => Some(target),
                None if gate.artifact.is_none() => prior.next_phase,
                None => super::forward_phase(gate.phase, expected.hand_down.is_some()),
            }
        } else {
            prior.next_phase
        };
        let (attempt, errors) = if accepted {
            (0, Vec::new())
        } else if gate.verdict == "fail" {
            (gate.attempt, gate.errors.clone())
        } else {
            (prior.attempt, prior.errors.clone())
        };
        if expected != self.case
            || accepted != gate.artifact.is_some()
            || self.phases != prior.phases + u32::from(accepted)
            || self.next_phase != next_phase
            || self.attempt != attempt
            || self.errors != errors
        {
            return Err(persist_error("GDL gate artifact/state mismatch"));
        }
        gate.verify_receipt(conn, self.run, row.seq, prior.case.rehypothesis_count)?;
        Ok(())
    }

    fn bind(&self, row: &Run) -> Result<(), LoopError> {
        self.validate()?;
        if row.kind != "troubleshoot"
            || row.domain != self.domain
            || row.status != self.status
            || row.revision != self.revision
            || row.json != serde_json::to_string(&self.case).map_err(persist_error)?
        {
            return Err(persist_error(
                "GDL state/status/revision binding changed; explicit new episode refused",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GateRecord {
    version: u32,
    phase: GdlPhase,
    verdict: String,
    attempt: u32,
    errors: Vec<String>,
    episode: String,
    exchange: Option<i64>,
    revision: i64,
    artifact: Option<String>,
    terminal: Option<GdlOutcome>,
    /// CG-04: the explicit forward target of this transition (phase-based).
    /// Additive — `default` keeps old rows readable; a legacy row's None
    /// derives the legacy value (identical at count == 0).
    #[serde(default)]
    next_phase: Option<GdlPhase>,
    /// CG-04: the rehypothesis count AFTER this transition. Additive —
    /// `default` keeps old rows readable (0, the only count the old code
    /// could produce).
    #[serde(default)]
    rehypothesis_count: u32,
    /// CG-05: the split child ids AFTER this transition (the case's
    /// `children` — empty before the split, N ids after). Additive —
    /// `default` keeps old rows readable ([] the only value the old
    /// code could produce). The state-derivation law binds it: the
    /// committed case re-derives `children` from the gate row, exactly
    /// as CG-04 binds the count (the split act's ids are the gate row's
    /// business, not the derivation's invention).
    #[serde(default)]
    split_children: Vec<i64>,
    /// F12b: the exchange's provider-reported usage. Additive — `default`
    /// keeps old rows (written before this field) readable.
    #[serde(default)]
    usage: Option<(u64, u64)>,
    /// The hand-down mark AFTER this transition. Additive — `default`
    /// keeps old rows readable (`None` the only value the old code
    /// could produce). The state-derivation law binds it: the
    /// committed case re-derives `hand_down` from the gate row,
    /// exactly as CG-04 binds the count and CG-05 binds the split
    /// children.
    #[serde(default)]
    hand_down: Option<crate::workflow::hand_down::HandDownMark>,
}

// Local decoder for the existing identity journal contract; no SDK change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExchangeEnd {
    version: u32,
    outcome: super::RunOutcome,
    assistant_key: Option<String>,
}

impl GateRecord {
    /// CG-04 C-14: `pre_count` is the PRIOR checkpoint's
    /// `rehypothesis_count` — the exchange key's loop disambiguator
    /// (pre-transition, matching the driver's key formation at the
    /// phase's start). Old rows have count 0 throughout → base keys.
    fn verify_receipt(
        &self,
        conn: &Connection,
        run: i64,
        gate_seq: i64,
        pre_count: u32,
    ) -> Result<(), LoopError> {
        use super::RunOutcome;
        let Some(id) = self.exchange else {
            if self.verdict == "route"
                && self.artifact.is_none()
                && matches!(&self.terminal, Some(GdlOutcome::Escalated { at, .. }) if *at == self.phase)
                && self.errors.len() == 1
                && self.errors[0]
                    == format!(
                        "authority: phase {} is owned by a higher tier — escalating with the bundle",
                        self.phase.as_str()
                    )
            {
                return Ok(());
            }
            return Err(persist_error("GDL transition lacks exchange receipt"));
        };
        let start_key: String = conn
            .query_row(
                "SELECT idempotency_key FROM agent_session_events WHERE run_id=?1 AND seq=?2 AND kind='control:exchange'",
                params![run, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL exchange start missing"))?;
        let expected_key = format!(
            "control:exchange:{}",
            super::exchange_key(&self.episode, self.phase, self.attempt, pre_count)
        );
        if start_key != expected_key && start_key != format!("{expected_key}:confirm") {
            return Err(persist_error("GDL exchange key mismatch"));
        }
        let start = session_log::exact(conn, run, &start_key)
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL exchange start missing"))?;
        let done = session_log::exact(conn, run, &format!("control:exchange_done:{id}"))
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL exchange receipt missing"))?;
        let end: ExchangeEnd = serde_json::from_str(&done.payload_json)
            .map_err(|_| persist_error("corrupt GDL exchange receipt"))?;
        // The driver's fingerprint is "v2:<policy>:v2:<input>" (hash_text is
        // versioned); validate shape, not a pinned digest.
        let parts: Vec<&str> = start.payload_json.split(':').collect();
        let fingerprint_ok = parts.len() == 4
            && parts[0] == "v2"
            && parts[2] == "v2"
            && parts[1].len() == 64
            && parts[3].len() == 64
            && parts[1].bytes().all(|b| b.is_ascii_hexdigit())
            && parts[3].bytes().all(|b| b.is_ascii_hexdigit());
        if id <= 0
            || start.seq != id
            || start.kind != "control:exchange"
            || done.kind != "control:exchange_done"
            || end.version != 1
            || done.seq <= id
            || done.seq >= gate_seq
            || !fingerprint_ok
        {
            return Err(persist_error("GDL exchange identity/kind mismatch"));
        }
        let turns = match &end.outcome {
            RunOutcome::Completed { turns, .. }
            | RunOutcome::TurnCapReached { turns, .. }
            | RunOutcome::BudgetExceeded { turns, .. } => Some(*turns),
            RunOutcome::Canceled | RunOutcome::ProviderFailed { .. } => None,
        };
        let text = if let Some(key) = end.assistant_key {
            let turn = key
                .strip_prefix(&format!("run{run}:a{id}:t"))
                .and_then(|s| s.parse::<u32>().ok());
            if turn.is_none_or(|t| t == 0) || turns.is_some_and(|t| Some(t) != turn) {
                return Err(persist_error("GDL receipt assistant identity mismatch"));
            }
            let assistant = session_log::exact(conn, run, &key)
                .map_err(persist_error)?
                .ok_or_else(|| persist_error("GDL receipt assistant absent"))?;
            let value: serde_json::Value =
                serde_json::from_str(&assistant.payload_json).map_err(persist_error)?;
            if assistant.kind != "assistant" || assistant.seq <= id || assistant.seq >= done.seq {
                return Err(persist_error("GDL receipt assistant kind/order mismatch"));
            }
            value
                .get("text")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| persist_error("GDL receipt assistant text absent"))?
                .to_string()
        } else if turns.is_none_or(|t| t == 0) {
            String::new()
        } else {
            return Err(persist_error("GDL receipt assistant reference absent"));
        };
        let compatible = match (&end.outcome, &self.terminal) {
            (RunOutcome::Canceled, Some(GdlOutcome::Canceled)) => true,
            (
                RunOutcome::ProviderFailed { failure },
                Some(GdlOutcome::ProviderFailed {
                    failure: terminal_failure,
                    ..
                }),
            ) => failure == terminal_failure,
            (RunOutcome::TurnCapReached { .. }, Some(GdlOutcome::Capped { reason, .. })) => {
                reason == "turn_cap"
            }
            (RunOutcome::BudgetExceeded { .. }, Some(GdlOutcome::Capped { reason, .. })) => {
                reason == "budget"
            }
            (
                RunOutcome::Completed { .. },
                None
                | Some(
                    GdlOutcome::Resolved { .. }
                    | GdlOutcome::Routed { .. }
                    | GdlOutcome::Escalated { .. }
                    | GdlOutcome::VerifyFailed { .. },
                ),
            ) => true,
            _ => false,
        };
        if !compatible {
            return Err(persist_error("GDL outcome/exchange mismatch"));
        }
        if let Some(artifact) = &self.artifact {
            let (_, normalized) = super::accept_ungated(self.phase, &text);
            // The triage artifact carries the server stamp
            // (`confirmed_matches`) — the driver's deterministic record of
            // the gate-verified citation, not the model's bytes. Compare
            // the model-determined bytes, then check the stamp invariant
            // (stamp == the committed citation set).
            let model_bytes_match = match self.phase {
                super::GdlPhase::Triage => {
                    let committed: Option<super::TriageArtifact> =
                        serde_json::from_str(artifact).ok();
                    let receipt: Option<super::TriageArtifact> = normalized
                        .as_ref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    match (committed, receipt) {
                        (Some(mut c), Some(mut r)) => {
                            let stamped = c.confirmed_matches.clone();
                            c.confirmed_matches = None;
                            r.confirmed_matches = None;
                            c == r && stamped.as_ref() == Some(&c.search_hits)
                        }
                        _ => false,
                    }
                }
                _ => normalized.as_ref() == Some(artifact),
            };
            if self.verdict != "pass" || !model_bytes_match {
                return Err(persist_error("GDL gate/receipt artifact mismatch"));
            }
        } else if !matches!(self.verdict.as_str(), "route" | "fail") {
            return Err(persist_error("invalid GDL receipt gate verdict"));
        }
        Ok(())
    }
}

fn checked_audit(
    conn: &Connection,
    cp: &Checkpoint,
    status: crate::audit::AuditStatus,
    detail: &str,
) -> Result<(), LoopError> {
    crate::audit::record_tenant(
        conn,
        crate::audit::AuditKind::Workflow,
        crate::workflow::ACTOR,
        &cp.key(),
        status,
        detail,
        &cp.domain,
    )
    .ok_or_else(|| persist_error("GDL checked audit insertion failed"))?;
    Ok(())
}

struct Run {
    json: String,
    revision: i64,
    kind: String,
    status: String,
    domain: String,
}

fn read_run(conn: &Connection, run: i64) -> Result<Run, LoopError> {
    conn.query_row(
        "SELECT CASE WHEN length(CAST(state_json AS BLOB)) <= ?2 THEN state_json ELSE NULL END,
         state_revision, kind, status, domain FROM workflow_runs WHERE id=?1",
        params![run, session_log::PAYLOAD_CAP_BYTES as i64],
        |r| {
            Ok(Run {
                json: r.get(0)?,
                revision: r.get(1)?,
                kind: r.get(2)?,
                status: r.get(3)?,
                domain: r.get(4)?,
            })
        },
    )
    .map_err(persist_error)
}

/// Validation and acquisition share BEGIN IMMEDIATE. In particular the fresh
/// sentinel is checked BEFORE our own claim creates session history.
pub(super) fn admit(
    conn: &mut Connection,
    run: i64,
    ticket: &str,
    policy: &str,
    owner: &str,
) -> Result<Checkpoint, LoopError> {
    let mut tx = WorkflowTx::begin(conn).map_err(persist_error)?;
    let row = read_run(tx.tx(), run)?;
    let latest: Option<String> = tx.tx().query_row(
        "SELECT idempotency_key FROM agent_session_events WHERE run_id=?1 AND kind='control:gdl' ORDER BY seq DESC LIMIT 1",
        [run], |r| r.get(0),
    ).optional().map_err(persist_error)?;
    let cp = if let Some(key) = latest {
        let event = session_log::exact(tx.tx(), run, &key)
            .map_err(persist_error)?
            .ok_or_else(|| persist_error("GDL checkpoint absent"))?;
        let cp: Checkpoint = serde_json::from_str(&event.payload_json)
            .map_err(|_| persist_error("corrupt GDL checkpoint"))?;
        cp.bind(&row)?;
        cp.verify_link(tx.tx())?;
        if cp.run != run || cp.key() != key || cp.case.ticket != ticket || cp.policy != policy {
            return Err(persist_error("GDL invocation binding mismatch"));
        }
        if cp.terminal.is_none() {
            let pause = session_log::exact(tx.tx(), run, &format!("{}:pause", cp.key()))
                .map_err(persist_error)?
                .ok_or_else(|| {
                    persist_error("GDL interrupted; explicit quiescent pause required")
                })?;
            let last: i64 = tx
                .tx()
                .query_row(
                    "SELECT MAX(seq) FROM agent_session_events WHERE run_id=?1",
                    [run],
                    |r| r.get(0),
                )
                .map_err(persist_error)?;
            if pause.kind != "control:gdl_pause"
                || pause.payload_json != cp.json()?
                || last != pause.seq + 1
            {
                return Err(persist_error(
                    "GDL pause changed or intervening session work",
                ));
            }
        }
        session_log::acquire(tx.tx(), run, owner).map_err(persist_error)?;
        cp
    } else {
        let history: bool = tx.tx().query_row(
            "SELECT EXISTS(SELECT 1 FROM workflow_steps WHERE run_id=?1) OR EXISTS(SELECT 1 FROM agent_session_events WHERE run_id=?1)",
            [run], |r| r.get(0),
        ).map_err(persist_error)?;
        // CG-05 (C-10): a seeded split child is the other launchable
        // shape — the row the Intake split act inserted (revision 0,
        // active, a fresh Intake case carrying the parent link, never
        // driven). `{}` stays `{}`; every other non-`{}` byte still
        // refuses as corrupt or ambiguous legacy. The predicate is the
        // one the launch verb's pre-check reads (`seeded_split_child`)
        // — the two checks cannot drift.
        let is_seeded = row.json.trim() != "{}" && GdlCase::seeded_split_child(&row.json).is_some();
        if row.kind != "troubleshoot"
            || row.status != "active"
            || row.revision != 0
            || !(row.json.trim() == "{}" || is_seeded)
            || history
        {
            return Err(persist_error("corrupt state or ambiguous legacy GDL run"));
        }
        session_log::acquire(tx.tx(), run, owner).map_err(persist_error)?;
        // The child's ticket IS the approved question: a launch
        // that names another ticket is the invocation binding mismatch,
        // the same law the resume path enforces above.
        let case = if is_seeded {
            let child = GdlCase::seeded_split_child(&row.json)
                .ok_or_else(|| persist_error("corrupt state or ambiguous legacy GDL run"))?;
            if child.ticket != ticket {
                return Err(persist_error("GDL invocation binding mismatch"));
            }
            child
        } else {
            GdlCase::fresh(ticket)
        };
        let cp = Checkpoint {
            version: 1,
            run,
            domain: row.domain,
            policy: policy.into(),
            episode: uuid::Uuid::new_v4().to_string(),
            revision: 1,
            case,
            next_phase: Some(GdlPhase::Intake),
            attempt: 0,
            errors: Vec::new(),
            phases: 0,
            exchange: None,
            terminal: None,
            status: "active".into(),
        };
        state::cas_update(
            tx.tx(),
            run,
            0,
            &serde_json::to_string(&cp.case).map_err(persist_error)?,
            "active",
            chrono::Utc::now().timestamp(),
        )
        .map_err(persist_error)?;
        cp.store(tx.tx())?;
        checked_audit(
            tx.tx(),
            &cp,
            crate::audit::AuditStatus::Ok,
            "gdl_checkpoint_sealed",
        )?;
        cp
    };
    if cp.terminal.is_some() {
        session_log::release(tx.tx(), run, owner).map_err(persist_error)?;
    }
    tx.commit().map_err(persist_error)?;
    Ok(cp)
}

pub(super) struct Transition {
    pub phase: GdlPhase,
    pub attempt: u32,
    pub verdict: &'static str,
    pub errors: Vec<String>,
    pub artifact: Option<String>,
    pub exchange: Option<i64>,
    pub terminal: Option<GdlOutcome>,
    /// The exchange's provider-reported usage (input, output) — F12b's
    /// measurement payload on the `gdl_gate` row. Additive: old rows read
    /// as absent, and rows for receipt-less transitions (route-at-gate)
    /// carry None.
    pub usage: Option<(u64, u64)>,
    /// CG-04: the explicit forward target of this transition. `None`
    /// derives the legacy value (`ALL[phases]` — identical to the
    /// phase-based value at count == 0); the admitted back edge sets
    /// `Some(Hypothesize)`.
    pub next_phase: Option<GdlPhase>,
    /// CG-04: the rehypothesis count after this transition. The driver
    /// increments it ONLY on the admitted edge; every other transition
    /// carries the prior count.
    pub rehypothesis_count: u32,
}

/// Called only after the exchange has settled, never across an external await.
///
/// `satellites` carries the phase-pass rows that make this pass actionable —
/// the soft-handoff evaluation, the handoff offer and handover, the resolved
/// back-referral, the SLA-armed clock, the adversarial finding. They are
/// written INSIDE this transaction, not beside it: a rollback takes them with
/// the advance, and an advance cannot commit without them. `None` is for the
/// direct-call paths that drive a checkpoint with no case behind it.
pub(super) fn advance(
    conn: &mut Connection,
    old: &Checkpoint,
    owner: &str,
    change: Transition,
    satellites: Option<&super::Satellites>,
) -> Result<Checkpoint, LoopError> {
    let result = advance_tx(conn, old, owner, change, satellites);
    if result.is_err()
        && let Err(denial_error) = record_denial(conn, old, owner)
    {
        tracing::error!(error = ?denial_error, "GDL denial persistence failed; original checkpoint error retained");
    }
    result
}

fn record_denial(conn: &mut Connection, old: &Checkpoint, owner: &str) -> Result<(), LoopError> {
    // The failed transaction has dropped. append's normal Ok audit is
    // best-effort; the additional Denied row is REQUIRED in this transaction.
    let mut tx = WorkflowTx::begin(conn).map_err(persist_error)?;
    session_log::append(
        tx.tx(),
        old.run,
        "control:gdl_denied",
        &serde_json::json!({
            "version": 1, "episode": old.episode, "revision": old.revision,
            "reason": "checkpoint_refused"
        })
        .to_string(),
        &format!("{}:denied:{owner}", old.key()),
        chrono::Utc::now().timestamp(),
    )
    .map_err(persist_error)?;
    checked_audit(
        tx.tx(),
        old,
        crate::audit::AuditStatus::Denied,
        "gdl_checkpoint_refused",
    )?;
    tx.commit().map_err(persist_error)?;
    Ok(())
}

fn advance_tx(
    conn: &mut Connection,
    old: &Checkpoint,
    owner: &str,
    change: Transition,
    satellites: Option<&super::Satellites>,
) -> Result<Checkpoint, LoopError> {
    let mut tx = WorkflowTx::begin(conn).map_err(persist_error)?;
    old.verify(tx.tx(), owner)?;
    session_log::check_idle(tx.tx(), old.run).map_err(persist_error)?;
    if old.terminal.is_some()
        || old.next_phase != Some(change.phase)
        || change.attempt != old.attempt + 1
    {
        return Err(persist_error("invalid GDL transition"));
    }
    let mut next = old.clone();
    next.revision = old
        .revision
        .checked_add(1)
        .ok_or_else(|| persist_error("GDL revision overflow"))?;
    next.exchange = change.exchange;
    next.terminal = change.terminal.clone();
    next.case.phase = change.phase;
    if let Some(artifact) = &change.artifact {
        super::apply(&mut next.case, change.phase, artifact);
        write_phase(tx.tx(), old.run, change.phase, artifact)?;
        next.phases += 1;
        next.next_phase = change
            .next_phase
            .or_else(|| super::forward_phase(change.phase, next.case.hand_down.is_some()));
        next.case.rehypothesis_count = change.rehypothesis_count;
        next.attempt = 0;
        next.errors.clear();
    } else if change.verdict == "fail" {
        next.attempt = change.attempt;
        next.errors.clone_from(&change.errors);
    }
    // CG-05: the split act — a satellite of the Intake transition. A
    // gate-passed decomposition splits INSIDE this transaction (one
    // commit point: no window where a child is durable and the parent's
    // update is not). The exactly-once guard (`children` empty) keeps
    // the act idempotent across retries of the same transition — the
    // `open_run` sibling creation happens in this one function and
    // nowhere else (pins S-1/S-2). The parent IS issue-1's case; issues
    // 2..=N each get a REAL sibling run — an independent case with its
    // own chain, seeded from its approved question with the parent link
    // (two-pass: rowids first, then the full N-set on each child). The
    // machine never auto-drives them: they sit `active` until the CRANK
    // enumeration or an operator launch drives each.
    if change.phase == GdlPhase::Intake
        && next.case.decomposition.is_some()
        && next.case.children.is_empty()
    {
        let domain = next.domain.clone();
        let parent_run = old.run;
        let questions: Vec<String> = next
            .case
            .decomposition
            .as_ref()
            .map(|d| {
                d.issues
                    .iter()
                    .skip(1)
                    .map(|issue| issue.question.clone())
                    .collect()
            })
            .unwrap_or_default();
        // Pass 1: insert each child row (revision 0, active) and collect
        // its id. The state rides seeded, so `admit`'s sentinel accepts
        // it as a launched-child (C-10's `seeded_split_child`).
        let mut ids: Vec<i64> = Vec::new();
        for question in &questions {
            let mut child = GdlCase::fresh(question);
            child.parent_case_id = Some(parent_run);
            let child_json = serde_json::to_string(&child).map_err(persist_error)?;
            let id = state::open_run(
                tx.tx(),
                &domain,
                "troubleshoot",
                &child_json,
                chrono::Utc::now().timestamp(),
            )
            .map_err(persist_error)?;
            ids.push(id);
        }
        // Pass 2: the IDs are rowids only after every row exists — now
        // the full N-set goes onto each child's own state (the update
        // never touches the revision column: the child stays revision 0).
        for (index, id) in ids.iter().enumerate() {
            let mut child = GdlCase::fresh(&questions[index]);
            child.parent_case_id = Some(parent_run);
            child.sibling_ids = ids.clone();
            let child_json = serde_json::to_string(&child).map_err(persist_error)?;
            tx.tx()
                .execute(
                    "UPDATE workflow_runs SET state_json = ?1 WHERE id = ?2",
                    params![child_json, id],
                )
                .map_err(persist_error)?;
        }
        next.case.children = ids;
    }
    // The intake match — a satellite of the Intake transition, in
    // this transaction. Scans marked procedures in this run's
    // domain, procedure ascending, capped at the scan horizon. A
    // stale or corrupt candidate is skipped by name in the match
    // row; a scan error refuses the advance (never a silent
    // no-mark). The first containment match wins; the choice is
    // recorded in one match row.
    if change.phase == GdlPhase::Intake && change.verdict == "pass" && next.case.hand_down.is_none()
    {
        let marks = crate::workflow::hand_down::marked_procedures(tx.tx(), &next.domain)
            .map_err(|e| persist_error(format!("hand-down scan: {e}")))?;
        let mut skipped: Vec<i64> = Vec::new();
        for (index, mark) in marks.iter().enumerate() {
            let snapshot = match crate::workflow::hand_down::load_snapshot(tx.tx(), mark) {
                Ok(s) => s,
                Err(_) => {
                    skipped.push(mark.procedure_id);
                    continue;
                }
            };
            if crate::workflow::hand_down::ticket_matches(&next.case.ticket, &snapshot.ticket) {
                next.case.hand_down = Some(mark.clone());
                let payload = crate::workflow::hand_down::HandDownMatchPayload {
                    procedure_id: mark.procedure_id,
                    conformance_run_id: mark.conformance.run_id.clone(),
                    scan_position: index + 1,
                    snapshot_digest: mark.snapshot_digest.clone(),
                    skipped: skipped.clone(),
                };
                let payload_json = serde_json::to_string(&payload).map_err(persist_error)?;
                session_log::append(
                    tx.tx(),
                    old.run,
                    crate::workflow::hand_down::HAND_DOWN_MATCH_ROW_KIND,
                    &payload_json,
                    &format!("run{}:control:match:{}", old.run, mark.procedure_id),
                    chrono::Utc::now().timestamp(),
                )
                .map_err(persist_error)?;
                crate::workflow::audit_write(
                    tx.tx(),
                    old.run,
                    &format!("hand_down_match:{}", mark.procedure_id),
                    crate::audit::AuditStatus::Ok,
                    &format!(
                        "procedure={} conf={} scan_position={} digest={} skipped={:?}",
                        mark.procedure_id,
                        mark.conformance.run_id,
                        index + 1,
                        mark.snapshot_digest,
                        skipped
                    ),
                );
                break;
            }
        }
        // Re-derive the forward target now that the mark is known.
        if next.case.hand_down.is_some() && next.case.phase == GdlPhase::Intake {
            next.next_phase = super::forward_phase(GdlPhase::Intake, next.case.hand_down.is_some());
        }
    }
    // The narrow-loop injection — a satellite of the second-phase
    // transition. The sealed steps become the plan; the differential
    // stays empty because its owner phase never runs. The forward
    // target jumps straight to the execute phase.
    if change.phase == GdlPhase::Triage && change.verdict == "pass" && next.case.hand_down.is_some()
    {
        if !next.case.hypotheses.is_empty() {
            return Err(persist_error(
                "hand-down case carries hypotheses — the differential is never laundered",
            ));
        }
        let mark = next
            .case
            .hand_down
            .clone()
            .ok_or_else(|| persist_error("hand-down mark absent"))?;
        let snapshot = crate::workflow::hand_down::load_snapshot(tx.tx(), &mark)
            .map_err(|e| persist_error(format!("hand-down snapshot: {e}")))?;
        next.case.plan = snapshot.plan.clone();
        next.case.verify_step = Some(snapshot.verify_step.clone());
        next.case.dead_end = Some(snapshot.dead_end.clone());
        next.next_phase = Some(GdlPhase::Act);
    }
    if matches!(next.terminal, Some(GdlOutcome::Resolved { .. })) {
        next.status = "resolved".into();
    }
    next.validate()?;
    session_log::append(
        tx.tx(),
        old.run,
        "gdl_gate",
        &serde_json::to_string(&GateRecord {
            version: 1,
            phase: change.phase,
            verdict: change.verdict.into(),
            attempt: change.attempt,
            errors: change.errors,
            episode: old.episode.clone(),
            exchange: change.exchange,
            revision: old.revision,
            artifact: change.artifact,
            terminal: change.terminal,
            next_phase: next.next_phase,
            rehypothesis_count: next.case.rehypothesis_count,
            split_children: next.case.children.clone(),
            usage: change.usage,
            hand_down: next.case.hand_down.clone(),
        })
        .map_err(persist_error)?,
        &format!("{}:gate", next.key()),
        chrono::Utc::now().timestamp(),
    )
    .map_err(persist_error)?;
    state::cas_update(
        tx.tx(),
        old.run,
        old.revision,
        &serde_json::to_string(&next.case).map_err(persist_error)?,
        &next.status,
        chrono::Utc::now().timestamp(),
    )
    .map_err(persist_error)?;
    if matches!(next.terminal, Some(GdlOutcome::Resolved { .. })) {
        crate::workflow::proficiency::capture_proposals_on_resolve(
            &mut tx,
            old.run,
            &next.case,
            chrono::Utc::now().timestamp(),
        )
        .map_err(persist_error)?;
        // The after-action reflection capture: retrospective-only, derives
        // from the sealed case's audited rows, and lands its additive rows
        // in THIS transaction so the capture is atomic with the closure it
        // reflects. It can only add rows; the outcome above is already
        // sealed.
        crate::workflow::reflection::capture_on_resolve(
            &mut tx,
            old.run,
            &next.case,
            owner,
            chrono::Utc::now().timestamp(),
        )
        .map_err(persist_error)?;
    }
    next.store(tx.tx())?;
    next.verify_link(tx.tx())?;
    let audit_detail = if matches!(next.terminal, Some(GdlOutcome::ProviderFailed { .. })) {
        "gdl_provider_failed"
    } else {
        "gdl_checkpoint_sealed"
    };
    checked_audit(tx.tx(), &next, crate::audit::AuditStatus::Ok, audit_detail)?;
    // The phase pass's own satellites — the rows that make THIS pass
    // actionable for the operator — commit in this transaction, after the
    // checkpoint they belong to and before the release. Writing them here
    // rather than beside the transaction is the whole point: there is no
    // longer an ordering in which one is durable and the other is not. A
    // refusal below aborts the advance along with them.
    if let Some(satellites) = satellites {
        satellites.write_in_tx(tx.tx())?;
    }
    if next.terminal.is_some() {
        session_log::release(tx.tx(), old.run, owner).map_err(persist_error)?;
    }
    tx.commit().map_err(persist_error)?;
    Ok(next)
}

pub(super) fn provider_failure_audit_count(conn: &Connection) -> Result<i64, LoopError> {
    conn.query_row(
        "SELECT COUNT(*) FROM audit_events WHERE kind='workflow' AND detail_hash=?1",
        [crate::audit::hash("gdl_provider_failed")],
        |row| row.get(0),
    )
    .map_err(persist_error)
}

pub(super) fn provider_failed_terminal(conn: &Connection, run: i64) -> Result<bool, LoopError> {
    let key: Option<String> = conn
        .query_row(
            "SELECT idempotency_key FROM agent_session_events WHERE run_id=?1 AND kind='control:gdl' ORDER BY seq DESC LIMIT 1",
            [run],
            |row| row.get(0),
        )
        .optional()
        .map_err(persist_error)?;
    let Some(key) = key else {
        return Ok(false);
    };
    let event = session_log::exact(conn, run, &key)
        .map_err(persist_error)?
        .ok_or_else(|| persist_error("GDL checkpoint absent"))?;
    if event.kind != "control:gdl" {
        return Err(persist_error("GDL checkpoint kind mismatch"));
    }
    let checkpoint: Checkpoint = serde_json::from_str(&event.payload_json)
        .map_err(|_| persist_error("corrupt GDL checkpoint"))?;
    Ok(matches!(
        checkpoint.terminal,
        Some(GdlOutcome::ProviderFailed { .. })
    ))
}

pub(super) fn pause(conn: &mut Connection, cp: &Checkpoint, owner: &str) -> Result<(), LoopError> {
    let mut tx = WorkflowTx::begin(conn).map_err(persist_error)?;
    cp.verify(tx.tx(), owner)?;
    session_log::check_idle(tx.tx(), cp.run).map_err(persist_error)?;
    if cp.terminal.is_some() {
        return Err(persist_error("terminal GDL cannot pause"));
    }
    let pending: bool = tx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM agent_session_events e WHERE e.run_id=?1 AND e.kind='control:exchange'
         AND NOT EXISTS(SELECT 1 FROM agent_session_events d WHERE d.run_id=e.run_id
         AND d.idempotency_key='control:exchange_done:' || e.seq AND d.kind='control:exchange_done'))",
        [cp.run], |r| r.get(0),
    ).map_err(persist_error)?;
    if pending {
        return Err(persist_error("unsettled exchange cannot pause"));
    }
    session_log::append(
        tx.tx(),
        cp.run,
        "control:gdl_pause",
        &cp.json()?,
        &format!("{}:pause", cp.key()),
        chrono::Utc::now().timestamp(),
    )
    .map_err(persist_error)?;
    session_log::release(tx.tx(), cp.run, owner).map_err(persist_error)?;
    tx.commit().map_err(persist_error)?;
    Ok(())
}

fn write_phase(
    conn: &Connection,
    run: i64,
    phase: GdlPhase,
    artifact: &str,
) -> Result<(), LoopError> {
    let name = phase.as_str();
    conn.execute(
        "INSERT INTO workflow_steps(run_id, phase, step_key, state_json) VALUES (?1, ?2, ?2, ?3)",
        params![run, name, artifact],
    )
    .map_err(persist_error)?;
    let parent = conn.last_insert_rowid();
    crate::workflow::audit_write(
        conn,
        run,
        &format!("step:{name}"),
        crate::audit::AuditStatus::Ok,
        &format!("gdl phase {name} passed"),
    );
    if phase == GdlPhase::Act {
        let rows: super::ActArtifact = serde_json::from_str(artifact).map_err(persist_error)?;
        for row in rows.rows {
            let key = format!("act-{}", row.order);
            conn.execute("INSERT INTO workflow_steps(run_id, phase, step_key, state_json, parent_step_id) VALUES (?1, 'act', ?2, ?3, ?4)", params![run, key, serde_json::to_string(&row).map_err(persist_error)?, parent]).map_err(persist_error)?;
            crate::workflow::audit_write(
                conn,
                run,
                &format!("step:{key}"),
                crate::audit::AuditStatus::Ok,
                &format!("test-log row {} persisted", row.order),
            );
        }
    }
    let batch = super::typed_evidence_for(phase, artifact, chrono::Utc::now().timestamp());
    if !batch.is_empty() {
        crate::workflow::evidence::record(conn, run, &batch, None)
            .map_err(|e| persist_error(format!("evidence: {e:?}")))?;
    }
    Ok(())
}
