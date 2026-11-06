//! The ranked differential over a GDL case's committed hypotheses
//! (CG-03, blind-spot 2). The acts are ordered; until this module the
//! hypotheses they test were not.
//!
//! ADVISORY BY ORDERING: the rank decides what is tested FIRST; it never
//! passes or fails a gate by itself, never reads the model's subjective
//! `confidence` (the `Hypothesis.confidence` law: no gate reads it, no
//! threshold binds it), and never touches `CertaintyGrade` (a case-level
//! opinion; per-hypothesis strength is re-derived from the same grounded
//! inputs — CG-03 C-2). The model proposes hypotheses; this order is
//! deterministic index arithmetic and string containment over committed
//! case state. Score is `i32` only — never a float.
//!
//! NOT this: [`super::confidence`] is the model-confidence -> human-deferral
//! CLASS policy. Both share the house shape: pure, conservative until
//! measured.

use super::gdl::{
    GdlCase, Hypothesis, MIN_CITATION_LOCATOR_CHARS, Verdict, citation_universe, kind_source,
    locator_resolves, red_flag_is_open,
};

/// Dominance weights (PROPOSED, not measured — CG-06's eval judges them;
/// changing them is a named act, never a tweak). Each weight exceeds the
/// maximum the lower weights can jointly contribute, so a flip in a
/// higher arm always moves the rank (what makes every arm red-provable):
/// falsified (`-400`) always below non-falsified (max `-102`, below
/// zero); worst-case (`+200`) always above non-worst non-falsified
/// (min `200`, above max `98`); testable (`+50`) always above source
/// strength alone (max `48`).
const W_FALSIFIED: i32 = 400;
const W_WORST: i32 = 200;
const W_TESTABLE: i32 = 50;
const W_KIND: i32 = 5;
const W_SOURCE: i32 = 1;

/// The ranked differential: the case's hypotheses reordered so rank 1
/// (first to test) is first, each carrying its gap-free `rank`
/// (`rank == position + 1`). Pure over committed case state (the
/// `citation_universe` discipline — nothing from the current pass's raw
/// model bytes). Called only from `gdl::apply`'s Hypothesize arm.
pub(crate) fn ranked(case: &GdlCase) -> Vec<Hypothesis> {
    let universe = citation_universe(case);
    let scores: Vec<i32> = case
        .hypotheses
        .iter()
        .map(|h| score_hypothesis(case, h, &universe))
        .collect();
    let mut order: Vec<usize> = (0..case.hypotheses.len()).collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .cmp(&scores[a])
            .then_with(|| {
                case.hypotheses[a]
                    .statement
                    .trim()
                    .to_lowercase()
                    .cmp(&case.hypotheses[b].statement.trim().to_lowercase())
            })
            .then_with(|| {
                case.hypotheses[a]
                    .prediction
                    .trim()
                    .to_lowercase()
                    .cmp(&case.hypotheses[b].prediction.trim().to_lowercase())
            })
            .then_with(|| a.cmp(&b))
    });
    order
        .into_iter()
        .enumerate()
        .map(|(pos, i)| {
            let mut h = case.hypotheses[i].clone();
            h.rank = Some((pos + 1) as u16);
            h
        })
        .collect()
}

/// One hypothesis's integer score under the dominance weights.
fn score_hypothesis(case: &GdlCase, h: &Hypothesis, universe: &[String]) -> i32 {
    let mut score: i32 = 0;
    if is_falsified(case, h) {
        score -= W_FALSIFIED;
    }
    if is_worst_adjacent(case, h) {
        score += W_WORST;
    }
    if is_testable(h, universe) {
        score += W_TESTABLE;
    }
    score += W_KIND * distinct_kinds(h) as i32;
    score += W_SOURCE * distinct_sources(h) as i32;
    score
}

/// Distinct evidence kinds over the hypothesis's sources (the
/// `hypothesis_status` vocabulary — kind-prefixed locators only).
fn distinct_kinds(h: &Hypothesis) -> usize {
    let mut kinds: Vec<&'static str> = h
        .sources
        .iter()
        .filter_map(|s| kind_source(s))
        .map(super::evidence::EvidenceKind::prefix)
        .collect();
    kinds.sort();
    kinds.dedup();
    kinds.len()
}

/// Distinct non-empty trimmed source strings (case-sensitive — a source
/// is an opaque locator, not prose).
fn distinct_sources(h: &Hypothesis) -> usize {
    let mut seen: Vec<&str> = h
        .sources
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    seen.sort();
    seen.dedup();
    seen.len()
}

/// Testable iff the prediction differs from the statement (a restated
/// prediction cannot fail) and at least one prediction needle resolves
/// into held evidence.
fn is_testable(h: &Hypothesis, universe: &[String]) -> bool {
    if h.prediction.trim().to_lowercase() == h.statement.trim().to_lowercase() {
        return false;
    }
    needles(&h.prediction)
        .iter()
        .any(|n| locator_resolves(n, universe))
}

/// Specific needles for the Plan gate's needle-order binding (S2):
/// the step's content words minus the ticket's topic words, so a
/// shared topic word (`vd5`) can never make every step "test" every
/// hypothesis.
pub(crate) fn specific_step_needles(ticket: &str, step: &super::gdl::PlanStep) -> Vec<String> {
    specific_needles(
        &format!("{} {} {}", step.description, step.command, step.expected),
        ticket,
    )
}

/// Specific needles for one hypothesis (its statement + prediction,
/// minus the ticket's topic words) — the text the falsified/worst
/// arms and the Plan gate match on.
pub(crate) fn specific_hypothesis_needles(ticket: &str, h: &Hypothesis) -> Vec<String> {
    specific_needles(&format!("{} {}", h.statement, h.prediction), ticket)
}

/// Needles of `text` that are NOT ticket topic words. Ticket words
/// are the case topic and appear in every hypothesis/row — matching
/// on them discriminates nothing (the S1 red-proof: both the
/// worst-case and falsified arms fired for every hypothesis via the
/// shared `vd5`). Empty means "no discriminating vocabulary" and the
/// caller must treat it as no-boost/no-demotion (conservative).
fn specific_needles(text: &str, ticket: &str) -> Vec<String> {
    let topic = needles(ticket);
    needles(text)
        .into_iter()
        .filter(|n| !topic.contains(n))
        .collect()
}

/// Worst-case adjacent iff the flag is OPEN and a worst-case-specific
/// needle appears in the hypothesis's statement or prediction (the ESI
/// must-not-miss reading — it stops pulling once ruled out).
fn is_worst_adjacent(case: &GdlCase, h: &Hypothesis) -> bool {
    let Some(triage) = &case.triage else {
        return false;
    };
    if !red_flag_is_open(&triage.red_flag) {
        return false;
    }
    let worst_specific = specific_needles(&triage.red_flag.worst_case, &case.ticket);
    if worst_specific.is_empty() {
        return false;
    }
    let haystack = format!("{} {}", h.statement, h.prediction).to_lowercase();
    worst_specific.iter().any(|n| haystack.contains(n.as_str()))
}

/// Falsified iff a hypothesis-specific needle appears in the text of a
/// `Fail` test-log row (vacuous on the first pass — the log is empty
/// until Act; CG-04's re-entry exercises it for real).
fn is_falsified(case: &GdlCase, h: &Hypothesis) -> bool {
    let needles = specific_hypothesis_needles(&case.ticket, h);
    if needles.is_empty() {
        return false;
    }
    case.test_log.iter().any(|row| {
        if row.verdict != Verdict::Fail {
            return false;
        }
        let mut text = format!("{} {} ", row.description, row.expected);
        if let Some(actual) = &row.actual {
            text.push_str(actual);
            text.push(' ');
        }
        text.push_str(&row.dtfvc.diagnose);
        text.push(' ');
        text.push_str(&row.dtfvc.test);
        for slot in [&row.dtfvc.fix, &row.dtfvc.verify, &row.dtfvc.capture]
            .into_iter()
            .flatten()
        {
            text.push(' ');
            text.push_str(slot);
        }
        let text = text.to_lowercase();
        needles.iter().any(|n| text.contains(n.as_str()))
    })
}

/// Needle extraction: lowercase whitespace tokens with non-alphanumeric
/// edges trimmed, kept at the citation floor or longer, deduped in
/// order. Extraction arithmetic, not model judgment (the
/// `certainty::ticket_locators` discipline, re-implemented here
/// because it is private to `certainty.rs`).
fn needles(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in text.to_lowercase().split_whitespace() {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric());
        if token.chars().count() >= MIN_CITATION_LOCATOR_CHARS && !out.iter().any(|s| s == token) {
            out.push(token.to_string());
        }
    }
    out
}

#[allow(clippy::too_many_lines)]
#[cfg(test)]
mod tests {
    use super::*;

    fn hyp(statement: &str, prediction: &str, sources: &[&str]) -> Hypothesis {
        serde_json::from_str(&format!(
            r#"{{"statement": {statement:?}, "prediction": {prediction:?}, "sources": {}}}"#,
            serde_json::to_string(sources).unwrap()
        ))
        .unwrap()
    }

    fn case_with(ticket: &str, triage_json: Option<&str>, hypotheses: Vec<Hypothesis>) -> GdlCase {
        let mut case = GdlCase::fresh(ticket);
        if let Some(t) = triage_json {
            case.triage = Some(serde_json::from_str(t).unwrap());
        }
        case.hypotheses = hypotheses;
        case
    }

    const OPEN_TRIAGE: &str = r#"{"priority": "P3", "verdict": "accept",
        "red_flag": {"worst_case": "data loss on vd5", "ruled_out": false}}"#;
    const CLOSED_TRIAGE: &str = r#"{"priority": "P3", "verdict": "accept",
        "red_flag": {"worst_case": "data loss on vd5", "ruled_out": true,
                     "rule_out_basis": ["verification:sel clean"]}}"#;

    fn statements(ranked: &[Hypothesis]) -> Vec<&str> {
        ranked.iter().map(|h| h.statement.as_str()).collect()
    }

    /// R-1: determinism — same case bytes rank equal twice, and the
    /// ranked output round-trips through serde equal.
    #[test]
    fn r1_same_case_ranks_equal_twice() {
        let case = case_with(
            "vd5 stuck rebuild on node-042",
            Some(OPEN_TRIAGE),
            vec![
                hyp(
                    "battery dead",
                    "vd5 battery reports Failed",
                    &["test:sel://events"],
                ),
                hyp(
                    "rebuild stalled",
                    "vd5 rebuild completes",
                    &["test:sel://events", "actual:write-cache"],
                ),
                hyp("data loss on vd5", "vd5 data lost", &["test:sel://events"]),
            ],
        );
        let a = ranked(&case);
        let b = ranked(&case);
        assert_eq!(a, b, "same case bytes must rank equal");
        let bytes = serde_json::to_string(&case).unwrap();
        let back: GdlCase = serde_json::from_str(&bytes).unwrap();
        assert_eq!(ranked(&back), a, "round-tripped case must rank equal");
    }

    /// R-2 arm (kinds): two distinct kinds outrank one, all else equal.
    #[test]
    fn r2_more_kinds_lift_rank() {
        let case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp("one kind", "vd5 rebuild completes", &["test:sel://events"]),
                hyp(
                    "two kinds",
                    "vd5 rebuild completes",
                    &["test:sel://events", "actual:write-cache"],
                ),
            ],
        );
        assert_eq!(statements(&ranked(&case))[0], "two kinds");
    }

    /// R-2 arm (sources): two distinct sources of one kind outrank one.
    #[test]
    fn r2_more_sources_lift_rank() {
        let case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp(
                    "one source",
                    "vd5 rebuild completes",
                    &["test:sel://events"],
                ),
                hyp(
                    "two sources",
                    "vd5 rebuild completes",
                    &["test:sel://events", "test:tsr://node-042"],
                ),
            ],
        );
        assert_eq!(statements(&ranked(&case))[0], "two sources");
    }

    /// R-2 arm (testability): a falsifiable, evidence-resolving
    /// prediction outranks a restated one, sources held equal.
    #[test]
    fn r2_testable_prediction_lifts_rank() {
        let case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp(
                    "vd5 rebuild completes",
                    "vd5 rebuild completes",
                    &["test:sel://events"],
                ),
                hyp(
                    "vd5 rebuild stalled",
                    "vd5 rebuild reports Failed",
                    &["test:sel://events"],
                ),
            ],
        );
        assert_eq!(statements(&ranked(&case))[0], "vd5 rebuild stalled");
    }

    /// R-2 arm (worst-case): the adjacent hypothesis leads while the
    /// flag is OPEN, and the lift reverses once the flag closes.
    /// (`zebra` vs `rebuild`: the tie-break alone would put rebuild
    /// first, so the closed order proves the boost is gone.)
    #[test]
    fn r2_worst_case_adjacency_lifts_only_while_open() {
        let hyps = || {
            vec![
                hyp(
                    "rebuild stalled",
                    "vd5 rebuild completes",
                    &["test:sel://events"],
                ),
                hyp("zebra data loss", "vd5 data lost", &["test:sel://events"]),
            ]
        };
        let open = case_with("vd5 rebuild", Some(OPEN_TRIAGE), hyps());
        assert_eq!(
            statements(&ranked(&open))[0],
            "zebra data loss",
            "open flag: worst-case neighbour first"
        );
        let closed = case_with("vd5 rebuild", Some(CLOSED_TRIAGE), hyps());
        assert_eq!(
            statements(&ranked(&closed))[0],
            "rebuild stalled",
            "closed flag: the lift reverses (tie-break alone would put rebuild first)"
        );
    }

    /// R-2 arm (falsified): a hypothesis whose needle sits in a `Fail`
    /// row demotes below an otherwise equal one.
    #[test]
    fn r2_falsified_row_demotes_rank() {
        let mut case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp(
                    "battery fault",
                    "vd5 battery reports Failed",
                    &["test:sel://events"],
                ),
                hyp(
                    "rebuild stalled",
                    "vd5 rebuild completes",
                    &["test:sel://events"],
                ),
            ],
        );
        // Row text carries `battery`/`fault` but NOT the shared ticket
        // word `vd5` — otherwise both hypotheses share the falsified
        // needle and neither demotes relative to the other.
        case.test_log = vec![
            serde_json::from_str(
                r#"{"order": 1, "kind": "check",
                "description": "battery fault reproduced",
                "expected": "Ready", "actual": "Failed", "verdict": "fail"}"#,
            )
            .unwrap(),
        ];
        assert_eq!(
            statements(&ranked(&case))[0],
            "rebuild stalled",
            "the falsified hypothesis must demote"
        );
        // Control: a Pass row with the same text demotes nothing.
        case.test_log[0].verdict = Verdict::Pass;
        assert_eq!(
            statements(&ranked(&case))[0],
            "battery fault",
            "a Pass row must not demote (tie falls to statement order)"
        );
    }

    /// R-3: dominance boundaries — a falsified worst-case hyp still
    /// ranks below a non-falsified one; a worst-case hyp outranks a
    /// max-source non-worst one.
    #[test]
    fn r3_dominance_boundaries_hold() {
        // Falsified + worst-adjacent vs plain non-falsified.
        let mut case = case_with(
            "vd5 rebuild",
            Some(OPEN_TRIAGE),
            vec![
                hyp(
                    "data loss on vd5",
                    "vd5 data lost",
                    &["test:sel://events", "actual:write-cache"],
                ),
                hyp(
                    "rebuild stalled",
                    "vd5 rebuild completes",
                    &["test:sel://events"],
                ),
            ],
        );
        // Row text carries `data`/`loss` but NOT the shared `vd5` —
        // see the battery fixture above.
        case.test_log = vec![
            serde_json::from_str(
                r#"{"order": 1, "kind": "check",
                "description": "data loss reproduced",
                "expected": "ok", "actual": "lost", "verdict": "fail"}"#,
            )
            .unwrap(),
        ];
        assert_eq!(
            statements(&ranked(&case))[0],
            "rebuild stalled",
            "falsified worst-case must still rank below non-falsified"
        );

        // Worst-adjacent (1 source) vs max-source non-worst (2 sources, 2 kinds).
        let case = case_with(
            "vd5 rebuild",
            Some(OPEN_TRIAGE),
            vec![
                hyp(
                    "rebuild stalled",
                    "vd5 rebuild completes",
                    &["test:sel://events", "actual:write-cache"],
                ),
                hyp("data loss on vd5", "vd5 data lost", &["test:sel://events"]),
            ],
        );
        assert_eq!(
            statements(&ranked(&case))[0],
            "data loss on vd5",
            "worst-case must outrank max-source non-worst"
        );
    }

    /// Generic-only overlap must NOT boost or demote: a hypothesis
    /// sharing only the ticket topic word (`vd5`) is neither
    /// worst-adjacent nor falsified (the S1 red-proof — both arms
    /// fired for every hypothesis via the shared topic word).
    #[test]
    fn generic_only_overlap_neither_boosts_nor_demotes() {
        // Worst: `vd5` is in the worst case AND the ticket — a hyp
        // mentioning only `vd5` must not lift over a tie-break winner.
        let case = case_with(
            "vd5 rebuild",
            Some(OPEN_TRIAGE),
            vec![
                hyp("vd5 generic", "vd5 generic outcome", &["test:sel://events"]),
                hyp(
                    "alpha specific",
                    "vd5 alpha completes",
                    &["test:sel://events"],
                ),
            ],
        );
        assert_eq!(
            statements(&ranked(&case)),
            vec!["alpha specific", "vd5 generic"],
            "generic-only worst overlap must not lift (tie-break orders alpha first)"
        );

        // Falsified: a Fail row mentioning only `vd5` demotes nothing.
        let mut case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp("vd5 generic", "vd5 generic outcome", &["test:sel://events"]),
                hyp(
                    "alpha specific",
                    "vd5 alpha completes",
                    &["test:sel://events"],
                ),
            ],
        );
        // Row text carries only the ticket topic word (plus words that
        // are in neither hypothesis's specific vocabulary) — a
        // generic-only Fail row must demote nothing.
        case.test_log = vec![
            serde_json::from_str(
                r#"{"order": 1, "kind": "check",
                "description": "vd5 check recorded",
                "expected": "ready", "actual": "recorded", "verdict": "fail"}"#,
            )
            .unwrap(),
        ];
        assert_eq!(
            statements(&ranked(&case)),
            vec!["alpha specific", "vd5 generic"],
            "generic-only Fail row must not demote"
        );
    }

    /// R-4: total tie-break — equal scores order by statement, then
    /// prediction, then original index (identical texts deterministic).
    #[test]
    fn r4_tie_break_is_a_total_order() {
        let case = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp("beta", "vd5 completes", &["test:sel://events"]),
                hyp("alpha", "vd5 completes", &["test:sel://events"]),
            ],
        );
        assert_eq!(statements(&ranked(&case)), vec!["alpha", "beta"]);

        // Identical statement + prediction: original index decides, twice.
        let dup = case_with(
            "vd5 rebuild",
            None,
            vec![
                hyp("same", "vd5 completes", &["test:sel://events"]),
                hyp("same", "vd5 completes", &["test:sel://events"]),
            ],
        );
        assert_eq!(ranked(&dup), ranked(&dup));
        assert_eq!(ranked(&dup).len(), 2);
    }

    /// R-6: needles honor the floor and the direction law.
    #[test]
    fn r6_needles_honor_floor_and_direction() {
        let found = needles("a vd5 rebuild on node-042");
        assert!(found.contains(&"vd5".to_string()), "floor-exact kept");
        assert!(!found.contains(&"a".to_string()), "1-char dropped");
        assert!(!found.contains(&"on".to_string()), "2-char dropped");
        // Direction: the SHORT needle sits inside the LONGER text.
        let hay = "rebuild started on vd5".to_lowercase();
        assert!(hay.contains("rebuild"));
        assert!(!"reb".contains("rebuild"), "evidence never fits the needle");
        // Empty/worst-less inputs are safe.
        assert!(needles("").is_empty());
        assert!(needles("a b c").is_empty());
    }

    /// Empty input ranks to empty; a single hypothesis is trivially first.
    #[test]
    fn empty_and_single_rank_trivially() {
        assert!(ranked(&GdlCase::fresh("vd5 rebuild")).is_empty());
        let one = case_with(
            "vd5 rebuild",
            None,
            vec![hyp("only", "vd5 completes", &["test:sel://events"])],
        );
        assert_eq!(statements(&ranked(&one)), vec!["only"]);
    }
}
