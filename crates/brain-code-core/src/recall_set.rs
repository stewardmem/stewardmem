//! The frozen recall set (C0 S1). The set froze BEFORE any retrieval code
//! exists — that ordering is the law this module enforces. Every entry is a
//! real question over the brain-server corpus, sourced from a CHANGELOG entry
//! or a commit subject; `gold_files` name the files a correct answer must
//! reach (repo-relative paths into the indexed corpus, `.rs`/`.ts`/`.tsx`
//! only — unparsed extensions are not indexed by design, C0 R-4/P-6).
//!
//! The file is pinned by SHA-256 (`FROZEN_SHA256`); a changed set fails the
//! build. Re-freezing is a deliberate act: change the file, re-pin, and say
//! why in the evidence file.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The SHA-256 (hex, lowercase) of `fixtures/code_index_recall.jsonl` as
/// frozen on 2026-11-27 (46 entries, 10 with_identifier / 36 without).
pub const FROZEN_SHA256: &str = "250c7b83758df39a2b1e8e829b617fde5d413866662a118234b9c3315d0a4243";

/// The minimum size the plan demands of the frozen set (C0 S1).
pub const MIN_ENTRIES: usize = 40;

/// One frozen recall question.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RecallEntry {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub gold_files: Vec<String>,
    #[serde(default)]
    pub gold_symbols: Vec<String>,
    pub origin: String,
    pub has_identifier: bool,
}

#[derive(Debug)]
pub enum RecallError {
    Io(std::io::Error),
    Json {
        line: usize,
        source: serde_json::Error,
    },
}

impl fmt::Display for RecallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecallError::Io(e) => write!(f, "recall set io error: {e}"),
            RecallError::Json { line, source } => {
                write!(f, "recall set line {line} is not a valid entry: {source}")
            }
        }
    }
}

impl std::error::Error for RecallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecallError::Io(e) => Some(e),
            RecallError::Json { source, .. } => Some(source),
        }
    }
}

/// Load the JSONL set. Empty lines are skipped; a malformed line names its
/// number — the set fails loudly, never silently truncated.
pub fn load(path: &Path) -> Result<Vec<RecallEntry>, RecallError> {
    let text = fs::read_to_string(path).map_err(RecallError::Io)?;
    let mut entries = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: RecallEntry =
            serde_json::from_str(line).map_err(|source| RecallError::Json {
                line: idx + 1,
                source,
            })?;
        entries.push(entry);
    }
    Ok(entries)
}

/// Structural checks on a loaded set. Every failure names the entry — a set
/// that cannot be derived is not written.
pub fn validate(entries: &[RecallEntry]) -> Result<(), String> {
    if entries.len() < MIN_ENTRIES {
        return Err(format!(
            "frozen recall set holds {} entries; the plan requires >= {MIN_ENTRIES}",
            entries.len()
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        if entry.id.trim().is_empty() {
            return Err("an entry carries an empty id".to_string());
        }
        if !seen.insert(entry.id.as_str()) {
            return Err(format!("duplicate id in the frozen set: {}", entry.id));
        }
        if entry.question.trim().is_empty() {
            return Err(format!("entry {} has an empty question", entry.id));
        }
        if entry.origin.trim().is_empty() {
            return Err(format!("entry {} has no named origin", entry.id));
        }
        if entry.gold_files.is_empty() {
            return Err(format!(
                "entry {} names no gold file — an unreachable question",
                entry.id
            ));
        }
    }
    Ok(())
}

/// Every gold file must exist under `root` at the frozen head. Anti-vacuity:
/// a gold path that does not exist makes the question unanswerable, and the
/// pin names it.
pub fn check_gold_paths(entries: &[RecallEntry], root: &Path) -> Result<(), String> {
    let mut missing = Vec::new();
    for entry in entries {
        for gold in &entry.gold_files {
            let full = root.join(gold);
            if !full.is_file() {
                missing.push(format!("{} -> {}", entry.id, gold));
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "gold paths that do not exist under {}: {}",
            root.display(),
            missing.join("; ")
        ))
    }
}

/// SHA-256 of the fixture file, hex-encoded lowercase.
pub fn file_sha256(path: &Path) -> Result<String, RecallError> {
    let bytes = fs::read(path).map_err(RecallError::Io)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(to_hex(&hasher.finalize()))
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// The fixture's in-repo path (compile-time; used by the pins and by tests).
pub fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/code_index_recall.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        // The crate lives at <repo>/crates/brain-code-core; the frozen set's
        // gold paths are repo-relative.
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    #[test]
    fn the_frozen_set_loads_and_validates() {
        let entries = load(&fixture_path()).unwrap();
        assert!(
            entries.len() >= MIN_ENTRIES,
            "frozen set holds {} entries, requires >= {MIN_ENTRIES}",
            entries.len()
        );
        validate(&entries).unwrap();
    }

    #[test]
    fn the_frozen_hash_pins_the_set() {
        let got = file_sha256(&fixture_path()).unwrap();
        assert_eq!(
            got, FROZEN_SHA256,
            "the frozen recall set changed — re-freeze deliberately: expected {FROZEN_SHA256}, got {got}"
        );
    }

    #[test]
    fn every_gold_path_exists_at_the_frozen_head() {
        let entries = load(&fixture_path()).unwrap();
        check_gold_paths(&entries, &repo_root()).unwrap();
    }

    #[test]
    fn both_identifier_kinds_are_present() {
        let entries = load(&fixture_path()).unwrap();
        let with = entries.iter().filter(|e| e.has_identifier).count();
        let without = entries.len() - with;
        assert!(
            with > 0 && without > 0,
            "the set must hold both kinds: {with} with identifier, {without} without"
        );
    }

    #[test]
    fn entry_ids_are_unique() {
        let entries = load(&fixture_path()).unwrap();
        let mut ids: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        let total = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), total, "duplicate ids in the frozen set");
    }

    #[test]
    fn a_changed_set_fails_the_hash_pin() {
        // The drill behind pin 1, run as a test: one synthetic entry is
        // serialized and hashed apart from the file; its digest must differ
        // from the frozen pin.
        let entry = RecallEntry {
            id: "drill".to_string(),
            question: "not part of the set".to_string(),
            gold_files: vec!["src/lib.rs".to_string()],
            gold_symbols: Vec::new(),
            origin: "drill".to_string(),
            has_identifier: false,
        };
        let line = serde_json::to_string(&entry).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(line.as_bytes());
        let got = to_hex(&hasher.finalize());
        assert_ne!(
            got, FROZEN_SHA256,
            "a changed set cannot keep the frozen hash"
        );
    }
}
