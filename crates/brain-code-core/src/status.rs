//! The status lane (S5): what the index holds, and whether it is honest
//! about freshness. `fresh` or `stale` is decided by the COMMIT alone —
//! `stale` = HEAD ≠ the indexed commit (pin 3's law: mtime never decides).

use std::path::Path;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::error::CodeIndexError;
use crate::ingest::ExtCoverage;
use crate::schema::CodeIndex;
use crate::snapshot;

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub root: String,
    /// The commit the index was built at.
    pub commit: String,
    /// HEAD right now.
    pub head_now: String,
    pub fresh: bool,
    pub index_version: i64,
    pub embed_dim: usize,
    pub embed_profile: String,
    pub files: usize,
    pub symbols: usize,
    pub occurrences: usize,
    pub chunks: usize,
    pub unparsed_files: usize,
    pub files_with_errors: usize,
    pub last_run_status: String,
    pub coverage: Vec<ExtCoverage>,
}

pub fn status(index: &CodeIndex, repo_root: &Path) -> Result<StatusReport, CodeIndexError> {
    let head_now = snapshot::read_head_commit(repo_root)?;
    let head_now_report = head_now.clone();
    let root_string = repo_root.to_string_lossy().into_owned();
    let conn = index.conn();

    let row = conn
        .query_row(
            "SELECT id, head_commit, index_version, embed_dim, embed_profile \
             FROM code_repo WHERE root = ?1",
            [&root_string],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((repo_id, commit, index_version, embed_dim, embed_profile)) = row else {
        return Err(CodeIndexError::Refusal(format!(
            "no index for {} — run `brain code-index ingest` first",
            repo_root.display()
        )));
    };

    let one = |sql: &str, id: i64| -> Result<i64, CodeIndexError> {
        conn.query_row(sql, [id], |r| r.get::<_, i64>(0))
            .map_err(CodeIndexError::from)
    };
    let files = one("SELECT COUNT(*) FROM code_file WHERE repo_id = ?1", repo_id)?;
    let symbols = one(
        "SELECT COUNT(*) FROM code_symbol s JOIN code_file f ON f.id = s.file_id \
         WHERE f.repo_id = ?1",
        repo_id,
    )?;
    let occurrences = one(
        "SELECT COUNT(*) FROM code_occurrence o JOIN code_file f ON f.id = o.file_id \
         WHERE f.repo_id = ?1",
        repo_id,
    )?;
    let chunks = one(
        "SELECT COUNT(*) FROM code_chunk c JOIN code_file f ON f.id = c.file_id \
         WHERE f.repo_id = ?1",
        repo_id,
    )?;
    let files_with_errors = one(
        "SELECT COUNT(*) FROM code_file WHERE repo_id = ?1 AND has_errors = 1",
        repo_id,
    )?;
    let last_run_status: String = conn.query_row(
        "SELECT status FROM code_index_run WHERE repo_id = ?1 ORDER BY id DESC LIMIT 1",
        [repo_id],
        |r| r.get(0),
    )?;
    let unparsed_files: i64 = conn.query_row(
        "SELECT unparsed_files FROM code_index_run WHERE repo_id = ?1 \
         ORDER BY id DESC LIMIT 1",
        [repo_id],
        |r| r.get(0),
    )?;

    // Per-extension coverage from the store, plus the unparsed extensions
    // from the last run's detail (pin 8: the status line states what was
    // seen and what was parsed — the .svelte files are named, never skipped).
    let mut coverage = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT lang, COUNT(*), COALESCE(SUM(parsed), 0), COALESCE(SUM(has_errors), 0) \
             FROM code_file WHERE repo_id = ?1 GROUP BY lang ORDER BY lang",
        )?;
        let rows = stmt.query_map([repo_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        let lang_to_ext = [("rust", "rs"), ("typescript", "ts"), ("tsx", "tsx")];
        for row in rows {
            let (lang, seen, parsed, errors) = row?;
            let ext = lang_to_ext
                .iter()
                .find(|(l, _)| *l == lang)
                .map(|(_, e)| *e)
                .unwrap_or(lang.as_str());
            coverage.push(ExtCoverage {
                ext: ext.to_string(),
                seen: seen as usize,
                parsed: parsed as usize,
                errors: errors as usize,
            });
        }
    }
    let detail: String = conn.query_row(
        "SELECT detail FROM code_index_run WHERE repo_id = ?1 AND status = 'ok' \
         ORDER BY id DESC LIMIT 1",
        [repo_id],
        |r| r.get(0),
    )?;
    let unparsed_map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&detail).unwrap_or_default();
    for (ext, value) in &unparsed_map {
        let seen = value.as_u64().unwrap_or(0) as usize;
        coverage.push(ExtCoverage {
            ext: ext.clone(),
            seen,
            parsed: 0,
            errors: 0,
        });
    }

    let fresh = commit == head_now;
    Ok(StatusReport {
        root: root_string,
        commit,
        head_now: head_now_report,
        fresh,
        index_version,
        embed_dim: embed_dim as usize,
        embed_profile,
        files: files as usize,
        symbols: symbols as usize,
        occurrences: occurrences as usize,
        chunks: chunks as usize,
        unparsed_files: unparsed_files as usize,
        files_with_errors: files_with_errors as usize,
        last_run_status,
        coverage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::fake::FakeEncoder;
    use crate::host::Vec0Host;
    use crate::ingest::ingest;
    use std::path::PathBuf;
    use std::process::Command;

    struct ShimHost;

    impl Vec0Host for ShimHost {
        fn ensure_vec0_registered(&self) -> Result<(), CodeIndexError> {
            brain_code_vec0::register_sqlite_vec();
            Ok(())
        }
    }

    fn fixture_repo(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "brain-code-core-s5-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        };
        run(&["init", "-b", "main"]);
        run(&["add", "-A"]);
        run(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "fixture",
        ]);
        dir
    }

    #[test]
    fn an_index_pinned_to_another_commit_reports_stale_never_fresh() {
        let repo = fixture_repo("stale");
        let path = std::env::temp_dir().join(format!(
            "brain-code-core-s5-stale-index-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let index = CodeIndex::open(&path, &ShimHost).unwrap();
        index.migrate(8).unwrap();
        ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();

        let fresh = status(&index, &repo).unwrap();
        assert!(fresh.fresh, "HEAD == the indexed commit: fresh");
        assert_eq!(fresh.commit, fresh.head_now);
        assert_eq!(fresh.files, 1);

        // Move HEAD: the indexed commit no longer matches — STALE, decided by
        // the commit alone, never by mtime (pin 3's law).
        std::fs::write(repo.join("src/main.rs"), "fn main() {\n    moved();\n}\n").unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success());
        };
        git(&["add", "-A"]);
        git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "move head",
        ]);

        let stale = status(&index, &repo).unwrap();
        assert!(
            !stale.fresh,
            "HEAD moved past the indexed commit: stale, never fresh"
        );
        assert_ne!(stale.commit, stale.head_now);

        // And the ingest after the move reports exactly the movers.
        let report = ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        assert_eq!(report.files_changed, 1);
        assert!(status(&index, &repo).unwrap().fresh);

        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&path);
    }
}
