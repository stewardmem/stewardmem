//! The workspace import's target-resolution pins — the posture laws (R150).
//!
//! MEASURED (2026-11-03): the shipped resolution hard-coded the app
//! posture's store while the attached kernel serves wherever its operator
//! pointed it — on the operator's machine the card named a file that did
//! not exist while the kernel served the workspace store. These pins hold
//! the replacement's laws over a temp data dir: the vouched postures
//! resolve their own store and require it to EXIST, the service choice
//! without an install is unresolved (the marker names a choice, the
//! install names a fact), a foreign kernel is never vouched, and the
//! degenerate source==target case refuses by name.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::import::{ensure_distinct, target_for};

// ── the harness ─────────────────────────────────────────────────────────────

/// A temp data dir with the named shape: `kernel/brain.db` and/or
/// `service/brain.db` created as real files when asked.
fn data_dir(kernel: bool, service: bool) -> PathBuf {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "brain-import-pins-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("the pin creates its own fixture dir");
    for (make, sub) in [(kernel, "kernel"), (service, "service")] {
        if make {
            let sub_dir = dir.join(sub);
            fs::create_dir_all(&sub_dir).expect("the pin creates its own fixture dir");
            fs::set_permissions(&sub_dir, fs::Permissions::from_mode(0o700))
                .expect("the fixture dir takes 0700");
            fs::write(sub_dir.join("brain.db"), b"fixture store\n")
                .expect("the fixture store is writable");
        }
    }
    dir
}

// ── the posture laws ────────────────────────────────────────────────────────

#[test]
fn the_app_posture_resolves_the_kernel_store_it_spawns() {
    let dir = data_dir(true, false);
    let target = target_for("app", &dir).expect("the app posture vouches for its own store");
    assert_eq!(target, dir.join("kernel").join("brain.db"));
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

#[test]
fn a_vouched_posture_whose_store_never_booted_is_named_missing() {
    let dir = data_dir(false, false);
    let error = target_for("app", &dir).expect_err("no store: the refusal is the law");
    assert!(
        error.starts_with("import_target_missing"),
        "the refusal names its kind, got {error}"
    );
    assert!(
        error.contains("the kernel's boot owns store creation"),
        "the refusal says who creates a store, got {error}"
    );
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

#[test]
fn the_service_posture_resolves_the_install_root_when_it_exists() {
    let dir = data_dir(false, true);
    let target =
        target_for("service", &dir).expect("an installed service vouches for its own store");
    assert_eq!(target, dir.join("service").join("brain.db"));
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

#[test]
fn a_service_marker_without_an_install_is_unresolved() {
    let dir = data_dir(false, false);
    let error = target_for("service", &dir)
        .expect_err("the marker names a choice; the install names a fact");
    assert!(
        error.starts_with("import_target_unresolved"),
        "the refusal names its kind, got {error}"
    );
    assert!(
        error.contains("has not installed its service"),
        "the refusal says what is missing, got {error}"
    );
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

#[test]
fn a_kernel_the_app_did_not_start_is_never_vouched() {
    let dir = data_dir(true, true);
    for posture in ["external", "", " mysteriously-vouched "] {
        let error =
            target_for(posture, &dir).expect_err("a foreign kernel's store is not ours to guess");
        assert!(
            error.starts_with("import_target_unresolved"),
            "posture {posture:?} must be unresolved, got {error}"
        );
        assert!(
            error.contains("cannot vouch for which store an import would land in"),
            "the refusal names the reason, got {error}"
        );
    }
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

// ── the degenerate case ─────────────────────────────────────────────────────

#[test]
fn a_source_that_is_the_kernel_own_store_refuses_by_name() {
    let dir = data_dir(true, false);
    let target = target_for("app", &dir).expect("the vouched store exists");
    // The canonical form — what validate_source hands over.
    let source = target.canonicalize().expect("the fixture store exists");
    let error = ensure_distinct(&source, &target).expect_err("importing a store into itself");
    assert!(
        error.starts_with("import_source_is_target"),
        "the refusal names its kind, got {error}"
    );
    // A genuinely different source passes.
    let other = dir.join("kernel").join("other.db");
    fs::write(&other, b"not the target\n").expect("the fixture is writable");
    ensure_distinct(&other, &target).expect("a different source is not the target");
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}

#[test]
fn the_target_comparison_survives_a_symlinked_target_path() {
    let dir = data_dir(true, false);
    let target = target_for("app", &dir).expect("the vouched store exists");
    let link = dir.join("kernel").join("alias.db");
    std::os::unix::fs::symlink(&target, &link).expect("the pin creates its own symlink");
    let source = target.canonicalize().expect("the fixture store exists");
    // The alias names the same file: the canonical comparison catches it.
    let error = ensure_distinct(&source, &link).expect_err("an alias of the target is the target");
    assert!(error.starts_with("import_source_is_target"), "got {error}");
    fs::remove_dir_all(&dir).expect("the pin cleans its fixture");
}
