//! The inventory (S3): the tracked corpus, classified by extension. The
//! tracked list comes from `git ls-files` (a subprocess with in-tree
//! precedent, R-9 — no git library); each indexed file's content hash is
//! sha256 computed in Rust over the working-tree bytes, never git's blob id.
//!
//! Language by extension: `.rs`, `.ts`, `.tsx`. Every other tracked source
//! file is COUNTED as unparsed with its extension — never skipped silently
//! (R-4/P-6: the 58 `.svelte` files are seen and reported, and indexed
//! never). Unparsed files are not read at all: no content, no hash, no row.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::CodeIndexError;
use crate::hashing::sha256_hex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLang {
    Rust,
    TypeScript,
    Tsx,
}

impl FileLang {
    pub fn as_str(self) -> &'static str {
        match self {
            FileLang::Rust => "rust",
            FileLang::TypeScript => "typescript",
            FileLang::Tsx => "tsx",
        }
    }

    pub fn from_extension(ext: &str) -> Option<FileLang> {
        match ext {
            "rs" => Some(FileLang::Rust),
            "ts" => Some(FileLang::TypeScript),
            "tsx" => Some(FileLang::Tsx),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct FileDraft {
    pub path: String,
    pub lang: FileLang,
    pub content_hash: String,
    pub loc: usize,
    pub content: String,
    pub is_generated: bool,
    pub is_test: bool,
    /// True when the file could not be read as UTF-8; it still gets a row
    /// (lossy content) and is counted in `files_with_errors`.
    pub read_error: bool,
}

#[derive(Debug)]
pub struct Inventory {
    pub files: Vec<FileDraft>,
    /// Per-extension counts of tracked files the index does not parse,
    /// extension-less gitlinks included as "(gitlink)". Sorted by extension.
    pub unparsed: Vec<(String, usize)>,
}

/// The tracked file list. A git that is missing or fails is a named refusal —
/// the corpus cannot be derived, so it is not invented.
fn tracked_files(repo_root: &Path) -> Result<Vec<String>, CodeIndexError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|e| {
            CodeIndexError::Refusal(format!(
                "cannot run git under {} ({e}) — ingest refuses (R-9)",
                repo_root.display()
            ))
        })?;
    if !output.status.success() {
        return Err(CodeIndexError::Refusal(format!(
            "git ls-files failed under {} ({}) — ingest refuses (R-9)",
            repo_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
}

fn is_generated(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "schema.d.ts"
        || name.contains(".gen.")
        || path.split('/').any(|segment| segment == "gen")
}

fn is_test(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.ends_with("_test.rs")
        || name.ends_with(".test.ts")
        || name.ends_with(".spec.ts")
        || name.starts_with("test_")
        || path
            .split('/')
            .any(|segment| segment == "tests" || segment == "__tests__")
}

fn loc_of(content: &str) -> usize {
    content.lines().count()
}

/// Classify and read every tracked file. Parsed-language files are read to
/// UTF-8 (a file that cannot be read cleanly keeps a row with lossy content
/// and is counted as a read error); everything else is only counted.
pub fn inventory(repo_root: &Path) -> Result<Inventory, CodeIndexError> {
    let tracked = tracked_files(repo_root)?;
    let mut files = Vec::new();
    let mut unparsed: BTreeMap<String, usize> = BTreeMap::new();
    for path in tracked {
        let ext = path.rsplit('.').next().unwrap_or("").to_string();
        let lang = match FileLang::from_extension(&ext) {
            Some(lang) if path.contains('.') => lang,
            _ => {
                let bucket = if path.contains('.') {
                    ext
                } else {
                    "(gitlink)".to_string()
                };
                *unparsed.entry(bucket).or_insert(0) += 1;
                continue;
            }
        };
        let bytes = fs::read(repo_root.join(&path))?;
        let read_error = String::from_utf8(bytes.clone()).is_err();
        let content = String::from_utf8_lossy(&bytes).into_owned();
        files.push(FileDraft {
            content_hash: sha256_hex(&bytes),
            loc: loc_of(&content),
            content,
            is_generated: is_generated(&path),
            is_test: is_test(&path),
            read_error,
            lang,
            path,
        });
    }
    Ok(Inventory {
        files,
        unparsed: unparsed.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    use std::path::PathBuf;

    #[test]
    fn the_inventory_counts_the_corpus_and_reports_the_unparsed() {
        let inv = inventory(&repo_root()).unwrap();
        let rs = inv
            .files
            .iter()
            .filter(|f| f.lang == FileLang::Rust)
            .count();
        let ts = inv
            .files
            .iter()
            .filter(|f| f.lang == FileLang::TypeScript)
            .count();
        let tsx = inv.files.iter().filter(|f| f.lang == FileLang::Tsx).count();
        // The plan's §0 corpus counts were measured at 5e3c2284 (639/171/58).
        // The corpus can only grow as this round lands code — the exact count
        // is reported per ingest, never hand-typed here.
        assert!(rs >= 639, "rs corpus at least the round-start count: {rs}");
        assert_eq!(ts, 171, "ts count (no ts files landed this round)");
        assert_eq!(tsx, 0, "no tsx exists (recorded finding)");
        let svelte = inv
            .unparsed
            .iter()
            .find(|(ext, _)| ext == "svelte")
            .map(|(_, count)| *count)
            .unwrap_or(0);
        assert_eq!(svelte, 58, "svelte is counted and reported, never skipped");
    }

    #[test]
    fn the_generated_wire_schema_is_marked() {
        let inv = inventory(&repo_root()).unwrap();
        let schema = inv
            .files
            .iter()
            .find(|f| f.path.ends_with("shell/src/lib/api/schema.d.ts"))
            .expect("schema.d.ts is tracked");
        assert!(schema.is_generated, "the generated wire schema is marked");
        assert!(!schema.is_test);
    }

    #[test]
    fn test_files_are_marked() {
        let inv = inventory(&repo_root()).unwrap();
        let tests = inv.files.iter().filter(|f| f.is_test).count();
        assert!(tests > 0, "the repo has tracked test files");
        let sources = inv
            .files
            .iter()
            .filter(|f| f.path.starts_with("src/search/"))
            .count();
        assert!(sources > 0);
    }

    #[test]
    fn hashes_are_stable_across_calls() {
        let inv = inventory(&repo_root()).unwrap();
        let second = inventory(&repo_root()).unwrap();
        let a = inv.files.iter().find(|f| f.path == "src/lib.rs").unwrap();
        let b = second
            .files
            .iter()
            .find(|f| f.path == "src/lib.rs")
            .unwrap();
        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.content_hash.len(), 64);
    }
}
