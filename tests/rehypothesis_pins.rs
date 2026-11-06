//! CG-04 — the bounded re-hypothesis's structural pins (S-1..S-3, W-1).
//!
//! **The invariants.** The back edge (`Verify → Hypothesize`) is
//! admitted in exactly one place, by exactly one rule, reading nothing
//! the model can soften:
//!
//! - **S-1 · one admission site.** `rehypothesis_admits` is evaluated
//!   from exactly one place — the driver intercept in `gdl.rs`
//!   production code (the function itself counts): two matches, no more,
//!   in one file, zero elsewhere.
//! - **S-2 · count only at the edge.** `rehypothesis_count` is
//!   incremented only on the admitted edge (the driver intercept) —
//!   never inside `apply`, never on a refusal. The checkpoint carries it
//!   unchanged; `verify_link` recomputes from the gate row.
//! - **S-3 · no-confidence / no-grade in the arms.** The admission
//!   helper reads no `confidence`, no `certainty`, no `CertaintyGrade`,
//!   no float. The ranker's OUTPUT scores are untouched by the lane
//!   (the falsified-input extension reads `case.rehypothesis`, a CG-04
//!   artifact; the dominance weights stand).
//! - **W-1 · byte stories.** The case struct carries
//!   `skip_serializing_if = "Option::is_none"` on the proposal (linear
//!   cases persist NO proposal key — behavioral witness: the in-file
//!   `cg04_w1_legacy_state_parses_and_bytes_tell_the_story` pin) and
//!   a plain serde default on the count (old states parse to `None`/0).
//!   The synthetic-default damage (C-8 recipe) reds the in-file pin.
//!
//! Matching runs against comment-stripped code (`common::code_only`) —
//! a pin that greps raw text passes on a comment that merely names the
//! symbol it forbids (the false-pass CG-01 recorded).
//!
//! NOTE — W-1's behavioral half lives in `gdl.rs`'s tests module (an
//! integration test cannot name the `pub(crate)` `GdlCase`; the
//! structural half — the serde attributes — is pinned here).

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

/// S-1: `rehypothesis_admits` is evaluated in exactly one function —
/// definition + the single driver-intercept call in `gdl.rs`, nowhere
/// else in production code.
///
/// Proof by damage: plant a second call in any other module's
/// production region and this reds.
#[test]
fn s1_one_admission_site() {
    let mut sites: Vec<(String, usize)> = Vec::new();
    for p in src_files() {
        let rel = p.strip_prefix(repo(".")).expect("relative path");
        let code = prod_code(rel.to_str().expect("utf8 path"));
        let n = code.matches("rehypothesis_admits").count();
        if n > 0 {
            sites.push((rel.to_string_lossy().into_owned(), n));
        }
    }
    assert_eq!(
        sites,
        vec![("src/workflow/gdl.rs".to_string(), 2usize)],
        "rehypothesis_admits: definition + one driver call, both in gdl.rs"
    );
    // The four arms, in order, inside the one function.
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl
        .find("pub(crate) fn rehypothesis_admits")
        .expect("the admission helper must exist");
    let end = gdl[start..]
        .find("fn verify_gate")
        .expect("verify_gate must follow the helper");
    let helper = &gdl[start..start + end];
    for arm in [
        "if red_flag_open",
        "if rehypothesis_count >= max",
        "if !proposal_present",
        "if attempt >= MAX_PHASE_ATTEMPTS",
    ] {
        assert!(
            helper.contains(arm),
            "the four admission arms evaluate, in the helper: `{arm}`"
        );
    }
}

/// S-2: the count is incremented only at the admitted edge. `apply`
/// never writes `rehypothesis_count` (it stores the proposal —
/// presence of the store is asserted, the count-write is asserted
/// absent); the only `+ 1` increment in production gdl.rs is the
/// intercept; the checkpoint carries, never increments.
///
/// Proof by damage: plant `case.rehypothesis_count += 1;` in `apply`
/// and this reds.
#[test]
fn s2_count_incremented_only_at_the_admitted_edge() {
    let apply = apply_region();
    assert!(
        apply.contains("case.rehypothesis = a.rehypothesis.clone()"),
        "apply stores the proposal (the ranker's input) — but never counts"
    );
    assert!(
        !apply.contains("rehypothesis_count"),
        "apply must never write rehypothesis_count — only the admitted edge increments"
    );
    let gdl = prod_code("src/workflow/gdl.rs");
    assert_eq!(
        gdl.matches("rehypothesis_count = cp.case.rehypothesis_count + 1")
            .count(),
        1,
        "exactly one `+ 1` increment site: the driver intercept"
    );
    let checkpoint = prod_code("src/workflow/gdl_checkpoint.rs");
    assert!(
        checkpoint.contains("next.case.rehypothesis_count = change.rehypothesis_count"),
        "the checkpoint carries the transition's count (it never increments)"
    );
    assert!(
        checkpoint.contains("expected.rehypothesis_count = gate.rehypothesis_count"),
        "verify_link recomputes the count from the gate row"
    );
    assert!(
        !checkpoint.contains("rehypothesis_count +="),
        "no increment operator anywhere in the checkpoint module"
    );
}

/// S-3: no-confidence / no-grade in the admission arms. The helper
/// reads no `confidence`, no `certainty`, no `CertaintyGrade`, no
/// float; and the ranker's dominance weights are byte-untouched by the
/// lane (read, never re-graded).
///
/// Proof by damage: plant `h.confidence` or a `case.certainty` read in
/// the helper and this reds.
#[test]
fn s3_admission_arms_read_no_confidence_or_grade() {
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl
        .find("pub(crate) fn rehypothesis_admits")
        .expect("the admission helper must exist");
    let end = gdl[start..]
        .find("fn verify_gate")
        .expect("verify_gate must follow the helper");
    let helper = &gdl[start..start + end];
    for token in ["confidence", "certainty", "CertaintyGrade", "f64", "rank"] {
        assert!(
            !helper.contains(token),
            "the admission arms must not read `{token}` (index arithmetic only)"
        );
    }
    // The ranker's outputs are untouched: the dominance weights still
    // stand at their CG-03 values.
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
    // And the grade was not touched to make room.
    let certainty = prod_code("src/workflow/certainty.rs");
    assert!(
        certainty.contains("pub(crate) fn for_case"),
        "certainty::for_case still exists (CG-01 module intact)"
    );
}

/// W-1 structural half: the proposal skips serialization when absent
/// (linear cases persist no proposal key); the count is serde-defaulted
/// (old states parse to 0 — behavioral half in-file).
#[test]
fn w1_structural_byte_attrs() {
    let gdl = prod_code("src/workflow/gdl.rs");
    let case_start = gdl
        .find("pub(crate) struct GdlCase")
        .expect("GdlCase must exist");
    let case_end = gdl[case_start..]
        .find("impl GdlCase")
        .expect("impl GdlCase must follow the struct");
    let shape = &gdl[case_start..case_start + case_end];
    assert!(
        shape.contains("skip_serializing_if = \"Option::is_none\"")
            && shape.contains("pub rehypothesis: Option<RehypothesisArtifact>"),
        "the case's proposal field must be skip-if-None (linear bytes: no key)"
    );
    assert!(
        shape.contains("pub rehypothesis_count: u32"),
        "the count field must exist with a serde default (old states parse to 0)"
    );
    // The Verify artifact side says the same for the artifact bytes.
    let verify_start = gdl
        .find("pub(crate) struct VerifyArtifact")
        .expect("VerifyArtifact must exist");
    let verify_end = gdl[verify_start..]
        .find("pub(crate) struct HandoffArtifact")
        .expect("HandoffArtifact must follow VerifyArtifact");
    let verify_shape = &gdl[verify_start..verify_start + verify_end];
    assert!(
        verify_shape.contains("skip_serializing_if = \"Option::is_none\""),
        "the artifact proposal must skip-when-None (pin N-1's artifact bytes)"
    );
    // The wire rides state_json: openapi gains no rehypothesis line (W-2's
    // file-level half; the regen-empty proof is verify-after §4.5).
    if let Ok(openapi) = std::fs::read_to_string(repo("openapi-schema/openapi.yaml")) {
        assert!(
            !openapi.contains("rehypothesis"),
            "no wire line in the schema — the bytes ride state_json"
        );
    }
}

/// The eval consumer is live: `EvalRow` carries the count off persisted
/// state with a serde default (old traces parse to None).
#[test]
fn eval_row_carries_the_count_off_persisted_state() {
    let eval = prod_code("src/workflow/gdl_eval.rs");
    assert!(
        eval.contains("pub(crate) rehypothesis_count: Option<u32>"),
        "EvalRow.rehypothesis_count must exist"
    );
    assert!(
        eval.contains("final_case.rehypothesis_count"),
        "the row reads the count off the persisted case state"
    );
}
