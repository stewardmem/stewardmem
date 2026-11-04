//! The evidence-quality grade over a GDL case's committed evidence set
//! (SPEC_ADDENDUM_GDL_DIAGNOSTIC_GAPS Gap 3: `CertaintyGrade`).
//!
//! ADVISORY BY CONSTRUCTION, per the house law `Hypothesis.confidence`
//! already states (gdl.rs): computed once at artifact application, stored
//! on the chain, read by the wire and the eval — no gate reads it, no
//! threshold binds it, and no routing consumes it until a lane names that
//! consumer (CG-03's ranker is the first). The model proposes evidence;
//! this grade is index arithmetic and pure functions over typed
//! artifacts — deterministic, `Eq`-able, never a float.
//!
//! The four axes (SPEC_ADDENDUM:34, each with its grounded input set):
//! bias (risk-of-bias flags over citation distinctness, the red-flag
//! rule-out posture, and off-playbook rows), directness (a ticket-derived
//! locator resolving into held evidence, reusing the citation floor and
//! direction discipline verbatim), imprecision (row counts — the honest
//! axis on string evidence), and consistency (the verify bar vs the test
//! rows). `overall` is a closed enum evaluated top-down; it is an
//! opinion ABOUT evidence, never an input to any gate.
//!
//! NOT this: [`super::confidence`] is the model-confidence → human-deferral
//! CLASS policy — model self-report, a different question over a different
//! input with a different consumer. Both share the house shape: pure,
//! conservative until measured.

use serde::{Deserialize, Serialize};

use super::gdl::{GdlCase, MIN_CITATION_LOCATOR_CHARS, RuledOutState, Verdict, ruled_out_state};

/// The evidence-quality grade over a case's committed evidence set
/// (SPEC_ADDENDUM Gap 3). ADVISORY BY CONSTRUCTION, per the house law
/// `Hypothesis.confidence` already states: computed once at artifact
/// application, stored on the chain, read by the wire and the eval —
/// no gate reads it, no threshold binds it, and no routing consumes it
/// until a lane names that consumer (CG-03's ranker is the first).
/// The model proposes evidence; this grade is index arithmetic and pure
/// functions over typed artifacts — deterministic, Eq-able, never a float.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CertaintyGrade {
    /// Closed vocabulary, declaration order (stable serialization).
    pub bias: Vec<BiasFlag>,
    pub directness: Directness,
    pub imprecision: Imprecision,
    pub consistency: Consistency,
    pub overall: Overall,
}

/// Risk-of-bias flags, each independent, accumulated in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BiasFlag {
    /// Fewer than 2 distinct evidence citations across the case
    /// (A4's triangulation floor, read over committed state).
    SingleSource,
    /// The worst case is not ruled out (`ruled_out ≠ Yes`) — absent or
    /// malformed is OPEN, the fail-closed law `RuledOutState` holds.
    UnruledWorstCase,
    /// An executed row with no playbook citation — the row the Act gate
    /// already calls "experimental (flagged, not silent)" (gdl.rs L9).
    ExperimentalRow,
}

/// Is the evidence on-questions? A ticket-derived locator resolving into
/// held evidence says Direct (the citation floor + direction discipline,
/// [`super::gdl::locator_resolves`] verbatim); nothing resolving is
/// Indirect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Directness {
    Direct,
    Indirect,
}

/// Imprecision as row counts. "Interval width" has no machine-checkable
/// reading on string evidence (the applied-bar lesson, `VerifyArtifact.
/// pass_condition`): the count axis is the honest one; widths are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Imprecision {
    /// Executed test-log rows.
    pub n_rows: usize,
    /// Rows that measured something (`actual` recorded).
    pub n_measured: usize,
}

/// Agreement between the independent kinds: the verify bar vs the executed
/// rows. No verify artifact yet means the kinds cannot be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Consistency {
    Agree,
    Disagree,
    NotYetVerifiable,
}

/// The overall grade. An enum, never a float (series law 3): no threshold
/// can be tuned against it and no gate may read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Overall {
    High,
    Moderate,
    Low,
}

/// The module's single public fn: grade the case from committed state
/// alone (the `citation_universe` discipline — nothing from the current
/// pass's raw model bytes). Called only from `gdl::apply`, the one point
/// where committed evidence changes.
pub(crate) fn for_case(case: &GdlCase) -> CertaintyGrade {
    let mut bias = Vec::new();
    if distinct_citations(case) < 2 {
        bias.push(BiasFlag::SingleSource);
    }
    let ruled_out = case
        .triage
        .as_ref()
        .and_then(|t| ruled_out_state(&t.red_flag.ruled_out));
    if ruled_out != Some(RuledOutState::Yes) {
        // Absent/malformed parses as None above — OPEN, fail-closed.
        bias.push(BiasFlag::UnruledWorstCase);
    }
    if case.test_log.iter().any(|r| r.playbook_ref.is_none()) {
        bias.push(BiasFlag::ExperimentalRow);
    }

    let needles = ticket_locators(case);
    let directness = if is_direct(case, &needles) {
        Directness::Direct
    } else {
        Directness::Indirect
    };

    let imprecision = Imprecision {
        n_rows: case.test_log.len(),
        n_measured: case.test_log.iter().filter(|r| r.actual.is_some()).count(),
    };

    let consistency = match &case.verify {
        None => Consistency::NotYetVerifiable,
        Some(v) => {
            let rows_agree = case.test_log.iter().all(|r| r.verdict == Verdict::Pass);
            if v.pass && rows_agree {
                Consistency::Agree
            } else {
                Consistency::Disagree
            }
        }
    };

    // The documented overall table, top-down, first hit wins:
    // 1. an open worst case OR nothing measured → Low;
    // 2. any other bias flag, OR fewer than two measured rows, OR the
    //    verify bar vs the rows disagreeing, OR indirect evidence →
    //    Moderate;
    // 3. otherwise (clean bias, ≥2 measured rows, Direct, Agree) → High.
    let overall = if bias.contains(&BiasFlag::UnruledWorstCase) || imprecision.n_measured == 0 {
        Overall::Low
    } else if !bias.is_empty()
        || imprecision.n_measured < 2
        || consistency == Consistency::Disagree
        || directness == Directness::Indirect
    {
        Overall::Moderate
    } else {
        Overall::High
    };

    CertaintyGrade {
        bias,
        directness,
        imprecision,
        consistency,
        overall,
    }
}

/// Distinct non-empty citations across the case's committed evidence:
/// the triage's `confirmed_matches`, the red-flag's `rule_out_basis`,
/// per-row `evidence_ref`s, and per-hypothesis `sources`. Distinctness is
/// case-insensitive on trimmed bytes (the locator discipline lowercases
/// for resolution; fewer distinct grades lower — conservative).
fn distinct_citations(case: &GdlCase) -> usize {
    let mut seen: Vec<String> = Vec::new();
    let note = |s: &str, seen: &mut Vec<String>| {
        let key = s.trim().to_lowercase();
        if !key.is_empty() && !seen.contains(&key) {
            seen.push(key);
        }
    };
    if let Some(t) = &case.triage {
        if let Some(matches) = &t.confirmed_matches {
            for m in matches {
                note(m, &mut seen);
            }
        }
        for b in &t.red_flag.rule_out_basis {
            note(b, &mut seen);
        }
    }
    for row in &case.test_log {
        if let Some(e) = &row.evidence_ref {
            note(e, &mut seen);
        }
    }
    for h in &case.hypotheses {
        for s in &h.sources {
            note(s, &mut seen);
        }
    }
    seen.len()
}

/// Ticket-derived locators: the lowercase ticket + the intake is_not rows
/// (the `citation_universe` head, gdl.rs), split into whitespace tokens
/// with punctuation edges trimmed, kept at the citation floor or longer,
/// deduped in order. Extraction arithmetic, not model judgment.
fn ticket_locators(case: &GdlCase) -> Vec<String> {
    let mut text = case.ticket.clone();
    if let Some(intake) = &case.intake {
        for row in [
            &intake.is_not.what,
            &intake.is_not.place,
            &intake.is_not.time,
            &intake.is_not.extent,
        ] {
            text.push(' ');
            text.push_str(&row.is);
        }
    }
    let mut out: Vec<String> = Vec::new();
    for token in text.to_lowercase().split_whitespace() {
        let token = token.trim_matches(|c: char| !c.is_alphanumeric());
        if token.chars().count() >= MIN_CITATION_LOCATOR_CHARS && !out.iter().any(|s| s == token) {
            out.push(token.to_string());
        }
    }
    out
}

/// Direct iff at least one needle resolves into held evidence:
/// case-insensitive containment of the SHORT needle inside the LONGER
/// evidence string (`locator_resolves`'s direction law) over the test
/// rows' descriptions and recorded actuals plus the triage's
/// `confirmed_matches` titles.
fn is_direct(case: &GdlCase, needles: &[String]) -> bool {
    let mut evidence: Vec<String> = Vec::new();
    for row in &case.test_log {
        evidence.push(row.description.to_lowercase());
        if let Some(actual) = &row.actual {
            evidence.push(actual.to_lowercase());
        }
    }
    if let Some(t) = &case.triage
        && let Some(matches) = &t.confirmed_matches
    {
        for m in matches {
            evidence.push(m.to_lowercase());
        }
    }
    needles
        .iter()
        .any(|n| evidence.iter().any(|e| e.contains(n.as_str())))
}

#[allow(clippy::derivable_impls)]
impl Default for CertaintyGrade {
    fn default() -> Self {
        CertaintyGrade {
            bias: Vec::new(),
            directness: Directness::Direct,
            imprecision: Imprecision {
                n_rows: 0,
                n_measured: 0,
            },
            consistency: Consistency::Agree,
            overall: Overall::High,
        } // CG-01 DAMAGE: a synthesized grade
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── deterministic fixtures ────────────────────────────────────────────

    /// The clean High case: ruled-out worst case, three distinct citations,
    /// both rows on-playbook and measured, a ticket needle inside a
    /// description, verify agreeing with the rows.
    fn high_case() -> GdlCase {
        let mut case = GdlCase::fresh("vd5 stuck rebuild on node-042");
        case.triage = Some(
            serde_json::from_str(
                r#"{
                "priority": "P3",
                "verdict": "accept",
                "confirmed_matches": ["kb:1 raid rebuild playbook"],
                "red_flag": {
                    "worst_case": "data loss on vd5",
                    "ruled_out": true,
                    "rule_out_basis": ["verify:sel://events clean"]
                }
            }"#,
            )
            .unwrap(),
        );
        case.hypotheses = vec![
            serde_json::from_str(
                r#"{
                "statement": "vd5 rebuild is stalled",
                "prediction": "rebuild completes after the kickoff",
                "sources": ["log:sel://events"]
            }"#,
            )
            .unwrap(),
        ];
        case.test_log = vec![
            serde_json::from_str(
                r#"{
                    "order": 1,
                    "kind": "check",
                    "description": "vd5 rebuild progress check on node-042",
                    "playbook_ref": "1",
                    "expected": "rebuild completes",
                    "actual": "rebuild progressed 12%",
                    "verdict": "pass",
                    "evidence_ref": "log:sel://events"
                }"#,
            )
            .unwrap(),
            serde_json::from_str(
                r#"{
                    "order": 2,
                    "kind": "action",
                    "description": "issued rebuild kickoff for vd5",
                    "playbook_ref": "2",
                    "expected": "kickoff accepted",
                    "actual": "kickoff accepted",
                    "verdict": "pass"
                }"#,
            )
            .unwrap(),
        ];
        case.verify = Some(
            serde_json::from_str(
                r#"{
                "re_run": "vd5 rebuild progress",
                "pass_condition": "rebuild completes",
                "pass": true
            }"#,
            )
            .unwrap(),
        );
        case
    }

    /// The xorshift64* generator behind P-1's 100-fixture sweep —
    /// dependency-free and seed-recorded. `below` mixes the HIGH bits
    /// (xorshift's low bits are near-constant while the state is small —
    /// the bias P-1's damage-red exposed: the first sweep silently ran
    /// one shape 100 times).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: u64) -> u64 {
            ((self.next() >> 11) * n) >> 53
        }
    }

    /// A generated case: every axis field drawn from the seed, so the
    /// sweep covers arm combinations, not one shape 100 times.
    fn generated_case(seed: u64) -> GdlCase {
        let mut rng = Rng(seed | 1);
        let mut case = GdlCase::fresh("vd5 stuck rebuild on node-042");
        if rng.below(2) == 0 {
            let ruled_out = ["true", "false", "\"partial\"", "\"garbage\""][rng.below(4) as usize];
            let matches = if rng.below(2) == 0 {
                r#""confirmed_matches": ["kb:1 raid rebuild playbook"],"#.to_string()
            } else {
                String::new()
            };
            let basis = if rng.below(2) == 0 {
                r#""rule_out_basis": ["verify:sel://events clean"],"#.to_string()
            } else {
                String::new()
            };
            case.triage = Some(
                serde_json::from_str(&format!(
                    r#"{{ {matches} {basis} "verdict": "accept",
                         "red_flag": {{"worst_case": "data loss", "ruled_out": {ruled_out}}} }}"#,
                ))
                .unwrap(),
            );
        }
        if rng.below(2) == 0 {
            let sources = if rng.below(2) == 0 {
                r#""sources": ["log:sel://events"],"#.to_string()
            } else {
                String::new()
            };
            case.hypotheses = vec![
                serde_json::from_str(&format!(
                    r#"{{ "statement": "stalled rebuild", "prediction": "completes", {sources}
                     "confidence": 0.7 }}"#,
                ))
                .unwrap(),
            ];
        }
        let n_rows = rng.below(3); // 0..=2 rows
        for i in 0..n_rows {
            let playbook = if rng.below(3) == 0 {
                String::new() // None → experimental row
            } else {
                format!(r#""playbook_ref": "{i}", "#)
            };
            let actual = if rng.below(2) == 0 {
                r#""actual": "rebuild progressed", "#.to_string()
            } else {
                String::new()
            };
            let verdict = ["pass", "fail", "pending"][rng.below(3) as usize];
            let evidence = if rng.below(2) == 0 {
                r#""evidence_ref": "log:sel://events", "#.to_string()
            } else {
                String::new()
            };
            case.test_log.push(
                serde_json::from_str(&format!(
                    r#"{{ "order": {i}, "kind": "check",
                          "description": "vd5 rebuild progress check on node-042",
                          {playbook}{actual}{evidence}"expected": "progress", "verdict": "{verdict}" }}"#,
                ))
                .unwrap(),
            );
        }
        if rng.below(2) == 0 {
            let pass = if rng.below(2) == 0 { "true" } else { "false" };
            case.verify = Some(
                serde_json::from_str(&format!(
                    r#"{{ "re_run": "vd5 progress", "pass_condition": "progress", "pass": {pass} }}"#,
                ))
                .unwrap(),
            );
        }
        case
    }

    // ── P-1: determinism (same case bytes → same grade) ──────────────────

    /// P-1: 100 seed-recorded generated fixtures, graded twice, Eq both
    /// times — the grade is pure index arithmetic, not a model readout.
    /// Seeds 1..=100, generator `Rng(xorshift64*)` above.
    #[test]
    fn p1_determinism_across_100_recorded_seeds() {
        for seed in 1..=100u64 {
            let case = generated_case(seed);
            let a = for_case(&case);
            let b = for_case(&case);
            assert_eq!(a, b, "seed {seed}: same case bytes must grade equal");
            // The stored-vs-recomputed law: a round-tripped case grades
            // equal too (serde field order is deterministic for these
            // closed vocabularies).
            let bytes = serde_json::to_string(&case).unwrap();
            let back: GdlCase = serde_json::from_str(&bytes).unwrap();
            assert_eq!(for_case(&back), a, "seed {seed}: round-trip grades equal");
        }
    }

    // ── P-2: the six arm flips ────────────────────────────────────────────

    /// P-2 arm 1: one distinct citation flags SingleSource; a SECOND
    /// distinct citation clears it (the A4 floor is ≥2).
    #[test]
    fn p2_single_source_flips_at_two_distinct_citations() {
        let mut one = GdlCase::fresh("vd5 stuck rebuild on node-042");
        one.hypotheses = vec![serde_json::from_str(
            r#"{"statement": "stalled", "prediction": "completes", "sources": ["log:sel://events"]}"#,
        )
        .unwrap()];
        one.triage = Some(
            serde_json::from_str(
                r#"{"verdict": "accept", "red_flag": {"worst_case": "d", "ruled_out": true}}"#,
            )
            .unwrap(),
        );
        assert!(
            for_case(&one).bias.contains(&BiasFlag::SingleSource),
            "one distinct citation must flag SingleSource"
        );
        let mut two = one.clone();
        two.triage.as_mut().unwrap().confirmed_matches =
            Some(vec!["kb:1 raid rebuild playbook".into()]);
        assert!(
            !for_case(&two).bias.contains(&BiasFlag::SingleSource),
            "two distinct citations must clear SingleSource"
        );
    }

    /// P-2 arm 2: `ruled_out != Yes` flags UnruledWorstCase (absent and
    /// malformed fail closed); `true` clears it.
    #[test]
    fn p2_unruled_worst_case_fails_closed_and_clears_on_yes() {
        let mut open = GdlCase::fresh("vd5 stuck rebuild");
        open.triage = Some(
            serde_json::from_str(
                r#"{"verdict": "accept", "red_flag": {"worst_case": "d", "ruled_out": false}}"#,
            )
            .unwrap(),
        );
        assert!(for_case(&open).bias.contains(&BiasFlag::UnruledWorstCase));
        // Malformed → None → OPEN.
        open.triage.as_mut().unwrap().red_flag.ruled_out = serde_json::json!("banana");
        assert!(
            for_case(&open).bias.contains(&BiasFlag::UnruledWorstCase),
            "malformed ruled_out is OPEN, fail-closed"
        );
        // Absent triage → no red_flag at all → OPEN.
        let bare = GdlCase::fresh("vd5 stuck rebuild");
        assert!(for_case(&bare).bias.contains(&BiasFlag::UnruledWorstCase));
        let mut ruled = open;
        ruled.triage.as_mut().unwrap().red_flag.ruled_out = serde_json::json!(true);
        assert!(!for_case(&ruled).bias.contains(&BiasFlag::UnruledWorstCase));
    }

    /// P-2 arm 3: a `playbook_ref: None` row flags ExperimentalRow; the
    /// same row on-playbook clears it.
    #[test]
    fn p2_experimental_row_flips_on_playbook_ref() {
        let mut case = high_case();
        case.test_log[1].playbook_ref = None;
        assert!(for_case(&case).bias.contains(&BiasFlag::ExperimentalRow));
        case.test_log[1].playbook_ref = Some("2".into());
        assert!(!for_case(&case).bias.contains(&BiasFlag::ExperimentalRow));
    }

    /// P-2 arm 4: a ticket needle inside a held description flips
    /// Directness Indirect → Direct.
    #[test]
    fn p2_ticket_needle_in_description_flips_directness() {
        let mut case = high_case();
        for row in &mut case.test_log {
            row.description = "an unrelated observation".into();
            row.actual = Some("no change observed".into());
        }
        case.triage.as_mut().unwrap().confirmed_matches = None;
        assert_eq!(for_case(&case).directness, Directness::Indirect);
        case.test_log[0].description = "vd5 rebuild progress check".into();
        assert_eq!(for_case(&case).directness, Directness::Direct);
    }

    /// P-2 arm 5: the second `actual.is_some()` row flips imprecision
    /// 1 → 2.
    #[test]
    fn p2_second_measured_row_flips_imprecision() {
        let mut case = high_case();
        case.test_log[1].actual = None;
        let g = for_case(&case);
        assert_eq!(g.imprecision.n_measured, 1);
        assert_eq!(g.imprecision.n_rows, 2);
        case.test_log[1].actual = Some("kickoff accepted".into());
        let g = for_case(&case);
        assert_eq!(g.imprecision.n_measured, 2);
    }

    /// P-2 arm 6: verify.pass vs the rows flips Consistency.
    #[test]
    fn p2_verify_vs_rows_flips_consistency() {
        let mut case = high_case();
        case.test_log[1].verdict = Verdict::Fail;
        assert_eq!(for_case(&case).consistency, Consistency::Disagree);
        case.test_log[1].verdict = Verdict::Pass;
        assert_eq!(for_case(&case).consistency, Consistency::Agree);
        case.verify = None;
        assert_eq!(for_case(&case).consistency, Consistency::NotYetVerifiable);
    }

    // ── P-3: the overall table's boundary cases ──────────────────────────

    /// P-3 row 1: an open worst case beats EVERYTHING to Low — even a
    /// case that would otherwise be clean-High.
    #[test]
    fn p3_unruled_worst_case_beats_everything_to_low() {
        let mut case = high_case();
        case.triage.as_mut().unwrap().red_flag.ruled_out = serde_json::json!(false);
        let g = for_case(&case);
        assert!(g.bias.contains(&BiasFlag::UnruledWorstCase));
        assert_eq!(g.imprecision.n_measured, 2);
        assert_eq!(g.directness, Directness::Direct);
        assert_eq!(g.consistency, Consistency::Agree);
        assert_eq!(g.overall, Overall::Low, "row 1 wins over a clean rest");
    }

    /// P-3 row 1: nothing measured → Low.
    #[test]
    fn p3_nothing_measured_is_low() {
        let mut case = high_case();
        for row in &mut case.test_log {
            row.actual = None;
        }
        assert_eq!(for_case(&case).imprecision.n_measured, 0);
        assert_eq!(for_case(&case).overall, Overall::Low);
    }

    /// P-3 row 2: one measured row with clean bias → Moderate.
    #[test]
    fn p3_one_measured_row_clean_bias_is_moderate() {
        let mut case = high_case();
        case.test_log.truncate(1);
        let g = for_case(&case);
        assert!(g.bias.is_empty(), "clean bias");
        assert_eq!(g.imprecision.n_measured, 1);
        assert_eq!(g.overall, Overall::Moderate);
    }

    /// P-3 row 3: the clean two-row case → High (the table's only High
    /// door: no flags, ≥2 measured, Direct, Agree).
    #[test]
    fn p3_clean_two_row_case_is_high() {
        let g = for_case(&high_case());
        assert!(g.bias.is_empty());
        assert_eq!(g.directness, Directness::Direct);
        assert_eq!(
            g.imprecision,
            Imprecision {
                n_rows: 2,
                n_measured: 2
            }
        );
        assert_eq!(g.consistency, Consistency::Agree);
        assert_eq!(g.overall, Overall::High);
    }

    /// P-3 row 2, the indirect door: clean bias, 2 measured, Agree, but
    /// nothing resolves → Moderate.
    #[test]
    fn p3_indirect_clean_case_is_moderate() {
        let mut case = high_case();
        for row in &mut case.test_log {
            row.description = "an unrelated observation".into();
            row.actual = Some("no change observed".into());
        }
        case.triage.as_mut().unwrap().confirmed_matches = None;
        let g = for_case(&case);
        assert_eq!(g.directness, Directness::Indirect);
        assert_eq!(g.imprecision.n_measured, 2);
        assert!(g.bias.is_empty());
        assert_eq!(g.consistency, Consistency::Agree);
        assert_eq!(g.overall, Overall::Moderate);
    }

    // ── P-4: the extraction helper honors floor + direction ──────────────

    /// P-4 floor: a sub-floor ticket token never becomes a needle, and a
    /// needle never resolves into sub-floor evidence (the
    /// `citation_locator_floor_rejects_substring_fragments` shape — the
    /// grade reuses the citation floor, not a new one).
    #[test]
    fn p4_ticket_locators_honor_the_floor() {
        let case = GdlCase::fresh("a vd5 rebuild on node-042");
        let needles = ticket_locators(&case);
        assert!(
            needles.contains(&"vd5".to_string()),
            "floor-exact token kept"
        );
        assert!(needles.contains(&"rebuild".to_string()));
        assert!(!needles.contains(&"a".to_string()), "1-char token dropped");
        assert!(!needles.contains(&"on".to_string()), "2-char token dropped");
        // The "ts"-in-"tsr://node-042" trap, on the grade's own surface:
        // the ticket token "ts" must never take "tsr://node-042" evidence,
        // and the full locator must.
        let mut trap = GdlCase::fresh("ts tsr://node-042 export");
        trap.test_log = vec![
            serde_json::from_str(
                r#"{"description": "tsr://node-042 export reviewed", "verdict": "pass"}"#,
            )
            .unwrap(),
        ];
        let trap_needles = ticket_locators(&trap);
        assert!(
            !trap_needles.contains(&"ts".to_string()),
            "the 2-char token never becomes a needle"
        );
        assert!(is_direct(&trap, &trap_needles), "the full locator resolves");
        let mut caught = trap.clone();
        caught.ticket = "vd5 only".into();
        assert!(
            !is_direct(&caught, &ticket_locators(&caught)),
            "without a resolving needle the case is Indirect"
        );
    }

    /// P-4 direction: the SHORT needle sits inside the LONGER held string.
    /// A longer ticket word must NOT resolve into shorter evidence — held
    /// evidence is never asked to fit inside a citation.
    #[test]
    fn p4_direction_law_short_needle_inside_longer_evidence() {
        let mut case = GdlCase::fresh("rebuild stalled");
        case.test_log =
            vec![serde_json::from_str(r#"{"description": "reb", "verdict": "pass"}"#).unwrap()];
        // The needle "rebuild" is LONGER than the evidence "reb": the
        // wrong direction would call this a resolution.
        assert!(
            !is_direct(&case, &ticket_locators(&case)),
            "evidence shorter than the needle never resolves (direction law)"
        );
        case.test_log[0].description = "rebuild started on vd5".into();
        assert!(is_direct(&case, &ticket_locators(&case)));
    }

    /// The needle source set is the ticket + intake is_not rows only —
    /// evidence-side text never becomes a needle (no invented inputs).
    #[test]
    fn p4_needles_come_from_ticket_and_is_not_rows_only() {
        let mut case = GdlCase::fresh("vd5 rebuild");
        case.intake = Some(
            serde_json::from_str(
                r#"{"is_not": {"what": {"is": "write cache", "is_not": "read cache"},
                              "where": {"is": "node-042", "is_not": "node-041"},
                              "when": {"is": "since rebuild", "is_not": "before"},
                              "extent": {"is": "vd5 only", "is_not": "all"}},
                "telemetry_refs": ["tsr://node-042"]}"#,
            )
            .unwrap(),
        );
        let needles = ticket_locators(&case);
        assert!(
            needles.contains(&"write".to_string()),
            "is_not row 'is' kept"
        );
        assert!(needles.contains(&"node-042".to_string()));
        assert!(
            !needles.iter().any(|n| n.contains("tsr://")),
            "telemetry_refs are NOT in the needle set (ticket + is_not only)"
        );
    }
}
