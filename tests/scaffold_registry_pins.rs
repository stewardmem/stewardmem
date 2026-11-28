//! Scaffold-registry pins (SI0 S1) — written RED before the crate exists:
//! the behavioral class fails to compile (E0433, the crate unresolved), the
//! source class fails by file-absence (the cascade_source_pins.rs runtime
//! convention). The set greens with `crates/brain-scaffold-core` in the same
//! step; S3 and S4 append their pins here as they land, red on arrival.
//!
//! The laws the S1 set carries (plan §3.1):
//! - the verdict is a pure function of the paired counts — no connection, no
//!   Result, no re-execution;
//! - the vocabulary is closed: an unknown kind, and every kind not validated
//!   this round, are refused `scaffold_kind_unvalidated`;
//! - fewer than ten changed entries never passes, whatever the tail says;
//! - the sign test is the exact one-sided binomial tail against 0.05/k — the
//!   02 §7 table is pinned row by row, and `k` is an argument;
//! - a guard that loses more entries than it gains is fail; a tie is not;
//! - a body over the complexity cap is refused;
//! - the crate holds zero DDL and never opens a transaction — source-pinned;
//! - every rule constant is pinned by value, so none moves without a pin
//!   moving with it.

use std::path::Path;

use brain_scaffold_core::rule::{
    ALPHA_DENOMINATOR, ALPHA_NUMERATOR, GUARDS, GuardCounts, MIN_CHANGED_ENTRIES, PRIMARY_METRIC,
    Pair, VALIDATED_KINDS, VerdictInput, decide_verdict,
};
use brain_scaffold_core::validate::{DIFF_MAX_LINES, validate_body};
use brain_scaffold_core::vocab::{Refusal, ScaffoldKind, Verdict};

const CRATE_SRC: &str = "crates/brain-scaffold-core/src";

fn crate_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(CRATE_SRC);
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("crate sources {dir:?}: {e}"));
    for entry in entries {
        let path = entry.expect("crate src entry").path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let src =
                std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
            out.push((path.display().to_string(), src));
        }
    }
    out.sort();
    assert!(!out.is_empty(), "the crate must have sources to pin");
    out
}

fn clean_guards() -> GuardCounts {
    GuardCounts {
        recall_at_1: Pair {
            improved: 0,
            worsened: 0,
        },
        recall_at_5: Pair {
            improved: 0,
            worsened: 0,
        },
        recall_at_10_with_identifier: Pair {
            improved: 0,
            worsened: 0,
        },
        recall_at_10_without: Pair {
            improved: 0,
            worsened: 0,
        },
    }
}

fn verdict_input(
    improved: u32,
    worsened: u32,
    candidates_tried: u32,
    guards: GuardCounts,
) -> VerdictInput {
    VerdictInput {
        improved,
        worsened,
        candidates_tried,
        guards,
    }
}

#[test]
fn decide_verdict_is_pure_and_infallible() {
    let rule = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/brain-scaffold-core/src/rule.rs"),
    )
    .unwrap_or_else(|e| panic!("rule.rs: {e}"));
    assert!(
        rule.contains("pub fn decide_verdict(input: &VerdictInput) -> Verdict {"),
        "decide_verdict's signature must stay exactly (&VerdictInput) -> Verdict — no connection, no Result, no mut"
    );
}

#[test]
fn an_unknown_kind_is_refused() {
    let refusal = ScaffoldKind::try_from("repo_config").unwrap_err();
    assert_eq!(refusal, Refusal::ScaffoldKindUnvalidated);
    assert_eq!(refusal.to_string(), "scaffold_kind_unvalidated");
}

#[test]
fn a_kind_not_validated_this_round_is_refused() {
    for kind in [
        ScaffoldKind::Prompt,
        ScaffoldKind::Frame,
        ScaffoldKind::ToolPolicy,
        ScaffoldKind::Routing,
        ScaffoldKind::DiffFormat,
    ] {
        assert_eq!(
            validate_body(kind, "an otherwise ordinary body"),
            Err(Refusal::ScaffoldKindUnvalidated),
            "{} is not validated in SI0 (A-1: retrieval only)",
            kind.as_str(),
        );
    }
    assert_eq!(validate_body(ScaffoldKind::Retrieval, "p"), Ok(()));
}

#[test]
fn gain_below_minimum_is_inconclusive() {
    // (9, 0) matters: its tail is inside the nine-candidate level, so this
    // pin proves the FLOOR refuses, not the tail.
    for (improved, worsened) in [(9u32, 0u32), (8, 1), (5, 4), (1, 8), (0, 9)] {
        assert_eq!(
            decide_verdict(&verdict_input(improved, worsened, 9, clean_guards())),
            Verdict::Inconclusive,
            "{improved} of {} changed entries is below the floor of ten",
            improved + worsened,
        );
    }
}

#[test]
fn gain_inside_selection_noise_is_inconclusive() {
    // (9, 1): ten changed entries, tail = 11/1024 ≈ 0.0107 — inside k=1's
    // 0.05, outside k=9's 0.0056. The ONLY input that moves is k.
    assert_eq!(
        decide_verdict(&verdict_input(9, 1, 1, clean_guards())),
        Verdict::Pass
    );
    assert_eq!(
        decide_verdict(&verdict_input(9, 1, 9, clean_guards())),
        Verdict::Inconclusive
    );
}

#[test]
fn the_sign_test_is_exact_and_pinned() {
    // The 02 §7 table, row α = 0.0056 (nine candidates): the minimum b that
    // passes, per m = b + c. The verdict must pass AT the table's b and stay
    // inconclusive one below it — the boundary is exact.
    const TABLE: [(u32, u32); 6] = [(10, 10), (20, 17), (30, 23), (40, 29), (50, 35), (100, 64)];
    for (m, min_improved) in TABLE {
        assert_eq!(
            decide_verdict(&verdict_input(
                min_improved,
                m - min_improved,
                9,
                clean_guards()
            )),
            Verdict::Pass,
            "b={min_improved} of m={m} must pass at k=9",
        );
        assert_eq!(
            decide_verdict(&verdict_input(
                min_improved - 1,
                m - min_improved + 1,
                9,
                clean_guards()
            )),
            Verdict::Inconclusive,
            "b={}-1 of m={m} must stay inconclusive at k=9",
            min_improved,
        );
    }
}

#[test]
fn regression_beyond_margin_is_fail() {
    // The primary passes clean at (10, 0, k=9) — the guard is what refuses.
    let regressed = GuardCounts {
        recall_at_1: Pair {
            improved: 0,
            worsened: 2,
        },
        ..clean_guards()
    };
    assert_eq!(
        decide_verdict(&verdict_input(10, 0, 9, regressed)),
        Verdict::Fail
    );
    // A tie is not a regression: equal counts stay non-inferior.
    let tied = GuardCounts {
        recall_at_1: Pair {
            improved: 1,
            worsened: 1,
        },
        ..clean_guards()
    };
    assert_eq!(
        decide_verdict(&verdict_input(10, 0, 9, tied)),
        Verdict::Pass
    );
}

#[test]
fn diff_over_cap_is_refused() {
    let over = "x\n".repeat(DIFF_MAX_LINES + 1);
    let refusal = validate_body(ScaffoldKind::Retrieval, &over).unwrap_err();
    assert_eq!(refusal, Refusal::DiffOverComplexityCap);
    assert_eq!(refusal.to_string(), "diff_over_complexity_cap");
    // At the cap exactly, the body passes: the cap is a ceiling, not a floor.
    let at_cap = "x\n".repeat(DIFF_MAX_LINES);
    assert_eq!(validate_body(ScaffoldKind::Retrieval, &at_cap), Ok(()));
}

#[test]
fn the_crate_holds_no_ddl() {
    for (path, src) in crate_sources() {
        let lowered = src.to_lowercase();
        assert!(
            !lowered.contains("create table"),
            "{path} must hold no DDL — the tables belong to src/migration.rs"
        );
        assert!(
            !src.contains("execute_batch"),
            "{path} must not run batch SQL"
        );
    }
}

#[test]
fn the_crate_never_opens_a_transaction() {
    for (path, src) in crate_sources() {
        assert!(
            !src.contains("BEGIN"),
            "{path} must not open its own transaction — the service core owns the boundaries"
        );
        assert!(
            !src.contains("Transaction"),
            "{path} must not take or build a rusqlite transaction"
        );
    }
}

#[test]
fn rule_constants_are_not_editable_without_a_pin() {
    assert_eq!(PRIMARY_METRIC, "file_level_recall_at_10_fused");
    assert_eq!(ALPHA_NUMERATOR, 1);
    assert_eq!(ALPHA_DENOMINATOR, 20);
    assert_eq!(MIN_CHANGED_ENTRIES, 10);
    assert_eq!(DIFF_MAX_LINES, 200);
    assert_eq!(
        GUARDS,
        [
            "recall_at_1",
            "recall_at_5",
            "recall_at_10_with_identifier",
            "recall_at_10_without"
        ]
    );
    assert_eq!(VALIDATED_KINDS, &[ScaffoldKind::Retrieval]);
}
