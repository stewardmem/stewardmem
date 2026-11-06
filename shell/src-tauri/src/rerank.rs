//! CG-12 — the rerank tier's WRITE path: the 0600 choice marker beside the
//! posture marker, the model staging, and the two owned postures' restart
//! flows. `cfg(not(feature = "mas"))` AS A WHOLE MODULE (the same excision
//! posture as `updater`/`service`/`detect`): the store edition has no
//! marker reader, no staging, and no spawn-env extension — nothing can arm
//! the tier there, so **no reranker download can ever occur in MAS** (P-4,
//! by construction, not by policy). The READ path needs nothing from here:
//! the section's kernel facts come over the typed wire (`kernel-facts.ts`),
//! and the renderer gates the write affordances on `build_channel()`.
//!
//! Laws this module keeps:
//! - **The marker is the CHOICE; the staged files and the restarted kernel
//!   are the REALITY** — the posture-marker law (`service.rs`'s own words)
//!   reused: the marker is written only after the staging can hold it, and
//!   the choice takes effect through a restart, never a live env poke.
//! - **The staging execs the SYSTEM `/usr/bin/curl`, argv only, no shell** —
//!   the `/bin/launchctl` precedent. The crate gains NO HTTP client (the
//!   lib.rs header law); curl is the OS's own transport, SIP-protected.
//! - **Every staged file is fail-closed on its E0-measured size**, and the
//!   ONNX additionally on its sha256 (`/usr/bin/shasum -a 256`, the same
//!   exec law) — a reranker swap IS a model swap; the dir law refuses a
//!   CWD-relative dir kernel-side, and the supply-chain law pins the bytes.
//! - **An external posture is REFUSED** (P-9): the app never becomes
//!   authoritative over a kernel it does not own. The kernel-side bounds
//!   (`top_n` clamp, fail-soft) are the kernel's own; this module only
//!   ever governs kernels the app OWNS (posture A's child, posture B's
//!   agent).
//!
//! The pure decision core ([`enable_plan`]) is hermetic and pinned in
//! `rerank_pins.rs`; the [`AppHandle`] edges are thin executors of the plan.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::service;
use crate::sidecar;

/// The choice marker — beside the posture marker (`posture-v1.json`), in the
/// app's config dir, written 0600 (the consent/posture marker's own law).
pub const RERANK_MARKER: &str = "rerank-v1.json";

/// The model dir UNDER the kernel's data root — the value
/// `BRAIN_RERANK_MODEL_DIR` carries (always absolute: a data root is).
pub const MODEL_DIR_REL: &str = "models/mxbai-rerank-large-v1";

/// The staging source. One URL, one repo, the golden pick's own files.
pub const MODEL_BASE_URL: &str =
    "https://huggingface.co/mixedbread-ai/mxbai-rerank-large-v1/resolve/main";

/// The staged files with their E0-measured sizes (2026-11-06, this machine's
/// download). The staging refuses a file whose bytes do not sum to exactly
/// this — a truncated or substituted fetch never reaches the kernel.
pub const MODEL_FILES: &[(&str, u64)] = &[
    ("onnx/model_quantized.onnx", 642_585_535),
    ("tokenizer.json", 8_649_139),
    ("config.json", 970),
    ("special_tokens_map.json", 970),
    ("tokenizer_config.json", 1_447),
];

/// The ONNX's sha256, measured on the same download. The one file that IS
/// the model — the supply-chain pin, verified after the size gate.
pub const MODEL_ONNX_SHA256: &str =
    "d03bd791409057b78184fa7726effdf5429114a68a7757939aa0d350761f7697";

/// The disclosed one-time download (the section renders this before the act).
pub const MODEL_TOTAL_BYTES: u64 = 651_238_061;

/// The system transport + hasher (the launchctl exec law).
const CURL: &str = "/usr/bin/curl";
const SHASUM: &str = "/usr/bin/shasum";

// ── the pure core (hermetic; the pins live here) ───────────────────────────

/// The marker's whole content. One fact: the CHOICE.
pub fn marker_json(enabled: bool) -> String {
    serde_json::json!({ "enabled": enabled }).to_string()
}

/// The marker parse: `{"enabled": true}` — anything else reads OFF. A
/// corrupt marker is not a choice to honor; the section shows the truth
/// (off) and a re-enable rewrites it.
pub fn marker_enabled(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("enabled").and_then(|e| e.as_bool()))
        .unwrap_or(false)
}

/// The spawn-env decision: `Some` IFF the marker says on. The model dir is
/// ABSOLUTE by construction (a data root is); a relative data root would
/// poison the kernel's dir law, so it is refused here too — fail-closed.
pub fn spawn_env(config_dir: &Path, data_root: &Path) -> Option<sidecar::RerankEnv> {
    if !data_root.is_absolute() {
        return None;
    }
    let raw = fs::read_to_string(config_dir.join(RERANK_MARKER)).ok()?;
    if marker_enabled(&raw) {
        Some(sidecar::RerankEnv {
            model_dir: data_root.join(MODEL_DIR_REL),
        })
    } else {
        None
    }
}

/// The staged-file reality: every file present at exactly its pinned size.
pub fn staging_complete(data_root: &Path) -> bool {
    let dir = data_root.join(MODEL_DIR_REL);
    MODEL_FILES.iter().all(|(rel, expected)| {
        fs::metadata(dir.join(rel))
            .map(|m| m.len() == *expected)
            .unwrap_or(false)
    })
}

/// The enable/disable decision, PURE: what the flow must do for a posture,
/// the marker's current state, and the staged reality. The edges execute the
/// plan; the pins pin it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnablePlan {
    /// P-9: not our kernel, not our choice.
    RefuseExternal,
    /// Reality first (stage), then the marker, then the restart.
    StageMarkerRestart,
    /// The files are already there: marker, then restart.
    MarkerRestart,
    /// Disable is marker + restart — the staged files STAY (no destructive
    /// delete on a flip; the off-switch is one flip forever).
    MarkerOffRestart,
}

pub fn enable_plan(posture: &str, marker_on: bool, staged: bool, enable: bool) -> EnablePlan {
    if posture == "external" {
        return EnablePlan::RefuseExternal;
    }
    match (enable, marker_on) {
        (true, false) if staged => EnablePlan::MarkerRestart,
        (true, false) => EnablePlan::StageMarkerRestart,
        (true, true) => EnablePlan::MarkerRestart, // re-enable over an existing choice: ensure restart
        (false, _) => EnablePlan::MarkerOffRestart,
    }
}

// ── the staging edge (curl, argv only) ─────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageOutcome {
    pub staged: bool,
    pub files: Vec<StageFile>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageFile {
    pub name: String,
    pub bytes: u64,
    pub ok: bool,
}

/// One curl fetch: argv only, no shell, `-L --fail` so an HTTP error never
/// writes a body, `--silent --show-error` so stderr carries the reason.
fn curl_args(url: &str, dest: &Path) -> Vec<String> {
    vec![
        "-L".to_string(),
        "--fail".to_string(),
        "--silent".to_string(),
        "--show-error".to_string(),
        "-o".to_string(),
        dest.display().to_string(),
        url.to_string(),
    ]
}

/// The sha256 of a written file, via `/usr/bin/shasum -a 256` (the exec law;
/// the crate gains no hasher dep). First whitespace token is the digest.
fn sha256_of(path: &Path) -> Result<String, String> {
    let output = std::process::Command::new(SHASUM)
        .arg("-a")
        .arg("256")
        .arg(path)
        .output()
        .map_err(|e| format!("shasum exec failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    stdout
        .split_whitespace()
        .next()
        .map(str::to_string)
        .ok_or_else(|| "shasum produced no digest".to_string())
}

/// The injected fetch: `(url, dest) -> bytes written` — the real edge execs
/// the system curl; the pins write scratch bytes without touching the
/// network (the launchctl runner-injection pattern, `service.rs`).
pub type Fetch<'a> = &'a mut dyn FnMut(&str, &Path) -> Result<u64, String>;

/// Stage the golden model under `<data_root>/models/mxbai-rerank-large-v1/`:
/// per-file download to a `.part` name, size gate, rename; the ONNX then the
/// sha256 gate. Already-complete files are skipped (a resume, not a repeat).
/// The REAL edge: the system `/usr/bin/curl`, argv only, no shell — the
/// `/bin/launchctl` precedent; the crate gains no HTTP client.
pub fn stage_model(data_root: &Path) -> StageOutcome {
    let mut fetch: Fetch = &mut |url: &str, dest: &Path| {
        let output = std::process::Command::new(CURL)
            .args(curl_args(url, dest))
            .output()
            .map_err(|e| format!("curl exec failed: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "curl failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        fs::metadata(dest)
            .map(|m| m.len())
            .map_err(|e| format!("staged bytes unreadable: {e}"))
    };
    stage_model_with(data_root, &mut fetch, &mut sha256_of)
}

/// The staging LOGIC with the fetch injected (the hermetic half; the pins
/// drive it with scratch bytes).
pub fn stage_model_with(
    data_root: &Path,
    fetch: Fetch,
    hasher: &mut dyn FnMut(&Path) -> Result<String, String>,
) -> StageOutcome {
    let dir = data_root.join(MODEL_DIR_REL);
    let mut files = Vec::new();
    if let Err(error) = fs::create_dir_all(dir.join("onnx")) {
        return StageOutcome {
            staged: false,
            files,
            error: Some(format!("model dir create failed: {error}")),
        };
    }
    for (rel, expected) in MODEL_FILES {
        let dest = dir.join(rel);
        if fs::metadata(&dest)
            .map(|m| m.len() == *expected)
            .unwrap_or(false)
        {
            files.push(StageFile {
                name: (*rel).to_string(),
                bytes: *expected,
                ok: true,
            });
            continue;
        }
        let part = dest.with_extension("part");
        let url = format!("{MODEL_BASE_URL}/{rel}");
        let bytes = match fetch(&url, &part) {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = fs::remove_file(&part);
                return StageOutcome {
                    staged: false,
                    files,
                    error: Some(format!("fetch failed for {rel}: {error}")),
                };
            }
        };
        if bytes != *expected {
            let _ = fs::remove_file(&part);
            return StageOutcome {
                staged: false,
                files,
                error: Some(format!(
                    "size gate refused {rel}: got {bytes}, pinned {expected}"
                )),
            };
        }
        if *rel == "onnx/model_quantized.onnx" {
            match hasher(&part) {
                Ok(digest) if digest == MODEL_ONNX_SHA256 => {}
                Ok(digest) => {
                    let _ = fs::remove_file(&part);
                    return StageOutcome {
                        staged: false,
                        files,
                        error: Some(format!(
                            "sha256 gate refused the ONNX: got {digest}, pinned {MODEL_ONNX_SHA256}"
                        )),
                    };
                }
                Err(error) => {
                    let _ = fs::remove_file(&part);
                    return StageOutcome {
                        staged: false,
                        files,
                        error: Some(error),
                    };
                }
            }
        }
        if let Err(error) = fs::rename(&part, &dest) {
            let _ = fs::remove_file(&part);
            return StageOutcome {
                staged: false,
                files,
                error: Some(format!("rename failed for {rel}: {error}")),
            };
        }
        files.push(StageFile {
            name: (*rel).to_string(),
            bytes,
            ok: true,
        });
    }
    StageOutcome {
        staged: true,
        files,
        error: None,
    }
}

// ── the marker edge (0600, beside the posture marker) ──────────────────────

fn marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join(RERANK_MARKER)
}

/// The marker read: the CHOICE, defaulting off when nothing was chosen.
pub fn marker_choice(config_dir: &Path) -> bool {
    fs::read_to_string(marker_path(config_dir))
        .map(|raw| marker_enabled(&raw))
        .unwrap_or(false)
}

/// The marker write, 0600 — the consent/posture marker's own mode law.
pub fn marker_store(config_dir: &Path, enabled: bool) -> Result<(), String> {
    let path = marker_path(config_dir);
    fs::write(&path, marker_json(enabled)).map_err(|e| format!("marker write failed: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("marker chmod failed: {e}"))?;
    }
    Ok(())
}

// ── the DTOs + the AppHandle edges (thin executors of the plan) ────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerankDto {
    /// The CHOICE (the marker).
    pub chosen: bool,
    /// The REALITY beside it: are the staged files complete?
    pub staged: bool,
    /// The disclosed one-time download (the section renders it pre-act).
    pub total_bytes: u64,
    /// The posture this app would act on ('app' | 'service' | 'external').
    pub posture: String,
    pub error: Option<String>,
}

pub fn rerank_posture(app: &tauri::AppHandle) -> RerankDto {
    use tauri::Manager;
    let config_dir = app.path().app_config_dir().ok();
    let data_root = app.path().app_data_dir().map(|d| d.join("kernel")).ok();
    let posture = service::posture_load(app);
    let chosen = config_dir.as_deref().map(marker_choice).unwrap_or(false);
    let staged = data_root.as_deref().map(staging_complete).unwrap_or(false);
    RerankDto {
        chosen,
        staged,
        total_bytes: MODEL_TOTAL_BYTES,
        posture: posture.posture.clone(),
        error: None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RerankOutcomeDto {
    pub outcome: String, // "enabled" | "disabled" | "refused" | "failed"
    pub posture: Option<String>,
    pub detail: Option<String>,
    pub error: Option<String>,
}

/// The WRITE edge: refuse, stage, mark, restart — per posture.
pub fn rerank_set_enabled(
    app: &tauri::AppHandle,
    state: &sidecar::KernelState,
    enable: bool,
) -> RerankOutcomeDto {
    use tauri::Manager;
    let posture_dto = service::posture_load(app);
    let posture = posture_dto.posture.clone();
    let config_dir = match app.path().app_config_dir() {
        Ok(dir) => dir,
        Err(error) => {
            return RerankOutcomeDto {
                outcome: "failed".to_string(),
                posture: Some(posture),
                detail: None,
                error: Some(format!("app_config_dir: {error}")),
            };
        }
    };
    let data_root = match posture.as_str() {
        "service" => app.path().app_data_dir().map(|d| d.join("service")),
        _ => app.path().app_data_dir().map(|d| d.join("kernel")),
    };
    let data_root = match data_root {
        Ok(dir) => dir,
        Err(error) => {
            return RerankOutcomeDto {
                outcome: "failed".to_string(),
                posture: Some(posture),
                detail: None,
                error: Some(format!("app_data_dir: {error}")),
            };
        }
    };

    let marker_on = marker_choice(&config_dir);
    let staged = staging_complete(&data_root);
    match enable_plan(&posture, marker_on, staged, enable) {
        EnablePlan::RefuseExternal => RerankOutcomeDto {
            outcome: "refused".to_string(),
            posture: Some(posture),
            detail: Some(
                "the kernel is managed outside the app — its operator owns the rerank choice"
                    .to_string(),
            ),
            error: None,
        },
        EnablePlan::StageMarkerRestart => {
            let stage = stage_model(&data_root);
            if !stage.staged {
                return RerankOutcomeDto {
                    outcome: "failed".to_string(),
                    posture: Some(posture),
                    detail: Some(format!(
                        "the model staging failed — nothing was armed: {:?}",
                        stage.error
                    )),
                    error: stage.error,
                };
            }
            if let Err(error) = marker_store(&config_dir, true) {
                return RerankOutcomeDto {
                    outcome: "failed".to_string(),
                    posture: Some(posture),
                    detail: Some("the model staged but the choice marker refused the write — nothing is armed".to_string()),
                    error: Some(error),
                };
            }
            restart(&posture, app, state, &config_dir)
        }
        EnablePlan::MarkerRestart | EnablePlan::MarkerOffRestart => {
            if let Err(error) = marker_store(&config_dir, enable) {
                return RerankOutcomeDto {
                    outcome: "failed".to_string(),
                    posture: Some(posture),
                    detail: None,
                    error: Some(error),
                };
            }
            restart(&posture, app, state, &config_dir)
        }
    }
}

/// The restart per owned posture: A = stop + start the child; B = the
/// idempotent install (the `AlreadyInstalled` plist repair) + kickstart.
fn restart(
    posture: &str,
    app: &tauri::AppHandle,
    state: &sidecar::KernelState,
    _config_dir: &Path,
) -> RerankOutcomeDto {
    if posture == "service" {
        // The repair rewrites the plist from the CURRENT marker (the spec
        // read happens inside install); the kickstart makes the running
        // agent re-read it. The DTO carries what actually happened.
        let install = service::service_install(app, state);
        if let Err(error) = service::service_kickstart() {
            return RerankOutcomeDto {
                outcome: "failed".to_string(),
                posture: Some(posture.to_string()),
                detail: Some(format!(
                    "the plist was repaired (install outcome: {}) but the agent restart failed",
                    install.outcome
                )),
                error: Some(error.to_string()),
            };
        }
        return RerankOutcomeDto {
            outcome: "restarted".to_string(),
            posture: Some(posture.to_string()),
            detail: Some(format!(
                "plist repaired ({}), agent kickstarted",
                install.outcome
            )),
            error: None,
        };
    }
    let report = sidecar::shutdown(state);
    let consent = false; // consent was granted long ago — the marker exists; start re-reads it
    let status = sidecar::start(app, state, consent, sidecar::WAIT_MS);
    RerankOutcomeDto {
        outcome: "restarted".to_string(),
        posture: Some(posture.to_string()),
        detail: Some(format!(
            "kernel restarted (kill_error: {:?}; kernel state: {})",
            report.kill_error, status.state
        )),
        error: None,
    }
}
