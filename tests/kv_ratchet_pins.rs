//! The KV-ratchet preregistration pins (IMPROVE10 S0): the ratchet's
//! constant is a preregistered promise, never a preference — the CG-04
//! law (`src/workflow/gdl.rs:74-75`) applied to `MLXChatTier.kvScheme`.
//!
//! RED LEDGER (2026-11-09, against `f0c68498` before this round's docs):
//!   1. absence — `docs/REKRA_PLAN_PREREGISTRATION_*.md` did not exist
//!      (the gap the round closes, tree-verified twice); every doc test
//!      in this file red with "no preregistration exists yet" — the
//!      gate that stops any matrix run before the rule is committed.
//!   2. (post-landing mutation proof, S0) the ENGINE PIN line deleted
//!      from the doc re-reds `the_engine_pin_line_names_the_engine` and
//!      `a_preregistration_without_an_engine_pin_reds` names the law —
//!      the pin working on the future path (KR-4's re-red clause).
//!
//! Green: the preregistration landed (S0) and the pins went green
//! against it in the same commit — git order IS KR-1 (the S0 commit
//! hash precedes every matrix run and is recorded in the evidence).
//!
//! The laws pinned here (plan 10 §3 KR-4 + the execution prompt):
//!   KR-4  the newest preregistration carries: the decision rule, the
//!         tier-scoped length ladders, the memory arithmetic, the
//!         refusal ledger, and the ENGINE PIN line — and a future
//!         re-preregistration WITHOUT an engine pin re-reds by name.
//!   This round's engine is pinned too: `3.32.3` = `3b339ad6` (a bump
//!   round updates the doc AND this pin consciously — the EngineFloorPins
//!   pattern; the plan-09 bump re-runs the matrix as a named act).
//!   The full roster (all seven MLX wire tiers) must be named — a
//!   ladders section that silently drops a tier hides a refusal.

use std::path::{Path, PathBuf};

/// The docs directory, at the repo root.
fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs")
}

/// The NEWEST preregistration (lexicographically last
/// `REKRA_PLAN_PREREGISTRATION_*.md`), std-only — no glob crate.
/// Absence is THE red: the matrix runs are gated on a committed
/// preregistration (KR-1), so a missing doc is not an empty pass.
fn newest_prereg_doc() -> Result<(PathBuf, String), String> {
    let dir = docs_dir();
    let mut newest: Option<PathBuf> = None;
    let entries = std::fs::read_dir(&dir)
        .map_err(|_| format!("the docs directory does not exist: {}", dir.display()))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("REKRA_PLAN_PREREGISTRATION_") && name.ends_with(".md") {
            match &newest {
                Some(current) => {
                    let current_name = current.file_name().unwrap().to_string_lossy();
                    if name > current_name {
                        newest = Some(entry.path());
                    }
                }
                None => newest = Some(entry.path()),
            }
        }
    }
    let path = newest.ok_or_else(|| {
        "no preregistration exists yet — the KV ratchet's matrix runs are gated on a committed \
         preregistration (KR-1/KR-4): the decision rule, the tier-scoped ladders, the engine pin \
         and the refusal ledger land BEFORE any run, and their home is \
         docs/REKRA_PLAN_PREREGISTRATION_<date>.md"
            .to_string()
    })?;
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "the newest preregistration is unreadable: {}",
            path.display()
        )
    })?;
    Ok((path, text))
}

/// The ENGINE PIN checker (KR-4's future-path law): finds the
/// `ENGINE PIN` line and extracts the `tag = commit` pair. A
/// preregistration without one is not a preregistration — the scheme
/// rows are engine-pinned and a constant moved on one engine's table
/// does not silently transfer (plan 10 §0.5 fact 5).
fn engine_pin_line(text: &str) -> Result<(String, String), String> {
    let line = text
        .lines()
        .find(|l| l.contains("ENGINE PIN:"))
        .ok_or_else(|| {
            "the preregistration carries no ENGINE PIN line — a re-preregistration without an \
             engine pin re-reds (KR-4): every table names its engine, and the bump re-runs the \
             matrix as a named act"
                .to_string()
        })?;
    let after = line.split("ENGINE PIN").nth(1).unwrap_or("");
    // The shape is `<engine> <tag> = <commit>` — the tag is the token
    // immediately before `=`, the commit the token after it, both bare
    // of markdown punctuation (the doc line is bold: `**…3b339ad6**`).
    let strip_md = |t: &str| {
        t.trim_matches(|c: char| matches!(c, '*' | '(' | ')' | '[' | ']' | ',' | ';' | ':' | '`'))
            .to_string()
    };
    let tokens: Vec<String> = after.split_whitespace().map(strip_md).collect();
    let eq = tokens.iter().position(|t| t == "=").ok_or_else(|| {
        "the ENGINE PIN line carries no `=` (shape: `ENGINE PIN: mlx-swift-lm 3.32.3 = 3b339ad6`)"
            .to_string()
    })?;
    let tag = eq
        .checked_sub(1)
        .and_then(|i| tokens.get(i))
        .ok_or_else(|| {
            "the ENGINE PIN line carries no tag token before `=` (shape: `ENGINE PIN: mlx-swift-lm 3.32.3 = 3b339ad6`)"
                .to_string()
        })?
        .clone();
    let commit = tokens
        .get(eq + 1)
        .ok_or_else(|| {
            "the ENGINE PIN line carries no commit hash after `=` (shape: `ENGINE PIN: mlx-swift-lm 3.32.3 = 3b339ad6`)"
                .to_string()
        })?
        .clone();
    let hexish =
        |t: &str| !t.is_empty() && t.len() >= 7 && t.chars().all(|c| c.is_ascii_hexdigit());
    if !hexish(&commit) {
        return Err(format!(
            "the ENGINE PIN line's commit token `{commit}` is not a commit hash — the pin names the engine AND its checkout"
        ));
    }
    Ok((tag, commit))
}

/// The seven MLX wire tier ids (MLXChatTier.tier(modelId:), the roster
/// of record). A ladders section that drops a tier hides a refusal.
const ROSTER: [&str; 7] = [
    "qwen3-8b-4bit",
    "qwen3.5-2b-4bit",
    "qwen3.5-9b-heretic-5bit",
    "qwen3.5-9b-4bit",
    "qwen3.5-4b-uncensored-4bit",
    "qwen3.5-9b-heretic-4bit",
    "qwen3.5-4b-huihui-4bit",
];

// ── the pins ─────────────────────────────────────────────────────────────

#[test]
fn the_preregistration_exists() {
    // RED (the ledger's shape 1): the doc was absent before this round —
    // this test's message IS the gate, by name.
    let (path, text) = newest_prereg_doc().expect("the preregistration must exist");
    assert!(
        text.trim().len() > 200,
        "the preregistration at {} is effectively empty — a stub is not a preregistration",
        path.display()
    );
}

#[test]
fn the_engine_pin_line_names_the_engine() {
    let (path, text) = newest_prereg_doc().expect("the preregistration must exist");
    let (tag, commit) = engine_pin_line(&text).expect("the ENGINE PIN line must parse (KR-4)");
    assert_eq!(
        tag,
        "3.32.3",
        "the preregistration at {} pins engine tag `{}` — this round's matrix runs on 3.32.3; \
         a bump round updates the doc AND this pin consciously (the EngineFloorPins pattern)",
        path.display(),
        tag
    );
    assert!(
        commit.starts_with("3b339ad6"),
        "the preregistration pins commit `{}` — expected the 3.32.3 checkout 3b339ad6 (the \
         engine the tables name)",
        commit
    );
}

#[test]
fn a_preregistration_without_an_engine_pin_reds() {
    // KR-4's future-path law, exercised on synthetic texts so it holds
    // regardless of the live doc's posture: the checker is the pin.
    assert!(engine_pin_line("ENGINE PIN: mlx-swift-lm 3.32.3 = 3b339ad6").is_ok());
    assert!(
        engine_pin_line("some prose without the line").is_err(),
        "a preregistration without an ENGINE PIN line must red"
    );
    assert!(
        engine_pin_line("ENGINE PIN: (none — the engine moved)").is_err(),
        "an ENGINE PIN line without a tag=commit pair must red"
    );
    assert!(
        engine_pin_line("ENGINE PIN: mlx-swift-lm = ").is_err(),
        "an ENGINE PIN line with a bare `=` must red"
    );
}

#[test]
fn the_decision_rule_section_is_present() {
    let (_, text) = newest_prereg_doc().expect("the preregistration must exist");
    assert!(
        text.contains("## The preregistered decision rule"),
        "the preregistration lost its decision-rule section — the rule is what makes the \
         ratchet a promise instead of a preference (KR-4); restore it"
    );
}

#[test]
fn the_tier_scoped_ladders_section_is_present() {
    let (_, text) = newest_prereg_doc().expect("the preregistration must exist");
    assert!(
        text.contains("## Tier-scoped length ladders"),
        "the preregistration lost its tier-scoped ladders — lengths are scoped by arithmetic \
         before the first run (the 8B declares 40,960; the plan's one-size ladder is impossible) \
         (KR-4); restore it"
    );
}

#[test]
fn the_refusal_ledger_section_is_present() {
    let (_, text) = newest_prereg_doc().expect("the preregistration must exist");
    assert!(
        text.contains("## The refusal ledger"),
        "the preregistration lost its refusal ledger — OOM / priced-out / ceiling refusals are \
         NAMED rows, never silent scope loss (KR-4); restore it"
    );
}

#[test]
fn the_memory_arithmetic_section_is_present() {
    let (_, text) = newest_prereg_doc().expect("the preregistration must exist");
    assert!(
        text.contains("## Memory arithmetic"),
        "the preregistration lost its memory-arithmetic table — the ladders are scoped by the \
         config-derived KV bytes/token BEFORE any run (DERIVED-FROM-CONFIGS); restore it"
    );
}

#[test]
fn the_ladders_name_every_roster_tier_or_its_refusal() {
    let (_, text) = newest_prereg_doc().expect("the preregistration must exist");
    for tier in ROSTER {
        assert!(
            text.contains(tier),
            "the preregistration never names wire tier `{}` — a ladders section that drops a \
             tier hides a refusal (the roster is seven MLX tiers; unmeasured tiers carry their \
             named posture, this round: transfer refused pending their own matrix)",
            tier
        );
    }
}
