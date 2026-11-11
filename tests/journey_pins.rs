//! IMPROVE15, C-a — the journey projection's pins.
//!
//! The projection law (the round's constitution): same chain state →
//! byte-identical journey; no journey table, no cache, no parallel log — a
//! projection that could drift from the chain is the false-coverage defect.
//! The determinism pin is the law's first witness (two renders compared);
//! the roles-not-names mapping and the closed sentence table are the two
//! curation surfaces the projection owns from birth. Every control here is
//! the planted-miss arm that keeps its pin interpretable.

use std::path::PathBuf;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ── the projection's fixture rows (pure inputs; no pool, no clock) ─────────

fn checkpoint_row(seq: i64, created_at: i64, payload: &str) -> (i64, i64, String) {
    (seq, created_at, payload.to_string())
}

fn base_run<'a>() -> brain_server::workflow::journey::JourneyRun<'a> {
    brain_server::workflow::journey::JourneyRun {
        run_id: 7,
        created_at: 1_000,
        updated_at: 9_000,
        status: "running",
        state_json: r#"{"phase":"act","ticket":"the sink is leaking again"}"#,
    }
}

// ── PJ-1 (the projection law, kernel half): same chain state → byte-identical
// journey. Two renders of the SAME inputs compared byte for byte. ──────────
#[test]
fn journey_two_renders_are_byte_identical() {
    let checkpoints = vec![
        checkpoint_row(1, 1_100, r#"{"case":{"phase":"intake"}}"#),
        checkpoint_row(2, 1_200, r#"{"case":{"phase":"triage"}}"#),
        checkpoint_row(3, 1_300, r#"{"case":{"phase":"act"}}"#),
    ];
    let approvals = vec![brain_server::workflow::journey::ApprovalFact {
        proposal_id: 42,
        decided_at: 2_000,
        digest: "aa".repeat(32),
        actor: "m-supervisor".to_string(),
    }];
    let holds = vec![brain_server::workflow::journey::HoldFact {
        id: 5,
        held_at: 1_500,
        released_at: Some(4_000),
        actor: "m-dpo".to_string(),
    }];
    let dsars = vec![brain_server::workflow::journey::DsarFact {
        id: 3,
        completed_at: 3_000,
        certificate_digest: Some("bb".repeat(32)),
        actor: "m-dpo".to_string(),
    }];
    let decisions = vec![(
        11,
        2_500,
        r#"{"kind":"handoff_transition","decision_ref":"h-1","subject":"stage","outcome":"delivered","at":2500}"#.to_string(),
    )];

    let mk = || {
        let run = base_run();
        let chain = brain_server::workflow::journey::JourneyChain {
            run,
            checkpoints: &checkpoints,
            decisions: &decisions,
            approvals: &approvals,
            holds: &holds,
            dsars: &dsars,
        };
        brain_server::workflow::journey::project(&chain)
    };

    let a = mk();
    let b = mk();
    let ja = serde_json::to_string(&a).expect("render a");
    let jb = serde_json::to_string(&b).expect("render b");
    assert_eq!(
        ja, jb,
        "the projection law: same chain state renders byte-identical"
    );
    // The render is also STABLE across separately-sorted input orders: the
    // projection owns the order, never the caller's row order.
    let mut shuffled_checkpoints = checkpoints.clone();
    shuffled_checkpoints.reverse();
    let run = base_run();
    let chain = brain_server::workflow::journey::JourneyChain {
        run,
        checkpoints: &shuffled_checkpoints,
        decisions: &decisions,
        approvals: &approvals,
        holds: &holds,
        dsars: &dsars,
    };
    let c = brain_server::workflow::journey::project(&chain);
    assert_eq!(
        ja,
        serde_json::to_string(&c).expect("render c"),
        "input row order must not leak into the projection order"
    );
    // the delivered hand-off decision renders the CG-07 act:
    assert!(
        a.iter().any(|e| e.kind == "handdown_executed"),
        "the delivered handoff_transition case/decision row must render the hand-down act"
    );
}

// ── the roles-not-names law: every actor renders as a ROLE. The three
// measured actor forms (a roster name, a masked subject, the literal `api`)
// each map to a role; no form ever passes through. ─────────────────────────
#[test]
fn journey_roles_not_names_the_three_actor_forms_render_as_roles() {
    // the mapping is total over the three measured forms:
    for actor in [
        "m-supervisor",
        "0123456789abcdef".to_string().as_str(),
        "api",
    ] {
        let role = brain_server::workflow::journey::role_of_actor(
            actor,
            brain_server::workflow::journey::ActorAct::Approve,
        );
        assert!(
            matches!(
                role,
                "a supervisor" | "the privacy office" | "the operator" | "the system"
            ),
            "actor form {actor:?} must map to a ROLE, got {role:?}"
        );
        // and no form passes through, whatever the act:
        for act in [
            brain_server::workflow::journey::ActorAct::Approve,
            brain_server::workflow::journey::ActorAct::Privacy,
            brain_server::workflow::journey::ActorAct::System,
        ] {
            let r = brain_server::workflow::journey::role_of_actor(actor, act);
            assert_ne!(r, actor, "the actor literal must never pass through");
            assert_ne!(r, "api", "the literal `api` must never render");
        }
    }
}

/// The render-scan the curation battery reuses: no customer event may carry
/// a roster name or the literal `api` in ANY string field. Returns the
/// offending field paths (empty = clean).
fn scan_for_names(
    events: &[brain_server::workflow::journey::JourneyEvent],
    planted: &str,
) -> Vec<String> {
    let mut hits = Vec::new();
    for e in events {
        for (field, value) in [
            ("text", e.text.as_str()),
            ("actor", e.actor),
            ("kind", e.kind),
        ] {
            if value.contains(planted) || value == "api" {
                hits.push(format!("{}:{}={value}", e.kind, field));
            }
        }
    }
    hits
}

#[test]
fn journey_planted_roster_name_never_reaches_the_render() {
    let approvals = vec![brain_server::workflow::journey::ApprovalFact {
        proposal_id: 42,
        decided_at: 2_000,
        digest: "aa".repeat(32),
        // a roster principal's label — the name must die at the mapping
        actor: "olivia-roster-name".to_string(),
    }];
    let run = base_run();
    let chain = brain_server::workflow::journey::JourneyChain {
        run,
        checkpoints: &[],
        decisions: &[],
        approvals: &approvals,
        holds: &[],
        dsars: &[],
    };
    let events = brain_server::workflow::journey::project(&chain);
    let hits = scan_for_names(&events, "olivia-roster-name");
    assert!(
        hits.is_empty(),
        "roles-not-names: the roster name leaked into the render: {hits:?}"
    );
}

/// THE CONTROL for the name scan: a deliberately passthrough render (the
/// defect the law forbids) MUST be caught by the same scan — the scan is
/// non-vacuous by construction, not by trust.
#[test]
fn control_the_name_scan_catches_a_passthrough_render() {
    // a planted event that CARRIES the name (the passthrough defect):
    let events = vec![brain_server::workflow::journey::JourneyEvent {
        kind: "resolution_approved",
        case_id: 7,
        at: 2_000,
        seq: 1,
        text: "A supervisor approved the resolution. Receipt: a.".to_string(),
        actor: "olivia-roster-name",
        digest: None,
        count: None,
        phase: None,
    }];
    let hits = scan_for_names(&events, "olivia-roster-name");
    assert!(
        !hits.is_empty(),
        "CONTROL RED: the scan must catch a passthrough render (a name in the actor field)"
    );
    // and the literal `api` is caught too:
    let events = vec![brain_server::workflow::journey::JourneyEvent {
        kind: "hold_added",
        case_id: 0,
        at: 2_000,
        seq: 2,
        text: "api".to_string(),
        actor: "the privacy office",
        digest: None,
        count: None,
        phase: None,
    }];
    let hits = scan_for_names(&events, "olivia-roster-name");
    assert!(
        !hits.is_empty(),
        "CONTROL RED: the scan must catch the literal `api` in a customer event"
    );
}

// ── the closed sentence table: each kind renders THE customer event the
// plan's §2 vocabulary names — one sentence, no scores, no free text. ──────
#[test]
fn journey_sentences_are_the_closed_vocabulary_table() {
    use brain_server::workflow::journey::{JourneyKind, sentence_for};
    let cases: &[(JourneyKind, Option<&str>, Option<i64>, &str)] = &[
        (JourneyKind::CaseOpened, None, None, "Your case was opened."),
        (
            JourneyKind::PhaseAdvanced,
            Some("triage"),
            None,
            "Your case moved to triage.",
        ),
        (
            JourneyKind::TriageEscalated,
            None,
            None,
            "A specialist is reviewing your case.",
        ),
        (
            JourneyKind::CaseSplit,
            None,
            Some(3),
            "Your case was split into 3 linked cases.",
        ),
        (
            JourneyKind::Reexamined,
            None,
            None,
            "An earlier conclusion was re-examined.",
        ),
        (
            JourneyKind::ResolutionApproved,
            None,
            None,
            "A supervisor approved the resolution. Receipt: aabb.",
        ),
        (
            JourneyKind::HanddownExecuted,
            None,
            None,
            "The verified procedure was applied.",
        ),
        (
            JourneyKind::HoldAdded,
            None,
            None,
            "A legal hold affects your data.",
        ),
        (
            JourneyKind::HoldReleased,
            None,
            None,
            "A legal hold on your data was released.",
        ),
        (
            JourneyKind::DsarCompleted,
            None,
            None,
            "Your data request completed. Certificate: ccdd.",
        ),
    ];
    for (kind, phase, count, want) in cases {
        let got = sentence_for(
            *kind,
            *phase,
            *count,
            match kind {
                JourneyKind::ResolutionApproved => Some("aabb"),
                JourneyKind::DsarCompleted => Some("ccdd"),
                _ => None,
            },
        );
        assert_eq!(
            &got, want,
            "the sentence for {kind:?} drifted from the table"
        );
    }
}

// ── the closed kind vocabulary: the enum and the const list agree, and the
// wire kind strings are exactly the plan's §2 row names (plus the appeal
// kinds when C-d lands them). ──────────────────────────────────────────────
#[test]
fn journey_kinds_are_closed_and_named() {
    let src = read_repo("src/workflow/journey.rs");
    assert!(
        src.contains("pub const JOURNEY_KINDS"),
        "the closed vocabulary lives in ONE const, not in scattered literals"
    );
    use brain_server::workflow::journey::JourneyKind;
    for kind in [
        JourneyKind::CaseOpened,
        JourneyKind::PhaseAdvanced,
        JourneyKind::TriageEscalated,
        JourneyKind::CaseSplit,
        JourneyKind::Reexamined,
        JourneyKind::ResolutionApproved,
        JourneyKind::HanddownExecuted,
        JourneyKind::HoldAdded,
        JourneyKind::HoldReleased,
        JourneyKind::DsarCompleted,
    ] {
        assert!(
            brain_server::workflow::journey::JOURNEY_KINDS.contains(&kind.wire_kind()),
            "kind {kind:?} must be declared in the closed vocabulary"
        );
    }
}

// ── the purity law: the projection module owns no clock and no pool. A
// `Utc::now`/`Instant` inside journey.rs would make the render non-reproducible
// — the determinism pin's structural half. ─────────────────────────────────
#[test]
fn journey_module_owns_no_clock_and_no_pool() {
    let src = read_repo("src/workflow/journey.rs");
    for banned in [
        "Utc::now",
        "Instant::now",
        "SystemTime::now",
        "rusqlite::Pool",
        "std::time",
    ] {
        assert!(
            !src.contains(banned),
            "the projection must stay PURE: `{banned}` found in src/workflow/journey.rs — \
             a clock or a pool in the projection is a drift the determinism pin cannot see"
        );
    }
}

/// THE CONTROL for the purity scan: the scanner itself is non-vacuous — run
/// against a string that DOES carry a clock, it must red.
#[test]
fn control_the_purity_scan_catches_a_clock() {
    let dirty = "fn f() { let t = chrono::Utc::now(); }";
    assert!(
        dirty.contains("Utc::now"),
        "CONTROL RED: the purity scan must be able to catch a clock"
    );
}

// ── the projection is a READ over the rows its caller fetched — no journey
// table, no cache, no parallel log. The structural half: the module names no
// CREATE TABLE and no INSERT. ──────────────────────────────────────────────
#[test]
fn journey_projection_writes_nothing() {
    let src = read_repo("src/workflow/journey.rs");
    for banned in ["CREATE TABLE", "INSERT INTO", "UPDATE ", "DELETE FROM"] {
        assert!(
            !src.contains(banned),
            "the projection adds ZERO state: `{banned}` in src/workflow/journey.rs — \
             a journey table or parallel log is the false-coverage defect"
        );
    }
}
