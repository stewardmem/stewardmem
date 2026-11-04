//! CG-01 — the certainty grade's structural pins (P-5, P-7, P-8, P-9).
//!
//! **The invariants.** The evidence-quality grade (`CertaintyGrade`) is
//! advisory by construction (SPEC_ADDENDUM:35 — it influences Handoff
//! routing + P-class SLA *never*, until a named lane builds that
//! consumer; CG-03 is the first candidate and has not landed). Four
//! structural pins hold that construction in place:
//!
//! - **P-5 · one compute point.** `certainty::for_case` is called from
//!   exactly one place outside its module — `gdl::apply`, the only point
//!   committed evidence changes. No handler, router, or reader ever
//!   recomputes at read time.
//! - **P-7 · purity.** `certainty.rs` is index arithmetic: no model, no
//!   I/O, no `unsafe`, no provider, no new imports beyond serde and the
//!   gdl types it reads.
//! - **P-8 · the persist path is grade-blind.** The checkpoint
//!   byte-comparison (`gdl_checkpoint.rs`) never references the grade,
//!   and no per-phase artifact struct carries one — the grade lives on
//!   the case, so the load-bearing artifact-bytes comparison is
//!   untouched by construction.
//! - **P-9 · no gate reads it.** Across all of `gdl.rs` the token
//!   appears on exactly two code lines — the field and the `apply`
//!   compute point — and the CLASS-policy lanes (`confidence`, `kappa`,
//!   `case_status`, `reflection`, `proficiency`) never touch it. The
//!   grade and model-confidence are different questions (CG-01's
//!   NOT-this boundary); a gate reading either would break the
//!   determinism law the same way.
//!
//! Matching runs against comment-stripped code (`common::code_only`) —
//! a pin that greps raw text passes on a comment that merely names the
//! symbol it forbids, the false-pass R51 recorded.

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
/// `mod tests {`. Everything a structural pin constrains lives ABOVE the
/// in-file test module (gdl.rs's `#[cfg(test)]`-gated items outside it
/// contain no grade references — if one ever does, the pin's failure is
/// the named decision that admits it). Scanning the whole file would let
/// the module's OWN tests trip the gate they exist to prove.
fn prod_code(rel: &str) -> String {
    let code = code_only(&read(rel));
    code.split("\nmod tests {")
        .next()
        .expect("a tests module")
        .to_string()
}

/// P-5: `certainty::for_case` is called from exactly one site outside its
/// module — the `apply` compute point in `gdl.rs`'s production region.
/// A stray call anywhere else (a handler warming the grade at read time,
/// a router re-grading, a second production call in gdl.rs itself) makes
/// this two and fails by name.
///
/// Proof by damage: plant `let _ = super::certainty::for_case;` in any
/// other module's production region and this reds ("found 2 call sites").
#[test]
fn p5_for_case_has_exactly_one_call_site_outside_its_module() {
    let mut sites: Vec<(String, usize)> = Vec::new();
    for p in src_files() {
        let rel = p.strip_prefix(repo(".")).expect("relative path");
        if rel == Path::new("src/workflow/certainty.rs") {
            continue; // the module itself (definition + its own tests)
        }
        let code = prod_code(rel.to_str().expect("utf8 path"));
        let n = code.matches("certainty::for_case").count();
        if n > 0 {
            sites.push((rel.to_string_lossy().into_owned(), n));
        }
    }
    assert_eq!(
        sites,
        vec![("src/workflow/gdl.rs".to_string(), 1usize)],
        "certainty::for_case must have exactly one production call site: \
         gdl::apply (the only point committed evidence changes)"
    );
}

/// P-7: the module is pure — no model handle, no provider, no I/O, no
/// `unsafe`, and no imports beyond serde derives and the gdl types it
/// reads (which is also the no-new-dependency pin at the import level;
/// `Cargo.toml`'s `[dependencies]` is witnessed unchanged in the round's
/// verify-after transcript).
///
/// Proof by damage: plant an `unsafe {}` block (or a `ModelManager`
/// reference) in `certainty.rs` and this reds on the named token.
#[test]
fn p7_certainty_module_is_pure() {
    let code = code_only(&read("src/workflow/certainty.rs"));
    for token in [
        "ModelManager",
        "unsafe",
        "LlmProvider",
        "LoopDriver",
        "std::fs",
        "std::net",
        "std::process",
        "tokio::",
        "reqwest",
        "Command::",
    ] {
        assert!(
            !code.contains(token),
            "certainty.rs must not reference `{token}` — the grade is index \
             arithmetic over committed case state, not a model or I/O readout"
        );
    }
    // The production import surface is pinned: exactly serde + the gdl
    // items (the test module's `use super::*;` is not an import surface).
    let prod = prod_code("src/workflow/certainty.rs");
    let use_lines: Vec<&str> = prod
        .lines()
        .filter(|l| l.trim_start().starts_with("use "))
        .collect();
    assert_eq!(
        use_lines.len(),
        2,
        "certainty.rs imports exactly serde and super::gdl — a new `use` \
         line is a new dependency surface and needs its own named decision, \
         found: {use_lines:?}"
    );
    assert!(
        use_lines[0].contains("serde::") && use_lines[1].contains("super::gdl::"),
        "the two imports are serde derives and the gdl reader types, found: {use_lines:?}"
    );
}

/// P-8: the persist path is grade-blind. `gdl_checkpoint.rs` never
/// references the grade — its load-bearing comparison is committed vs
/// receipt ARTIFACT BYTES plus the `confirmed_matches` stamp, and that
/// machinery must not learn about `certainty` — and no per-phase
/// artifact struct carries a grade field (the grade lives on `GdlCase`;
/// C-5 refused per-kind stripping churn).
///
/// Proof by damage: plant any `certainty` reference in
/// `gdl_checkpoint.rs` (or a `CertaintyGrade` field on an artifact
/// struct) and the named scan reds.
#[test]
fn p8_persist_path_is_grade_blind() {
    let checkpoint = code_only(&read("src/workflow/gdl_checkpoint.rs"));
    assert!(
        !checkpoint.contains("certainty"),
        "gdl_checkpoint.rs must not reference the grade — the persist \
         comparison compares artifact bytes, not grades"
    );
    let gdl = code_only(&read("src/workflow/gdl.rs"));
    for artifact in [
        "IntakeArtifact",
        "TriageArtifact",
        "RedFlagArtifact",
        "HypothesizeArtifact",
        "ActArtifact",
        "VerifyArtifact",
        "CaptureArtifact",
        "ClosureArtifact",
        "HandoffArtifact",
    ] {
        let marker = format!("struct {artifact}");
        let start = gdl
            .find(&marker)
            .unwrap_or_else(|| panic!("{marker} must exist in gdl.rs"));
        let body_end = start + gdl[start..].find("\n}").expect("struct body close");
        let body = &gdl[start..body_end];
        assert!(
            !body.contains("CertaintyGrade") && !body.contains("certainty"),
            "the {artifact} struct must not carry the grade — a grade inside \
             an artifact forces per-kind stamp stripping (C-5, refused)"
        );
    }
}

/// P-9: the advisory law, pinned by scan. Across `gdl.rs` code the token
/// `certainty` appears on exactly TWO lines — the additive field and the
/// `apply` compute point — and no gate/verdict function reads it. The
/// CLASS-policy lanes (the NOT-this boundary: model-confidence →
/// human-deferral is a different question over a different input) never
/// reference the grade at all.
///
/// Proof by damage: plant `let _ = &case.certainty;` inside any gate
/// function in `gdl.rs` and the two-line pin reds by name.
#[test]
fn p9_no_gate_reads_the_grade() {
    let gdl = prod_code("src/workflow/gdl.rs");
    let lines: Vec<&str> = gdl.lines().filter(|l| l.contains("certainty")).collect();
    let expected: [&str; 4] = [
        "pub certainty: Option<super::certainty::CertaintyGrade>,",
        "certainty: None,",
        "let grade = super::certainty::for_case(case);",
        "case.certainty = Some(grade);",
    ];
    let surface: Vec<String> = lines.iter().map(|l| l.trim().to_string()).collect();
    let named: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        surface, named,
        "gdl.rs production code must touch the grade on exactly the four \
         structural lines — the additive field, fresh()'s None init, and \
         apply()'s compute+store; any other line is a new reader and the \
         advisory law (SPEC_ADDENDUM:35) says no gate may be one, found: \
         {surface:?}"
    );
    for lane in [
        "src/workflow/confidence.rs",
        "src/workflow/kappa.rs",
        "src/workflow/case_status.rs",
        "src/workflow/reflection.rs",
        "src/workflow/proficiency.rs",
    ] {
        assert!(
            !code_only(&read(lane)).contains("certainty"),
            "{lane} must not reference the grade — the CLASS policy and the \
             evidence grade are different questions (the CG-01 NOT-this \
             boundary); wiring them together needs its own named lane"
        );
    }
}
