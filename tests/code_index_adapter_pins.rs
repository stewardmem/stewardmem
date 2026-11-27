//! Pins 17 and 19 — `sql_lives_in_the_core_only` and
//! `the_kernel_adapter_is_wiring_only`. The kernel's code-index rows are
//! adapters in the house shape: flags, parse, print, exit. No SQL, no
//! tree-sitter, no hashing lives in `src/bin/brain.rs`'s code-index region;
//! every statement lives in `brain-code-core` and the kernel fills its two
//! ports (the audited vec0 registration and the active profile's embedder).
//!
//! The guard extracts every code-index unit by name (so renames fail loud,
//! never vacuous) and scans its CODE lines — comments are stripped first,
//! because the units' doc comments legitimately talk about what is absent.

use std::collections::BTreeMap;

/// The unit-name prefixes that make up the code-index adapter region. If a
/// unit is renamed away from these prefixes the guard goes red on the
/// region being too small — an intentional rename updates this list as a
/// named act.
const UNIT_PREFIXES: [&str; 5] = [
    "fn cmd_code_index_",
    "fn code_index_",
    "struct CodeIndex",
    "impl brain_code_core::host::Vec0Host for CodeIndex",
    "impl brain_code_core::encoder::Encoder for CodeIndex",
];

/// What must never appear in adapter code: the crate owns every statement,
/// every parse, every digest.
const FORBIDDEN: [&str; 9] = [
    "SELECT ",
    "INSERT ",
    "UPDATE ",
    "DELETE ",
    "PRAGMA ",
    "tree_sitter",
    "TreeSitter",
    "Sha256",
    "sha2::",
];

fn adapter_units(src: &str) -> BTreeMap<String, Vec<String>> {
    let mut units = BTreeMap::new();
    for (idx, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if !UNIT_PREFIXES.iter().any(|p| trimmed.starts_with(*p)) {
            continue;
        }
        // Extract to the unit's closing brace by brace depth. Strings with
        // braces exist in the units (format strings), so depth counting can
        // overshoot — the region just needs to COVER the unit, and an
        // overshoot only widens the scan, which errs safe.
        let mut depth = 0i64;
        let mut body = Vec::new();
        for inner in src.lines().skip(idx) {
            let code = inner.trim_start();
            if code.starts_with("//") {
                continue;
            }
            body.push(inner.to_string());
            depth += inner.matches('{').count() as i64;
            depth -= inner.matches('}').count() as i64;
            if depth <= 0 && body.len() > 1 {
                break;
            }
        }
        let name = trimmed
            .split(['(', '<', ' '])
            .next()
            .unwrap_or(trimmed)
            .to_string();
        units.insert(format!("L{}:{name}", idx + 1), body);
    }
    units
}

#[test]
fn the_code_index_adapter_is_wiring_only() {
    let src = include_str!("../src/bin/brain.rs");
    let units = adapter_units(src);
    assert!(
        units.len() >= 6,
        "the code-index adapter region vanished or was renamed — the guard must never pass \
         vacuously (found {} units)",
        units.len()
    );
    let scanned: usize = units.values().map(|body| body.len()).sum();
    assert!(
        scanned >= 60,
        "the adapter region is suspiciously small ({scanned} lines) — the guard must never \
         pass vacuously"
    );
    let mut offenders = Vec::new();
    for (name, body) in &units {
        for (idx, line) in body.iter().enumerate() {
            for token in FORBIDDEN {
                if line.contains(token) {
                    offenders.push(format!("{name}+{idx}: {token}"));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the code-index adapter stopped being wiring-only: {} — the crate owns every \
         statement (pins 17/19)",
        offenders.join("; ")
    );
}
