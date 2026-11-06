//! CG-05 — the multi-issue intake's structural pins (S-1..S-3, W-1, W-2).
//!
//! **The invariants.** A gate-passed decomposition splits in exactly
//! one place, by exactly one act, reading nothing the model can soften:
//!
//! - **S-1 · one split site.** The exactly-once guard (`children`
//!   empty), the parent-side link write (`next.case.children = ids`),
//!   the sibling creation (`state::open_run`) and the issue slicing
//!   (`.skip(1)` — the parent IS issue-1's case) all live in exactly
//!   one function: `advance_tx` in `gdl_checkpoint.rs`. No second
//!   split site exists, wherever the census looks in production code.
//! - **S-2 · no sibling creation outside the satellite.** `apply`
//!   (the one point committed evidence changes) never inserts a run:
//!   the sibling's `open_run` call lives in `advance_tx` alone, never
//!   on a refusal, never in `apply` — the exact-once guard makes the
//!   act idempotent rather than a second implementation.
//! - **S-3 · no-confidence / no-grade in the arms.** The decomposition
//!   gate arms read no `confidence`, no `certainty`, no
//!   `CertaintyGrade`, no float, no `rank` — index arithmetic and
//!   byte counts only; the `Certainty`/`ranking` scores are untouched
//!   by the lane (CG-02's trust tier and CG-03's weights read where
//!   the child differential runs them, never re-graded here).
//! - **W-1 · byte stories.** The case carries the split-pending marker
//!   with `skip_serializing_if = "Option::is_none"` (a no-split
//!   persisted case serializes NO decomposition key — behavioral
//!   witness in `gdl.rs`'s tests module; the `children`/`sibling_ids`
//!   are additive defaults, old states parse to `[]`, and
//!   `parent_case_id` parses to `None`). The synthetic-default damage
//!   (dropping the skip) reds the in-file pin.
//! - **W-2 · zero wire surface.** `openapi.yaml` gains no
//!   `decomposition`/`parent_case_id` line — the bytes ride
//!   `state_json` (the regen-empty proof is verify-after §4.5).
//!
//! Matching runs against comment-stripped code (`common::code_only`) —
//! a pin that greps raw text passes on a comment that merely names the
//! symbol it forbids (the false-pass CG-01 recorded).
//!
//! NOTE — W-1's behavioral half lives in `gdl.rs`'s tests module (an
//! integration test cannot name the `pub(crate)` `GdlCase`,
//! `IntakeArtifact`, `ProposedIssue` or `Decomposition`); the
//! structural halves — the serde attributes and the wire census —
//! are pinned here.

mod common;

use std::path::{Path, PathBuf};

fn code_only(src: &str) -> String {
    common::code_only(src)
}

fn repo(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read(rel: &str) -> String {
    let p = repo(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} must exist: {e}", p.display()))
}

/// Every `.rs` file under `src/`, recursively (std-only walk).
fn src_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![repo("src")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src dir readable") {
            let p = entry.expect("dir entry readable").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out
}

/// The production region of a module file: comment-stripped code, cut at
/// `mod tests {` (the CG-01 precedent).
fn prod_code(rel: &str) -> String {
    let code = code_only(&read(rel));
    code.split("\nmod tests {")
        .next()
        .expect("a tests module")
        .to_string()
}

/// The apply body's region inside gdl.rs production code.
fn apply_region() -> String {
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl.find("pub(crate) fn apply").expect("apply must exist");
    let end = gdl[start..]
        .find("pub(crate) fn hypothesis_status")
        .expect("hypothesis_status must follow apply");
    gdl[start..start + end].to_string()
}

/// The Intake-advance satellite's region inside gdl_checkpoint.rs
/// production code: `advance_tx`, the one function that splits.
fn split_act_region() -> String {
    let checkpoint = prod_code("src/workflow/gdl_checkpoint.rs");
    let start = checkpoint
        .find("fn advance_tx")
        .expect("advance_tx must exist");
    checkpoint[start..].to_string()
}

/// S-1: the split act lives in exactly one function — `advance_tx` in
/// `gdl_checkpoint.rs`. The four distinctive tokens of the act each
/// occur exactly once in production code; no second split site exists.
///
/// Proof by damage: plant a second `state::open_run` sibling creation
/// anywhere else (or a second `children = ids` write) and this reds.
#[test]
fn s1_one_split_site() {
    let checkpoint = prod_code("src/workflow/gdl_checkpoint.rs");
    for token in [
        "next.case.children.is_empty()",
        "next.case.children = ids",
        "state::open_run(",
        ".skip(1)",
    ] {
        assert_eq!(
            checkpoint.matches(token).count(),
            1,
            "the split act's `{token}` occurs exactly once (advance_tx)"
        );
    }
    // No second split site anywhere else in production code: the
    // parent-side link write exists in exactly one module.
    let mut sites: Vec<(String, usize)> = Vec::new();
    for p in src_files() {
        let rel = p.strip_prefix(repo(".")).expect("relative path");
        let code = prod_code(rel.to_str().expect("utf8 path"));
        let n = code.matches("next.case.children = ids").count();
        if n > 0 {
            sites.push((rel.to_string_lossy().into_owned(), n));
        }
    }
    assert_eq!(
        sites,
        vec![("src/workflow/gdl_checkpoint.rs".to_string(), 1usize)],
        "the parent-side link write exists in exactly one module"
    );
    // And inside advance_tx, the act's order holds: guard before the
    // parent-side write (the exactly-once discipline).
    let act = split_act_region();
    let guard = act
        .find("next.case.children.is_empty()")
        .expect("the exactly-once guard");
    let write = act
        .find("next.case.children = ids")
        .expect("the link write");
    let create = act.find("state::open_run(").expect("sibling creation");
    assert!(
        guard < create && create < write,
        "guard → creation → link write, inside advance_tx"
    );
}

/// S-2: `apply` never inserts a sibling run. The one point committed
/// evidence changes reads no `open_run`; creation happens only in the
/// satellite (advance_tx), and a gate refusal can never reach it (the
/// act requires the applied `decomposition` on a passing Intake).
///
/// Proof by damage: plant `state::open_run(` in `apply` and this reds.
#[test]
fn s2_no_sibling_creation_in_apply() {
    let apply = apply_region();
    assert!(
        !apply.contains("open_run"),
        "apply must never insert a run — sibling creation is the satellite's alone"
    );
    assert!(
        apply.contains("case.decomposition = a.decomposition.clone()"),
        "apply stores the split-pending marker (case state) — never creates rows"
    );
    // The Intake gate's Intake arm keeps the settle point in
    // parse_and_gate: the refusal path returns before any advance —
    // a refused decomposition costs nothing and inserts nothing.
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl
        .find("pub(crate) fn parse_and_gate")
        .expect("parse_and_gate must exist");
    let end = gdl[start..]
        .find("pub(crate) fn apply")
        .expect("apply must follow parse_and_gate");
    let gate = &gdl[start..start + end];
    assert!(
        !gate.contains("open_run"),
        "the arbiter never inserts a run (the refusal path writes nothing)"
    );
}

/// S-3: no-confidence / no-grade in the decomposition arms. The intake
/// gate reads no `confidence`, no `certainty`, no `CertaintyGrade`,
/// no float, no `rank`; and the certainty/ranking scores are untouched
/// by the lane (the child's differential runs CG-02/CG-03 where it
/// runs, never re-graded here).
///
/// Proof by damage: plant a `case.certainty` read (or a
/// `confidence` comparison) in the intake gate and this reds.
#[test]
fn s3_decomposition_arms_read_no_confidence_or_grade() {
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl.find("fn intake_gate").expect("intake_gate must exist");
    let end = gdl[start..]
        .find("fn triage_gate")
        .expect("triage_gate must follow intake_gate");
    let gate = &gdl[start..start + end];
    for token in ["confidence", "certainty", "CertaintyGrade", "f64", "rank"] {
        assert!(
            !gate.contains(token),
            "the decomposition arms must not read `{token}` (index arithmetic only)"
        );
    }
    // The certainty module is intact: the grade computes where it
    // always computed, never for decomposition.
    let certainty = prod_code("src/workflow/certainty.rs");
    assert!(
        certainty.contains("pub(crate) fn for_case"),
        "certainty::for_case still exists (the CG-01 module is intact)"
    );
    assert!(
        !certainty.contains("decomposition"),
        "the grade never reads a decomposition (advisory, never split-aware)"
    );
    // The ranker's OUTPUT scores are untouched by the lane: the
    // dominance weights still stand at their CG-03 values.
    let ranking = prod_code("src/workflow/ranking.rs");
    for weight in [
        "W_FALSIFIED: i32 = 400",
        "W_WORST: i32 = 200",
        "W_TESTABLE: i32 = 50",
        "W_KIND: i32 = 5",
        "W_SOURCE: i32 = 1",
    ] {
        assert!(
            ranking.contains(weight),
            "ranking weight `{weight}` still stands (scores untouched)"
        );
    }
}

/// W-1 structural half: the split-pending marker skips serialization
/// when absent (a no-split case persists NO decomposition key);
/// `children`/`sibling_ids`/`parent_case_id` are plain serde defaults
/// (old states parse to `[]`/`None`; absence is no data — behavioral
/// half in-file). The wire rides state_json: openapi gains no
/// decomposition line (W-2's file-level half; the regen-empty proof
/// is verify-after §4.5).
#[test]
fn w1_structural_byte_attrs() {
    let gdl = prod_code("src/workflow/gdl.rs");
    // The Intake artifact side: the decomposition skips when absent
    // (single-issue intakes serialize byte-identically — N-1's bytes).
    let intake_start = gdl
        .find("pub(crate) struct IntakeArtifact")
        .expect("IntakeArtifact must exist");
    let intake_end = gdl[intake_start..]
        .find("pub(crate) struct ProposedIssue")
        .expect("ProposedIssue must follow IntakeArtifact");
    let intake_shape = &gdl[intake_start..intake_start + intake_end];
    assert!(
        intake_shape.contains("skip_serializing_if = \"Option::is_none\"")
            && intake_shape.contains("pub decomposition: Option<Decomposition>"),
        "the artifact's decomposition field must skip-when-None (N-1's artifact bytes)"
    );
    // The case state side: the split-pending marker skips when
    // absent; the linkage fields exist with plain serde defaults.
    let case_start = gdl
        .find("pub(crate) struct GdlCase")
        .expect("GdlCase must exist");
    let case_end = gdl[case_start..]
        .find("impl GdlCase")
        .expect("impl GdlCase must follow the struct");
    let shape = &gdl[case_start..case_start + case_end];
    assert!(
        shape.contains("pub decomposition: Option<Decomposition>"),
        "the case's split-pending marker must exist"
    );
    assert!(
        shape.contains("pub children: Vec<i64>"),
        "the parent-side link field must exist with a serde default (old states parse to [])"
    );
    assert!(
        shape.contains("pub parent_case_id: Option<i64>"),
        "the child-side parent link must exist with a serde default (old states parse to None)"
    );
    assert!(
        shape.contains("pub sibling_ids: Vec<i64>"),
        "the child-side sibling set must exist with a serde default (old states parse to [])"
    );
    // The wire rides state_json: openapi gains no decomposition line.
    if let Ok(openapi) = std::fs::read_to_string(repo("openapi-schema/openapi.yaml")) {
        for token in ["decomposition", "parent_case_id", "sibling_ids"] {
            assert!(
                !openapi.contains(token),
                "no wire line for `{token}` — the bytes ride state_json"
            );
        }
    }
}

/// The eval consumer is live: `EvalRow` carries the child count off
/// persisted state with a serde default (old traces parse to None).
#[test]
fn eval_row_carries_the_child_count_off_persisted_state() {
    let eval = prod_code("src/workflow/gdl_eval.rs");
    assert!(
        eval.contains("pub(crate) split_child_count: Option<u32>"),
        "EvalRow.split_child_count must exist"
    );
    assert!(
        eval.contains("final_case.children.len()"),
        "the row reads the child count off the persisted case state"
    );
}
