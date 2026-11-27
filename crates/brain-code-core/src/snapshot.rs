//! The snapshot law (R-9): the commit is read from `.git` files — HEAD, then
//! the ref file, then `packed-refs` — never from a git library, and the
//! change detector is a content hash computed in Rust, not git's blob id.
//! If `.git` cannot be read, ingest refuses and says so; an empty index is
//! never written for an unreadable repo (pin 16).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::CodeIndexError;

/// Resolve the repository's `.git` directory — following the `gitdir:`
/// pointer when `.git` is a worktree file.
fn resolve_git_dir(repo_root: &Path) -> Result<PathBuf, CodeIndexError> {
    let dot = repo_root.join(".git");
    if dot.is_dir() {
        return Ok(dot);
    }
    if dot.is_file() {
        let text = fs::read_to_string(&dot)?;
        if let Some(pointer) = text.trim().strip_prefix("gitdir: ") {
            let pointed = PathBuf::from(pointer);
            return Ok(if pointed.is_absolute() {
                pointed
            } else {
                repo_root.join(pointed)
            });
        }
    }
    Err(CodeIndexError::Refusal(format!(
        "no readable .git under {} — ingest refuses (R-9)",
        repo_root.display()
    )))
}

/// The commit the working tree is pinned to. Detached HEAD (a raw 40-hex
/// hash in `HEAD`) is honored; anything else unreadable is a named refusal.
pub fn read_head_commit(repo_root: &Path) -> Result<String, CodeIndexError> {
    let git_dir = resolve_git_dir(repo_root)?;
    let head_path = git_dir.join("HEAD");
    let head = fs::read_to_string(&head_path).map_err(|_| {
        CodeIndexError::Refusal(format!(
            "cannot read {} — ingest refuses (R-9)",
            head_path.display()
        ))
    })?;
    let head = head.trim_end();
    if let Some(ref_name) = head.strip_prefix("ref: ") {
        let ref_file = git_dir.join(ref_name);
        if let Ok(sha) = fs::read_to_string(&ref_file) {
            return Ok(sha.trim().to_string());
        }
        let packed_path = git_dir.join("packed-refs");
        if let Ok(packed) = fs::read_to_string(&packed_path) {
            for line in packed.lines() {
                if line.starts_with('#') || line.trim().is_empty() {
                    continue;
                }
                if let Some((sha, name)) = line.split_once(' ')
                    && name == ref_name
                {
                    return Ok(sha.to_string());
                }
            }
        }
        Err(CodeIndexError::Refusal(format!(
            "ref {ref_name} not found under {} — ingest refuses (R-9)",
            git_dir.display()
        )))
    } else if head.len() == 40 && head.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(head.to_string())
    } else {
        Err(CodeIndexError::Refusal(format!(
            "unrecognizable HEAD ({head:?}) in {} — ingest refuses (R-9)",
            git_dir.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn the_kernel_repo_reads_its_own_head() {
        // The crate lives at <repo>/crates/brain-code-core.
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let ours = read_head_commit(&repo_root).unwrap();
        let output = Command::new("git")
            .args([
                "-C",
                repo_root.to_string_lossy().as_ref(),
                "rev-parse",
                "HEAD",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let theirs = String::from_utf8(output.stdout).unwrap().trim().to_string();
        assert_eq!(ours, theirs, "the .git reader and git agree on HEAD");
        assert_eq!(ours.len(), 40);
    }

    #[test]
    fn a_directory_without_git_refuses_by_name() {
        let bare =
            std::env::temp_dir().join(format!("brain-code-core-s3-nogit-{}", std::process::id()));
        std::fs::create_dir_all(&bare).unwrap();
        let refused = read_head_commit(&bare);
        match refused {
            Err(CodeIndexError::Refusal(reason)) => {
                assert!(
                    reason.contains(&bare.display().to_string()) && reason.contains("refuses"),
                    "the refusal names the path: {reason}"
                );
            }
            other => panic!("expected a named refusal, got {other:?}"),
        }
        let _ = std::fs::remove_dir(&bare);
    }
}
