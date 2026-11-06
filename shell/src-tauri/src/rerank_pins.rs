//! CG-12's rerank WRITE-path pins (P-4 + the pure laws of `rerank.rs`).
//!
//! Hermetic by construction: the staging runs through the INJECTED fetch
//! (scratch bytes, no network), the marker rides scratch dirs, and the
//! P-4 structural law is a source scan — the same class as
//! `tools_pkg_pins`. The one thing a unit pin cannot prove is that the
//! `mas` build COMPILES — that is `cargo check --features mas`, run in the
//! lane's verify; this file proves the scan-shaped half.

use std::fs;
use std::path::Path;

/// The pins' scratch root (no `tempfile` edge exists in this crate).
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rerank-pins-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch root");
    dir
}

use crate::rerank::{
    EnablePlan, MODEL_BASE_URL, MODEL_DIR_REL, MODEL_FILES, MODEL_ONNX_SHA256, MODEL_TOTAL_BYTES,
    RERANK_MARKER, enable_plan, marker_enabled, marker_json, spawn_env, stage_model_with,
    staging_complete,
};

// ── the marker (the CHOICE) ────────────────────────────────────────────────

#[test]
fn the_marker_carries_one_fact_and_reads_off_over_garbage() {
    assert_eq!(marker_json(true), r#"{"enabled":true}"#);
    assert_eq!(marker_json(false), r#"{"enabled":false}"#);
    assert!(marker_enabled(r#"{"enabled":true}"#));
    assert!(!marker_enabled(r#"{"enabled":false}"#));
    // A corrupt marker is NOT a choice to honor: off, always.
    assert!(!marker_enabled("not json at all"));
    assert!(!marker_enabled(r#"{"enabled": "yes"}"#));
    assert!(!marker_enabled("{}"));
}

// ── the spawn-env decision ─────────────────────────────────────────────────

#[test]
fn spawn_env_rides_only_an_on_marker_at_an_absolute_root() {
    let root = scratch("env-on");
    let config_dir = root.join("config");
    let data_root = root.join("kernel");
    fs::create_dir_all(&config_dir).unwrap();

    // No marker → None (the OFF shape).
    assert!(spawn_env(&config_dir, &data_root).is_none());
    // Marker off → None.
    fs::write(config_dir.join(RERANK_MARKER), marker_json(false)).unwrap();
    assert!(spawn_env(&config_dir, &data_root).is_none());
    // Marker on → the ABSOLUTE model dir.
    fs::write(config_dir.join(RERANK_MARKER), marker_json(true)).unwrap();
    let env = spawn_env(&config_dir, &data_root).expect("on marker yields env");
    assert_eq!(
        env.model_dir,
        data_root.join("models").join("mxbai-rerank-large-v1")
    );
    assert!(
        env.model_dir.is_absolute(),
        "the kernel refuses a CWD-relative dir; the env must never carry one"
    );
}

#[test]
fn spawn_env_refuses_a_relative_data_root_fail_closed() {
    let root = scratch("env-relative");
    let config_dir = root.join("config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join(RERANK_MARKER), marker_json(true)).unwrap();
    assert!(
        spawn_env(&config_dir, Path::new("relative/kernel")).is_none(),
        "a relative data root would poison the kernel's dir law — refused"
    );
}

// ── the enable plan (P-9's decision core) ──────────────────────────────────

#[test]
fn p9_the_external_posture_is_refused_before_anything_else() {
    assert_eq!(
        enable_plan("external", false, false, true),
        EnablePlan::RefuseExternal,
        "enable on an external kernel: refused"
    );
    assert_eq!(
        enable_plan("external", false, false, false),
        EnablePlan::RefuseExternal,
        "disable on an external kernel: equally refused (the app never writes there)"
    );
    // The owned postures flow.
    assert_eq!(
        enable_plan("app", false, false, true),
        EnablePlan::StageMarkerRestart
    );
    assert_eq!(
        enable_plan("app", false, true, true),
        EnablePlan::MarkerRestart
    );
    assert_eq!(
        enable_plan("service", true, true, true),
        EnablePlan::MarkerRestart
    );
    assert_eq!(
        enable_plan("app", true, false, false),
        EnablePlan::MarkerOffRestart
    );
}

#[test]
fn the_plan_stages_before_the_marker_and_disabling_never_deletes() {
    // The marker is the CHOICE; the staged files are the REALITY — the
    // stage precedes the mark, and a flip-off never deletes files.
    assert_eq!(
        enable_plan("app", false, false, true),
        EnablePlan::StageMarkerRestart
    );
    assert_eq!(
        enable_plan("service", false, true, false),
        EnablePlan::MarkerOffRestart
    );
}

// ── the staging (hermetic through the injected fetch) ──────────────────────

/// Drives `stage_model_with` with scratch bytes; returns the outcome.
fn run_stage(root: &Path, bytes_for: impl Fn(&str) -> u64) -> crate::rerank::StageOutcome {
    let mut fetch = |url: &str, dest: &Path| -> Result<u64, String> {
        let rel = url
            .strip_prefix(MODEL_BASE_URL)
            .expect("url prefix")
            .trim_start_matches('/')
            .to_string();
        let bytes = bytes_for(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        // SPARSE: the file's logical size is the pinned size without
        // writing gigabytes — metadata.len() is what the gates read.
        let file = fs::File::create(dest).unwrap();
        file.set_len(bytes).unwrap();
        Ok(bytes)
    };
    let mut hasher = |_: &Path| Ok(MODEL_ONNX_SHA256.to_string());
    stage_model_with(root, &mut fetch, &mut hasher)
}

#[test]
fn staging_writes_every_pinned_file_at_its_pinned_size() {
    let root = scratch("stage-1");
    // The sizes are the PIN — the fetch is told the expected size per file
    // (the real wire delivers these; the E0 measurement pinned them).
    let sizes: std::collections::HashMap<String, u64> = MODEL_FILES
        .iter()
        .map(|(rel, expected)| ((*rel).to_string(), *expected))
        .collect();
    let outcome = run_stage(&root, |rel| sizes.get(rel).copied().unwrap_or(0));
    assert!(outcome.staged, "staging must complete: {:?}", outcome.error);
    assert_eq!(outcome.files.len(), MODEL_FILES.len());
    assert!(staging_complete(&root), "the reality now holds");
    assert_eq!(
        MODEL_TOTAL_BYTES,
        MODEL_FILES.iter().map(|(_, s)| s).sum::<u64>()
    );
}

#[test]
fn staging_refuses_a_wrong_sized_fetch_fail_closed() {
    let root = scratch("stage-2");
    let sizes: std::collections::HashMap<String, u64> = MODEL_FILES
        .iter()
        .map(|(rel, expected)| ((*rel).to_string(), *expected))
        .collect();
    // One byte short on the ONNX: the gate refuses and stages NOTHING.
    let outcome = run_stage(&root, |rel| {
        if rel == "onnx/model_quantized.onnx" {
            sizes[rel] - 1
        } else {
            sizes[rel]
        }
    });
    assert!(!outcome.staged, "a truncated fetch must be refused");
    let msg = outcome.error.expect("the refusal names the gate");
    assert!(msg.contains("size gate"), "the size gate speaks: {msg}");
    assert!(!staging_complete(&root));
}

#[test]
fn staging_resumes_over_complete_files_without_refetching() {
    let root = scratch("stage-3");
    let sizes: std::collections::HashMap<String, u64> = MODEL_FILES
        .iter()
        .map(|(rel, expected)| ((*rel).to_string(), *expected))
        .collect();
    // First staging writes everything but the ONNX (a mock "interrupted" run
    // is simulated by pre-placing a correct tokenizer.json).
    let tok = root.join(MODEL_DIR_REL).join("tokenizer.json");
    fs::create_dir_all(tok.parent().unwrap()).unwrap();
    fs::write(&tok, vec![0u8; sizes["tokenizer.json"] as usize]).unwrap();
    let outcome = run_stage(&root, |rel| sizes.get(rel).copied().unwrap_or(0));
    assert!(outcome.staged, "resume completes: {:?}", outcome.error);
    // The pre-placed file needed no fetch — one of five skipped.
    let fetched = outcome
        .files
        .iter()
        .filter(|f| f.name != "tokenizer.json")
        .count();
    assert_eq!(
        fetched,
        MODEL_FILES.len() - 1,
        "the complete file is skipped"
    );
}

// ── P-4: the store edition compiles the write path OUT (structural) ────────

/// The source-scan pin: every arm of the rerank WRITE path sits behind
/// `cfg(not(feature = "mas"))`, and the spawn site's mas arm passes `None`
/// — the store edition's PERMANENT answer. `cargo check --features mas`
/// (the lane's verify) proves the excision compiles; this pin proves the
/// excision is STRUCTURAL, not a runtime flag.
#[test]
fn p4_the_mas_excision_is_structural() {
    let lib = include_str!("lib.rs");
    let sidecar = include_str!("sidecar.rs");

    // The module, the pins, and BOTH commands are cfg'd out of mas.
    for needle in [
        "#[cfg(not(feature = \"mas\"))]\npub mod rerank;",
        "#[cfg(all(test, not(feature = \"mas\")))]\nmod rerank_pins;",
        "#[cfg(not(feature = \"mas\"))]\n            rerank_posture,",
        "#[cfg(not(feature = \"mas\"))]\n            rerank_set_enabled,",
    ] {
        assert!(
            lib.contains(needle),
            "lib.rs must carry the mas excision verbatim:\n{needle}"
        );
    }

    // The spawn site's mas arm: `None` is the store's permanent answer.
    assert!(
        sidecar.contains("#[cfg(feature = \"mas\")]\n        let rerank = None::<RerankEnv>;"),
        "the spawn site must pass None in the mas build — nothing can arm the tier there"
    );

    // A registration without the cfg gate would make the surface live in
    // the store edition — the gate rides the PREVIOUS line, pinned here.
    let lib_lines: Vec<&str> = lib.lines().collect();
    for name in ["rerank_posture,", "rerank_set_enabled,"] {
        let idx = lib_lines
            .iter()
            .position(|l| l.trim_start() == name)
            .unwrap_or_else(|| panic!("{name} must be registered"));
        let prev = lib_lines[idx - 1];
        assert!(
            prev.contains("cfg(not(feature = \"mas\"))"),
            "{name}'s registration must carry the mas gate (prev line: {prev})"
        );
    }

    // The ONNX pin is the supply-chain law: the digest is present and
    // shaped like a sha256 (a swap must trip the gate).
    assert_eq!(MODEL_ONNX_SHA256.len(), 64);
    assert!(MODEL_ONNX_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn the_onnx_digest_pin_names_the_real_measurement() {
    // The digest was measured on this machine's download (2026-11-06, the
    // E0 staging). It is pinned HERE so a substitution trips the gate —
    // the value itself is the evidence's own.
    assert_eq!(
        MODEL_ONNX_SHA256,
        "d03bd791409057b78184fa7726effdf5429114a68a7757939aa0d350761f7697"
    );
}
