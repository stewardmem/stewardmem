//! The workspace import's APP SIDE (the Settings surface's engine) — the
//! boundary that makes the D3 card's "import later from Settings" sentence
//! true. The merge core is the KERNEL's (`migrate_merge.rs`, run through the
//! bundled sidecar's `--import` offline mode); this module ONLY resolves the
//! paths, spawns the sidecar, and hands back the tool's OWN JSON report.
//! No SQL here, no schema knowledge here — the shell has no second path to
//! the store's bytes.
//!
//! ## The laws
//!
//! - **The source is named, documented, and fenced.** The import reads the
//!   OpenClaw workspace only (`~/.openclaw/workspace/`, the path the D3
//!   detection already lists by metadata) — a path outside it is refused
//!   before any process runs. The sidecar itself re-refuses anything its
//!   laws reject; this fence is the APP's own, so the webview can never
//!   point the tool at an arbitrary file.
//! - **The target is the app's own kernel store**, resolved the same way
//!   the service core resolves it (`<app_data_dir>/kernel/brain.db`) and
//!   required to EXIST (the tool refuses to create a target — the kernel's
//!   boot owns store creation).
//! - **The report is the tool's own JSON**, byte-for-byte what
//!   `--import` printed on stdout. A nonzero exit carries the state
//!   `refused` and the stderr tail — never a synthesized success.
//! - **Consent lives in the UI flow, the tool enforces it mechanically**:
//!   the report (the default) lands nothing; the commit must carry the
//!   report's own batch id (the tool refuses a stale one); the revert reads
//!   the batch receipt the commit wrote. This module never invents a batch
//!   id and never re-runs a commit the user did not name.
//! - **Bounded wait.** A wedged sidecar is killed at the deadline (the
//!   notarytool wedge lesson); the deadline is generous (a real 8.7k-row
//!   merge measured ~6 s) but finite.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::sidecar;

/// The only directory an import source may live in (the documented OpenClaw
/// workspace the D3 detection lists).
fn workspace_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let home = app
        .path()
        .home_dir()
        .map_err(|error| format!("home_dir: {error}"))?;
    Ok(home.join(".openclaw").join("workspace"))
}

/// Fail-closed fence: the source must be an existing regular file INSIDE the
/// documented workspace. The error strings name the refusal; none quotes the
/// unvalidated input beyond the path itself (the user chose that path).
fn validate_source(app: &tauri::AppHandle, source: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(source);
    if !path.is_absolute() {
        return Err("import_source_not_absolute".to_string());
    }
    let workspace = workspace_dir(app)?;
    let canon_source = path
        .canonicalize()
        .map_err(|_| "import_source_missing".to_string())?;
    let canon_workspace = workspace
        .canonicalize()
        .map_err(|_| "import_workspace_missing".to_string())?;
    if !canon_source.starts_with(&canon_workspace) {
        return Err("import_source_outside_workspace".to_string());
    }
    if !canon_source.is_file() {
        return Err("import_source_not_a_file".to_string());
    }
    Ok(canon_source)
}

/// The import target, resolved from the posture the app can VOUCH for.
///
/// MEASURED (2026-11-03): the shipped resolution hard-coded
/// `<app_data_dir>/kernel/brain.db` — the app posture's store — while the
/// kernel the app attaches to serves wherever ITS operator pointed it (the
/// service core resolves `BRAIN_DATA_ROOT` > `BRAIN_DB_PATH`'s parent > the
/// workspace default). On the operator's own machine the card named a file
/// that did not exist while the kernel served the workspace store: the
/// unserved-store defect, caught fail-closed only because the guessed path
/// happened to be absent. The resolution now follows the posture:
///
/// - `app` — the kernel this app spawns serves `<app_data_dir>/kernel/brain.db`;
/// - `service` — the kernel this app installs serves `<app_data_dir>/service/brain.db`;
/// - `external` (and any unrecognized marker) — REFUSED: a kernel the app
///   did not start keeps its store wherever its operator pointed it, and an
///   app-side guess is the defect itself. The card names the refusal.
///
/// The target must EXIST in the vouched postures — the kernel's boot owns
/// store creation; the import never creates one. A `service` marker whose
/// install root is absent is ALSO unresolved: the marker names a choice,
/// the install names a fact.
pub(crate) fn target_for(posture: &str, app_data_dir: &Path) -> Result<PathBuf, String> {
    let (root, what) = match posture {
        "app" => (app_data_dir.join("kernel"), "app"),
        "service" => {
            let root = app_data_dir.join("service");
            if !root.is_dir() {
                return Err(
                    "import_target_unresolved: the service posture is chosen but this app has \
                     not installed its service — the kernel on the port was not started by this \
                     app, so the app cannot vouch for which store an import would land in"
                        .to_string(),
                );
            }
            (root, "service")
        }
        _ => {
            return Err(
                "import_target_unresolved: the attached kernel was not started by this app — \
                 the app cannot vouch for which store an import would land in"
                    .to_string(),
            );
        }
    };
    let db = root.join("brain.db");
    if !db.is_file() {
        return Err(format!(
            "import_target_missing: the {what} posture's kernel has not booted its store \
             ({}) — the kernel's boot owns store creation; run the kernel first",
            db.display()
        ));
    }
    Ok(db)
}

fn resolve_target(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("app_data_dir: {error}"))?;
    let posture = crate::service::posture_load(app).posture;
    target_for(&posture, &dir)
}

/// The degenerate deployment refuses BY NAME: the fence directory may
/// contain the kernel's own store (the operator's machine serves
/// `~/.openclaw/workspace/brain.db` itself), and importing a store into
/// itself lands nothing while looking like an act. Both paths are compared
/// canonicalized — the source already is; the target exists by law.
pub(crate) fn ensure_distinct(source: &Path, target: &Path) -> Result<(), String> {
    let target_canon = target
        .canonicalize()
        .unwrap_or_else(|_| target.to_path_buf());
    if source == target_canon {
        return Err(
            "import_source_is_target: the chosen store IS the kernel's own — importing it into \
             itself lands nothing; pick one of the workspace's other stores"
                .to_string(),
        );
    }
    Ok(())
}

fn resolve_sidecar() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let sidecar = sidecar::resolve_sidecar_path(&exe);
    if !sidecar.is_file() {
        return Err("import_sidecar_missing".to_string());
    }
    Ok(sidecar)
}

/// What the Settings surface renders. `report` is the tool's OWN JSON
/// (parsed, so the webview reads fields — but never anything this module
/// computed about the store).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    /// "planned" | "committed" | "reverted" | "refused"
    pub state: String,
    /// The tool's own JSON report (null only when the tool produced nothing
    /// parseable — then `stderr_tail` names why).
    pub report: Option<serde_json::Value>,
    /// The process exit code (present when the sidecar ran).
    pub exit_code: Option<i32>,
    /// The last stderr lines (progress/refusal text) — diagnosis only.
    pub stderr_tail: Vec<String>,
}

/// The bounded wait: run the sidecar to completion, cap the wall time.
fn run_sidecar(
    sidecar: &Path,
    args: &[String],
    deadline: Duration,
) -> Result<ImportOutcome, String> {
    let started = Instant::now();
    let mut child = std::process::Command::new(sidecar)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("import_spawn_failed: {error}"))?;

    let mut stderr_lines: Vec<String> = Vec::new();
    let mut stdout_bytes: Vec<u8> = Vec::new();
    loop {
        let elapsed = started.elapsed();
        if elapsed > deadline {
            let _ = child.kill();
            return Err("import_timeout".to_string());
        }
        // Drain pipes without blocking forever: read whatever is ready.
        use std::io::Read;
        if let Some(stdout) = child.stdout.as_mut() {
            let mut buf = [0u8; 8192];
            if let Ok(n) = stdout.read(&mut buf) {
                stdout_bytes.extend_from_slice(&buf[..n]);
            }
        }
        if let Some(stderr) = child.stderr.as_mut() {
            let mut buf = [0u8; 8192];
            if let Ok(n) = stderr.read(&mut buf) {
                let chunk = String::from_utf8_lossy(&buf[..n]);
                for line in chunk.lines() {
                    stderr_lines.push(line.to_string());
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                let state = if exit_code == 0 {
                    // The final argv decides the verb; the report's own
                    // verdict field is the truth the UI renders.
                    match args.iter().position(|a| a == "--revert-batch") {
                        Some(_) => "reverted",
                        None if args.iter().any(|a| a == "--commit-batch") => "committed",
                        None => "planned",
                    }
                } else {
                    "refused"
                };
                let report = if stdout_bytes.is_empty() {
                    None
                } else {
                    serde_json::from_slice(&stdout_bytes).ok()
                };
                stderr_lines.truncate(12);
                return Ok(ImportOutcome {
                    state: state.to_string(),
                    report,
                    exit_code: Some(exit_code),
                    stderr_tail: stderr_lines,
                });
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(error) => return Err(format!("import_wait_failed: {error}")),
        }
    }
}

fn import_argv(
    sidecar: &Path,
    source: Option<&Path>,
    target: &Path,
    commit_batch: Option<&str>,
    revert_batch: Option<&str>,
) -> Vec<String> {
    let mut args = vec![sidecar.display().to_string(), "--import".to_string()];
    if let Some(source) = source {
        args.push("--source".to_string());
        args.push(source.display().to_string());
    }
    args.push("--dest".to_string());
    args.push(target.display().to_string());
    if let Some(batch) = commit_batch {
        args.push("--commit-batch".to_string());
        args.push(batch.to_string());
    }
    if let Some(batch) = revert_batch {
        args.push("--revert-batch".to_string());
        args.push(batch.to_string());
    }
    args
}

const IMPORT_DEADLINE: Duration = Duration::from_secs(300);

/// The report (the default act — lands nothing). `sourcePath` must name a
/// file inside the documented OpenClaw workspace.
pub fn report(app: &tauri::AppHandle, source_path: &str) -> Result<ImportOutcome, String> {
    let source = validate_source(app, source_path)?;
    let target = resolve_target(app)?;
    ensure_distinct(&source, &target)?;
    let sidecar = resolve_sidecar()?;
    let args = import_argv(&sidecar, Some(&source), &target, None, None);
    // argv[0] must not ride as an arg: rebuild without the program name.
    let args = args[1..].to_vec();
    run_sidecar(&sidecar, &args, IMPORT_DEADLINE)
}

/// The commit — the batch id comes from the report the user SAW. The tool
/// refuses a stale id; this layer refuses an empty one before spending a
/// process on it.
pub fn commit(
    app: &tauri::AppHandle,
    source_path: &str,
    batch_id: &str,
) -> Result<ImportOutcome, String> {
    if batch_id.trim().is_empty() {
        return Err("import_batch_required".to_string());
    }
    let source = validate_source(app, source_path)?;
    let target = resolve_target(app)?;
    ensure_distinct(&source, &target)?;
    let sidecar = resolve_sidecar()?;
    let args = import_argv(
        &sidecar,
        Some(&source),
        &target,
        Some(batch_id.trim()),
        None,
    )[1..]
        .to_vec();
    run_sidecar(&sidecar, &args, IMPORT_DEADLINE)
}

/// The reversal — reads only the target (and the receipt the commit wrote).
pub fn revert(app: &tauri::AppHandle, batch_id: &str) -> Result<ImportOutcome, String> {
    if batch_id.trim().is_empty() {
        return Err("import_batch_required".to_string());
    }
    let target = resolve_target(app)?;
    let sidecar = resolve_sidecar()?;
    let args = import_argv(&sidecar, None, &target, None, Some(batch_id.trim()))[1..].to_vec();
    run_sidecar(&sidecar, &args, IMPORT_DEADLINE)
}

/// One candidate the Settings surface offers BY NAME. The FULL path rides
/// the DTO so the webview passes back exactly what the core listed — the
/// UI never reconstructs a path (a guessable home is a second path to the
/// store's bytes).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateDto {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_epoch: u64,
}

/// The detection-side listing the Settings surface offers BY NAME: the
/// workspace's candidate stores (metadata only — names, sizes, mtimes; the
/// contents are the user's). The same documented directory the installer's
/// D3 card reports.
pub fn workspace_candidates(app: &tauri::AppHandle) -> Result<Vec<ImportCandidateDto>, String> {
    let workspace = workspace_dir(app)?;
    let mut out = Vec::new();
    let entries =
        std::fs::read_dir(&workspace).map_err(|_| "import_workspace_missing".to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_db = path.extension().is_some_and(|ext| {
            let ext = ext.to_string_lossy().to_ascii_lowercase();
            ext == "db" || ext == "sqlite" || ext == "sqlite3"
        });
        if !is_db || !path.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let modified_epoch = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        out.push(ImportCandidateDto {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: path.display().to_string(),
            size_bytes: meta.len(),
            modified_epoch,
        });
    }
    out.sort_by_key(|db| std::cmp::Reverse(db.size_bytes));
    Ok(out)
}
