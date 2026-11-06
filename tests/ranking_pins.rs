//! CG-03 — the ranked differential's structural pins (S-2..S-5, R-7).
//!
//! **The invariants.** The ranker (`ranking.rs`) is advisory by ordering:
//! it decides what is tested FIRST, never passes/fails a gate by itself.
//! Four structural pins hold that construction in place:
//!
//! - **S-2 · one compute point.** `ranking::ranked` is called from
//!   exactly one place outside its module — `gdl::apply`'s Hypothesize
//!   arm. No gate, handler, or reader ever recomputes at read time.
//! - **S-3 · persist-blind.** `gdl_checkpoint.rs` never references
//!   `rank`; the normalized Hypothesize artifact carries no rank key
//!   (the gate canonicalizes model-sent ranks to `None`), so the
//!   load-bearing artifact-bytes comparison is untouched.
//! - **S-4 · no-confidence.** The ranker and the Plan rank checks never
//!   read `confidence` (the house law: no gate reads it, no threshold
//!   binds it) and never touch a float.
//! - **S-5 · certainty untouched.** `ranking.rs` never reads
//!   `CertaintyGrade` — per-hypothesis strength is re-derived from the
//!   same grounded inputs, `certainty.rs` is byte-untouched by this lane.
//! - **R-7 · purity.** `ranking.rs` is index arithmetic: no model, no
//!   I/O, no `unsafe`, no float.
//!
//! Matching runs against comment-stripped code (`common::code_only`) —
//! a pin that greps raw text passes on a comment that merely names the
//! symbol it forbids (the false-pass CG-01 recorded).

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
/// `mod tests {` (the CG-01 precedent — scanning the whole file would
/// let the module's own tests trip the gates they exist to prove).
fn prod_code(rel: &str) -> String {
    let code = code_only(&read(rel));
    code.split("\nmod tests {")
        .next()
        .expect("a tests module")
        .to_string()
}

/// S-2: `ranking::ranked` is called from exactly one site outside its
/// module — the `apply` compute point in `gdl.rs`'s production region.
/// (`ranking::specific_*` are the Plan gate's needle helpers, not
/// recomputes — the scan counts `ranking::ranked` only.)
///
/// Proof by damage: plant `let _ = super::ranking::ranked;` in any
/// other module's production region and this reds ("found 2 call sites").
#[test]
fn s2_ranked_has_exactly_one_call_site_outside_its_module() {
    let mut sites: Vec<(String, usize)> = Vec::new();
    for p in src_files() {
        let rel = p.strip_prefix(repo(".")).expect("relative path");
        if rel == Path::new("src/workflow/ranking.rs") {
            continue; // the module itself (definition + its own tests)
        }
        let code = prod_code(rel.to_str().expect("utf8 path"));
        let n = code.matches("ranking::ranked").count();
        if n > 0 {
            sites.push((rel.to_string_lossy().into_owned(), n));
        }
    }
    assert_eq!(
        sites,
        vec![("src/workflow/gdl.rs".to_string(), 1usize)],
        "ranking::ranked must have exactly one production call site: \
         gdl::apply's Hypothesize arm (rank is born once, at construction)"
    );
}

/// S-3: the persist path is rank-blind. `gdl_checkpoint.rs` never
/// references `rank` — its comparison is committed vs receipt ARTIFACT
/// BYTES, and the normalized Hypothesize artifact carries no rank key
/// (the gate canonicalizes model-sent ranks to `None`, CG-03 C-6).
///
/// Proof by damage: plant a `rank` reference in `Checkpoint::validate`
/// and this reds.
#[test]
fn s3_persist_path_is_rank_blind() {
    let checkpoint = code_only(&read("src/workflow/gdl_checkpoint.rs"));
    assert!(
        !checkpoint.contains("rank"),
        "gdl_checkpoint.rs must not reference rank — the persist comparison \
         compares artifact bytes (canonicalized None), not ranks"
    );
    // The normalize side stamps no rank either: the Hypothesize arm's
    // canonicalization is the writer, `accept_ungated` never adds one.
    let gdl = prod_code("src/workflow/gdl.rs");
    assert!(
        gdl.contains("h.rank = None"),
        "the gate canonicalizes model-sent ranks to None (the persist-blind writer)"
    );
}

/// S-4: no-confidence. The ranker and the Plan rank checks contain zero
/// `confidence` reads (comment-stripped) and zero `f64` — score is `i32`
/// only, per the house law (no gate reads `confidence`, no threshold
/// binds it).
///
/// Proof by damage: plant `h.confidence` in `ranking.rs` and this reds.
#[test]
fn s4_no_confidence_reads_no_float() {
    let ranking = prod_code("src/workflow/ranking.rs");
    assert!(
        !ranking.contains("confidence"),
        "ranking.rs must not read confidence — the ranker is index arithmetic, \
         never the model's subjective self-report"
    );
    assert!(
        !ranking.contains("f64"),
        "ranking.rs must not touch a float — score is i32 only (series law 3)"
    );
    // The Plan rank checks (the only gate-adjacent rank consumer) —
    // slice the helper out of gdl.rs production code.
    let gdl = prod_code("src/workflow/gdl.rs");
    let start = gdl
        .find("fn plan_rank_errors")
        .expect("plan_rank_errors must exist");
    let end = gdl[start..]
        .find("fn act_gate")
        .expect("act_gate must follow plan_rank_errors");
    let helper = &gdl[start..start + end];
    assert!(
        !helper.contains("confidence") && !helper.contains("f64"),
        "plan_rank_errors must not read confidence or touch a float"
    );
}

/// S-5: certainty untouched. `ranking.rs` never reads `CertaintyGrade`
/// (per-hypothesis strength is re-derived from the same grounded
/// inputs — CG-03 C-2), and the grade's own pins stay green beside this
/// lane (witnessed in verify-after §4.8, not duplicated here).
///
/// Proof by damage: plant a `case.certainty` read in `ranking.rs` and
/// this reds.
#[test]
fn s5_ranker_never_reads_the_grade() {
    let ranking = prod_code("src/workflow/ranking.rs");
    assert!(
        !ranking.contains("certainty") && !ranking.contains("CertaintyGrade"),
        "ranking.rs must not read the case-level grade — it re-derives \
         per-hypothesis strength from the same grounded inputs (C-2)"
    );
    // And the grade was not touched to make room: its module still
    // carries exactly its CG-01 shape (the pins in certainty_pins.rs
    // hold the rest; this guards the import surface here).
    let certainty = prod_code("src/workflow/certainty.rs");
    assert!(
        certainty.contains("pub(crate) fn for_case"),
        "certainty::for_case still exists (CG-01 module intact)"
    );
}

/// R-7: purity — the ranker is index arithmetic (the CG-01 P-7 shape).
/// The crate-level `deny(unsafe_code)` is the language's stronger
/// guard against the unsafe class; this scan backstops it for the
/// model/I-O/float classes the deny cannot name.
#[test]
fn r7_ranker_module_is_pure() {
    let code = code_only(&read("src/workflow/ranking.rs"));
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
        "f64",
    ] {
        assert!(
            !code.contains(token),
            "ranking.rs must not reference `{token}` — the ranker is index \
             arithmetic over committed case state, not a model or I/O readout"
        );
    }
}
