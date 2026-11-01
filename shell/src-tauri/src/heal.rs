//! The credential self-heal: when the running kernel rejects the shell's
//! bearer, the shell can repair ITSELF — adopt the credential the kernel
//! actually accepts, from the kernel's own on-disk token source, after
//! PROVING the candidate against the live kernel. No dialog, no Settings
//! round-trip: the fault the banner names is gone by the time it renders.
//!
//! ## Why this is lawful here
//!
//! `token.rs` never reads the kernel's `AUTH_TOKEN`/`BRAIN_TOKEN` ENV VARS —
//! that boundary stands, untouched. This module reads the kernel's token
//! FILE at a documented path (the same class of read `detect.rs`'s D1 card
//! already performs on the operator plist) and only when the wire itself
//! named the fault `unauthorized` or `no_credential`. The adoption makes the
//! shell hold, deliberately and visibly (the DTO names the source), the
//! credential the machine's own kernel issued to its own user — both live
//! under the same uid, and the file is owner-only (0600) or it is refused.
//!
//! ## The laws
//!
//! - **Prove before adopt.** A candidate that has not answered 2xx on an
//!   authenticated route is never stored. A candidate the kernel just
//!   rejected is never re-stored.
//! - **The mode law, before the read.** A group/world-readable token file is
//!   skipped and named — never read, never tightened silently (the kernel's
//!   own `read_owner_only_token` shape).
//! - **Wire-safe tokens only.** The kernel's middleware splits the file on
//!   whitespace and compares one bearer; a fragment carrying a control
//!   character can never arrive in an Authorization header. The candidate is
//!   the FIRST whitespace-split token, and only if it is entirely visible
//!   ASCII.
//! - **Nothing is adopted on a non-credential fault.** `unreachable` belongs
//!   to the gate's start path; `origin_not_allowed`/`unexpected` are
//!   configuration facts no credential can repair.
//! - **Fail closed, name everything.** Every skip/refusal carries its code
//!   and path in the report. The keychain write is the LAST step; a heal
//!   that fails leaves the machine exactly as it stood.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::sidecar::{self, WireOutcome};

/// The one HTTP fact a candidate must produce.
pub type ProveFn<'a> = dyn Fn(&str) -> WireOutcome + 'a;

/// Why one candidate path produced no credential. Every variant names the
/// path it happened at; none carries token material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealSkip {
    /// The path does not exist — a source this machine does not have.
    Absent,
    /// The path exists but is not a regular file (a symlink, a directory).
    NotRegularFile,
    /// The file is group/world-readable: the mode law refuses the READ, not
    /// the heal. Named so the operator can fix the mode.
    InsecureMode { mode: u32 },
    /// The file could not be read (permissions, I/O).
    Unreadable,
    /// The file holds no token at all.
    Empty,
    /// No whitespace-split fragment of the file is usable over the wire —
    /// every fragment carried a control character or was empty.
    NotWireSafe,
    /// The candidate is byte-identical to the bearer the kernel just
    /// rejected; adopting it again cannot heal anything.
    AlreadyRejected,
    /// The candidate was PROVEN against the kernel — but the keychain write
    /// failed. The machine is unchanged; the fact is named.
    KeychainRefused,
}

impl HealSkip {
    /// The stable code the report carries.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::NotRegularFile => "not_regular_file",
            Self::InsecureMode { .. } => "insecure_mode",
            Self::Unreadable => "unreadable",
            Self::Empty => "empty",
            Self::NotWireSafe => "not_wire_safe",
            Self::AlreadyRejected => "already_rejected",
            Self::KeychainRefused => "keychain_refused",
        }
    }
}

/// Why one candidate path produced no credential. Every variant names the
/// The heal's verdict, before the DTO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealReport {
    /// The current bearer already works: nothing to do, nothing touched.
    NotNeeded,
    /// Nothing is listening — the gate's start path owns this, not the heal.
    KernelDown,
    /// The wire named a fault no credential can repair (origin, protocol).
    NotCredentialFault { kind: String },
    /// A candidate was proven and adopted into the keychain.
    Healed { source: PathBuf },
    /// Every candidate failed, each for its own named reason.
    NoSource { tried: Vec<(PathBuf, HealSkip)> },
}

impl HealReport {
    /// The DTO's `outcome` string.
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::KernelDown => "kernel_down",
            Self::NotCredentialFault { .. } => "not_credential_fault",
            Self::Healed { .. } => "healed",
            Self::NoSource { .. } => "no_source",
        }
    }
}

/// The first whitespace-split fragment of `text`, when it exists and is
/// wire-safe: entirely visible ASCII (0x21..=0x7E). This mirrors the
/// kernel's own law — the auth middleware splits the token file on
/// whitespace and compares ONE bearer — so the fragment the shell adopts is
/// exactly a token the kernel may hold, and the fragment can travel in an
/// Authorization header without corrupting the request framing.
pub fn first_wire_safe_token(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    if token.is_empty() {
        return None;
    }
    if !token.chars().all(|c| ('\u{21}'..='\u{7e}').contains(&c)) {
        return None;
    }
    Some(token.to_string())
}

/// One candidate source, read under the mode law. `Ok(token)` is a wire-safe
/// fragment; every `Err` names its reason and never carries content.
pub fn read_candidate(path: &Path) -> Result<String, HealSkip> {
    use std::io::Read;

    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HealSkip::Absent);
        }
        Err(_) => return Err(HealSkip::Unreadable),
    };
    if !metadata.is_file() {
        return Err(HealSkip::NotRegularFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(HealSkip::InsecureMode { mode });
        }
    }
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return Err(HealSkip::Unreadable),
    };
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return Err(HealSkip::Unreadable);
    }
    first_wire_safe_token(&text).ok_or(HealSkip::NotWireSafe)
}

/// The self-heal, pure and injected: the probe and the keychain write arrive
/// as functions so every pin drives this without a real kernel or keychain.
///
/// Order is the law: diagnose FIRST (a working wire is never touched), then
/// per candidate — read under the mode law, skip the fragment the kernel
/// just rejected, PROVE against the live kernel, and only then write.
pub fn heal(
    current_bearer: Option<&str>,
    candidates: &[PathBuf],
    probe: &ProveFn<'_>,
    keychain_write: &mut dyn FnMut(&str) -> Result<(), ()>,
) -> HealReport {
    // 1. The diagnosis: is this even a credential fault?
    let outcome = probe(current_bearer.unwrap_or(""));
    match outcome {
        WireOutcome::Reachable => return HealReport::NotNeeded,
        WireOutcome::Unreachable => return HealReport::KernelDown,
        WireOutcome::OriginNotAllowed { .. } | WireOutcome::Unexpected { .. } => {
            return HealReport::NotCredentialFault {
                kind: outcome.kind().to_string(),
            };
        }
        // The two faults a credential can repair.
        WireOutcome::NoCredential | WireOutcome::Unauthorized => {}
    }

    // 2. The candidates, in order, each proven before it is adopted.
    let mut tried: Vec<(PathBuf, HealSkip)> = Vec::new();
    for path in candidates {
        let token = match read_candidate(path) {
            Ok(token) => token,
            Err(skip) => {
                tried.push((path.clone(), skip));
                continue;
            }
        };
        if Some(token.as_str()) == current_bearer {
            tried.push((path.clone(), HealSkip::AlreadyRejected));
            continue;
        }
        if !matches!(probe(&token), WireOutcome::Reachable) {
            tried.push((path.clone(), HealSkip::AlreadyRejected));
            continue;
        }
        match keychain_write(&token) {
            Ok(()) => {
                return HealReport::Healed {
                    source: path.clone(),
                };
            }
            Err(()) => {
                tried.push((path.clone(), HealSkip::KeychainRefused));
                continue;
            }
        }
    }
    HealReport::NoSource { tried }
}

/// The DTO the renderer sees. `tried` carries paths + stable codes — the
/// evidence law — and never token material.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealDto {
    pub outcome: String,
    pub source: Option<String>,
    pub fault: Option<String>,
    pub tried: Vec<TriedDto>,
}

/// One tried candidate: the path and why it produced nothing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriedDto {
    pub path: String,
    pub code: String,
}

impl HealDto {
    pub(crate) fn from_report(report: &HealReport) -> Self {
        match report {
            HealReport::NotNeeded | HealReport::KernelDown => Self {
                outcome: report.outcome().to_string(),
                source: None,
                fault: None,
                tried: Vec::new(),
            },
            HealReport::NotCredentialFault { kind } => Self {
                outcome: report.outcome().to_string(),
                source: None,
                fault: Some(kind.clone()),
                tried: Vec::new(),
            },
            HealReport::Healed { source } => Self {
                outcome: report.outcome().to_string(),
                source: Some(source.display().to_string()),
                fault: None,
                tried: Vec::new(),
            },
            HealReport::NoSource { tried } => Self {
                outcome: report.outcome().to_string(),
                source: None,
                fault: None,
                tried: tried
                    .iter()
                    .map(|(path, skip)| TriedDto {
                        path: path.display().to_string(),
                        code: skip.code().to_string(),
                    })
                    .collect(),
            },
        }
    }
}

/// The candidate order, resolved once: OUR posture-A data root, OUR posture-B
/// service root, the operator kernel's DECLARED token file (the D1 plist's
/// `EnvironmentVariables.AUTH_TOKEN_FILE`), the documented default. Deduped,
/// preserving order.
pub fn candidate_paths(data_root: &Path, service_root: &Path, home: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut push = |path: PathBuf| {
        if !paths.contains(&path) {
            paths.push(path);
        }
    };
    push(data_root.join(sidecar::AUTH_FILE));
    push(service_root.join(sidecar::AUTH_FILE));
    if let Some(declared) = crate::detect::operator_auth_token_file(home) {
        push(declared);
    }
    push(home.join(".config").join("brain-server").join("auth-token"));
    paths
}

/// The Tauri edge: resolve the machine's paths, probe with the shell's
/// current bearer, prove candidates against the LIVE kernel, and adopt.
/// Blocking fs + loopback IO — runs on the blocking pool at the call site.
pub fn heal_running(
    data_root: &Path,
    service_root: &Path,
    home: &Path,
    current_bearer: Option<&str>,
) -> HealDto {
    let candidates = candidate_paths(data_root, service_root, home);
    let probe = |bearer: &str| -> WireOutcome {
        sidecar::probe_wire(
            sidecar::PORT,
            Duration::from_millis(sidecar::PROBE_TIMEOUT_MS),
            sidecar::APP_ORIGIN,
            if bearer.is_empty() {
                None
            } else {
                Some(bearer)
            },
        )
    };
    let mut keychain_write =
        |token: &str| -> Result<(), ()> { crate::token::write_platform(token).map_err(|_| ()) };
    let report = heal(current_bearer, &candidates, &probe, &mut keychain_write);
    HealDto::from_report(&report)
}
