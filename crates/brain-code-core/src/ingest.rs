//! The ingest (S3): one transaction per ingest, or nothing (pin 4). The run
//! row is the record (P-1's signed mitigation): a `running` row commits
//! first, the data transaction finalizes it to `ok` atomically with the
//! data, and a failure rolls the data back and records `failed` — no
//! partial symbols, ever. The commit is read BEFORE the index file exists,
//! so an unreadable `.git` never creates an index (pin 16).

use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::chunk;
use crate::encoder::Encoder;
use crate::error::CodeIndexError;
use crate::inventory::{self, Inventory};
use crate::parse;
use crate::schema::{CodeIndex, INDEX_VERSION};
use crate::snapshot;

#[derive(Debug, Clone, Serialize)]
pub struct ExtCoverage {
    pub ext: String,
    pub seen: usize,
    pub parsed: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestReport {
    pub commit: String,
    pub files_indexed: usize,
    /// Files rebuilt this run — changed or removed. Zero on an unchanged
    /// re-ingest (pin 5), exactly the movers on a change (pin 6).
    pub files_changed: usize,
    pub symbols: usize,
    pub occurrences: usize,
    pub chunks: usize,
    pub unparsed_files: usize,
    pub files_with_errors: usize,
    pub coverage: Vec<ExtCoverage>,
}

/// The test seam for pin 4: a hook called after each file's rows are written.
/// Returning an error must roll the whole data transaction back.
pub type FileHook<'a> = Option<&'a dyn Fn(usize) -> Result<(), CodeIndexError>>;

/// One repo per index file: the `code_repo` row must already carry the same
/// profile, or the ingest refuses naming both (pin 14's mechanism; P-5).
fn check_profile(
    index: &CodeIndex,
    root: &str,
    embed_dim: usize,
    embed_profile: &str,
) -> Result<(), CodeIndexError> {
    let row = index
        .conn()
        .query_row(
            "SELECT embed_dim, embed_profile FROM code_repo WHERE root = ?1",
            [root],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((stored_dim, stored_profile)) = row
        && (stored_dim != embed_dim as i64 || stored_profile != embed_profile)
    {
        return Err(CodeIndexError::Refusal(format!(
            "profile change refused until rebuilt: index holds dim {stored_dim} profile \
             '{stored_profile}', ingest carries dim {embed_dim} profile '{embed_profile}'"
        )));
    }
    Ok(())
}

pub fn ingest(
    index: &CodeIndex,
    repo_root: &Path,
    encoder: &dyn Encoder,
    hook: FileHook<'_>,
) -> Result<IngestReport, CodeIndexError> {
    // 1. The commit, before anything exists (pin 16).
    let commit = snapshot::read_head_commit(repo_root)?;
    let commit_for_report = commit.clone();
    let root_string = repo_root.to_string_lossy().into_owned();

    // 2. The corpus, classified. A git that fails is a named refusal.
    let inv = inventory::inventory(repo_root)?;

    // 3. The profile recorded by the port must agree with anything already
    //    in the store (pin 14; P-5: the port, never a literal).
    let embed_dim = encoder.dim();
    let embed_profile = encoder.profile().to_string();
    check_profile(index, &root_string, embed_dim, &embed_profile)?;

    // 4. TX1: the repo row + the running row. This is the record.
    let tx = index.conn().unchecked_transaction()?;
    tx.execute(
        "INSERT INTO code_repo(root, head_commit, index_version, embed_dim, embed_profile)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(root) DO UPDATE SET
           head_commit = excluded.head_commit,
           index_version = excluded.index_version,
           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
        rusqlite::params![
            root_string,
            commit,
            INDEX_VERSION,
            embed_dim as i64,
            embed_profile
        ],
    )?;
    let repo_id: i64 = tx.query_row(
        "SELECT id FROM code_repo WHERE root = ?1",
        [&root_string],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO code_index_run(repo_id, head_commit, status) VALUES (?1, ?2, 'running')",
        rusqlite::params![repo_id, commit],
    )?;
    let run_id: i64 = tx.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?;
    tx.commit()?;

    // 5. TX2: all data, atomically — counts verified INSIDE the transaction,
    //    before the commit. Any failure rolls back to an empty store; the
    //    run row is then finalized as failed in its own transaction.
    let result = ingest_data(index, repo_id, run_id, &inv, encoder, hook);

    match result {
        Ok(report) => {
            let tx = index.conn().unchecked_transaction()?;
            tx.execute(
                "UPDATE code_index_run SET status = 'ok', finished = \
                   strftime('%Y-%m-%dT%H:%M:%fZ','now'), files = ?2, symbols = ?3, \
                   occurrences = ?4, chunks = ?5, unparsed_files = ?6, files_with_errors = ?7, \
                   detail = ?8
                 WHERE id = ?1",
                rusqlite::params![
                    run_id,
                    report.files_indexed as i64,
                    report.symbols as i64,
                    report.occurrences as i64,
                    report.chunks as i64,
                    report.unparsed_files as i64,
                    report.files_with_errors as i64,
                    unparsed_detail(&report.coverage),
                ],
            )?;
            tx.commit()?;
            Ok(IngestReport {
                commit: commit_for_report,
                ..report
            })
        }
        Err(err) => {
            // The data rolled back; the failed run is still the record.
            let tx = index.conn().unchecked_transaction()?;
            tx.execute(
                "UPDATE code_index_run SET status = 'failed', finished = \
                   strftime('%Y-%m-%dT%H:%M:%fZ','now'), detail = ?2 WHERE id = ?1",
                rusqlite::params![run_id, err.to_string()],
            )?;
            tx.commit()?;
            Err(err)
        }
    }
}

fn unparsed_detail(coverage: &[ExtCoverage]) -> String {
    let unparsed: serde_json::Map<String, serde_json::Value> = coverage
        .iter()
        .filter(|c| c.parsed == 0)
        .map(|c| (c.ext.clone(), serde_json::Value::from(c.seen)))
        .collect();
    serde_json::Value::Object(unparsed).to_string()
}

fn ingest_data(
    index: &CodeIndex,
    repo_id: i64,
    run_id: i64,
    inv: &Inventory,
    encoder: &dyn Encoder,
    hook: FileHook<'_>,
) -> Result<IngestReport, CodeIndexError> {
    parse::assert_grammar_abis_in_range()?;
    let conn = index.conn();
    let tx = conn.unchecked_transaction()?;

    // The incremental rule (S5): reparse exactly the files whose content_hash
    // moved; delete and rebuild only those files' rows (plus the tracked files
    // that disappeared). The vec table has no FK to the chunk rows (R-6:
    // chunk_id only), so its stale rows go FIRST, while the doomed chunk ids
    // are still findable. The content hash — never mtime (D-g) — is the
    // change detector, so an unchanged re-ingest writes zero data rows (pin 5).
    let stored: BTreeMap<String, (i64, String)> = {
        let mut stmt =
            tx.prepare("SELECT path, id, content_hash FROM code_file WHERE repo_id = ?1")?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (row.get::<_, i64>(1)?, row.get::<_, String>(2)?),
            ))
        })?;
        rows.collect::<Result<BTreeMap<String, (i64, String)>, rusqlite::Error>>()?
    };
    let mut doomed_ids: Vec<i64> = Vec::new();
    for (path, (id, hash)) in &stored {
        let still_tracked = inv.files.iter().any(|f| &f.path == path);
        let moved = inv
            .files
            .iter()
            .any(|f| &f.path == path && &f.content_hash != hash);
        if !still_tracked || moved {
            doomed_ids.push(*id);
        }
    }
    // The movers: files that are new or whose content_hash moved — exactly
    // the set this ingest (re)parses, so the report's changed-file figure
    // counts every file it touched, including a fresh store's full build.
    let changed_paths: std::collections::BTreeSet<&str> = inv
        .files
        .iter()
        .filter(|f| {
            stored
                .get(&f.path)
                .is_none_or(|(_, stored_hash)| stored_hash != &f.content_hash)
        })
        .map(|f| f.path.as_str())
        .collect();
    let files_changed = changed_paths.len();
    // The doomed files' contribution to the totals, read before deletion —
    // the totals stay independently derived (pin 12 stays honest).
    let mut removed = (0i64, 0i64, 0i64, 0i64); // symbols, occurrences, chunks, error-files
    for id in &doomed_ids {
        removed.0 += tx.query_row(
            "SELECT COUNT(*) FROM code_symbol WHERE file_id = ?1",
            [id],
            |r| r.get::<_, i64>(0),
        )?;
        removed.1 += tx.query_row(
            "SELECT COUNT(*) FROM code_occurrence WHERE file_id = ?1",
            [id],
            |r| r.get::<_, i64>(0),
        )?;
        removed.2 += tx.query_row(
            "SELECT COUNT(*) FROM code_chunk WHERE file_id = ?1",
            [id],
            |r| r.get::<_, i64>(0),
        )?;
        removed.3 += tx.query_row(
            "SELECT has_errors FROM code_file WHERE id = ?1",
            [id],
            |r| r.get::<_, i64>(0),
        )?;
    }
    let before = (
        tx.query_row("SELECT COUNT(*) FROM code_symbol", [], |r| {
            r.get::<_, i64>(0)
        })?,
        tx.query_row("SELECT COUNT(*) FROM code_occurrence", [], |r| {
            r.get::<_, i64>(0)
        })?,
        tx.query_row("SELECT COUNT(*) FROM code_chunk", [], |r| {
            r.get::<_, i64>(0)
        })?,
        tx.query_row(
            "SELECT COUNT(*) FROM code_file WHERE has_errors = 1",
            [],
            |r| r.get::<_, i64>(0),
        )?,
    );
    for id in &doomed_ids {
        tx.execute(
            "DELETE FROM code_chunk_vec WHERE chunk_id IN (SELECT id FROM code_chunk \
             WHERE file_id = ?1)",
            [id],
        )?;
        tx.execute("DELETE FROM code_file WHERE id = ?1", [id])?;
    }

    let mut inserted_symbols = 0usize;
    let mut inserted_occurrences = 0usize;
    let mut inserted_chunks = 0usize;
    let mut inserted_error_files = 0usize;
    // (chunk row id, chunk text) — embedded in one port pass after all files.
    let mut to_embed: Vec<(i64, String)> = Vec::new();

    for (file_index, draft) in inv.files.iter().enumerate() {
        // Unchanged since the last ingest: the rows stay, nothing is written
        // (pin 5's no-op law; the content hash — never mtime — decides).
        if !changed_paths.contains(draft.path.as_str()) {
            continue;
        }

        let parsed_file = if draft.is_generated || draft.read_error {
            // Generated files are indexed for their text (the file-level
            // lexical leg still finds them) but never yield symbol work
            // (pin 9); a file that cannot be read as UTF-8 is a named error
            // and is not parsed at all.
            None
        } else {
            Some(parse::parse(draft.lang, &draft.content)?)
        };
        let has_errors = draft.read_error || parsed_file.as_ref().is_some_and(|p| p.has_error);

        tx.execute(
            "INSERT INTO code_file(repo_id, path, lang, content_hash, loc, content, \
               is_generated, is_test, has_errors, parsed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                repo_id,
                draft.path,
                draft.lang.as_str(),
                draft.content_hash,
                draft.loc as i64,
                draft.content,
                draft.is_generated as i64,
                draft.is_test as i64,
                has_errors as i64,
                parsed_file.is_some() as i64,
            ],
        )?;
        let file_id: i64 = tx.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?;

        if let Some(parsed) = &parsed_file {
            let mut symbol_ids: Vec<i64> = Vec::with_capacity(parsed.symbols.len());
            for symbol in &parsed.symbols {
                let parent_id = symbol
                    .parent
                    .and_then(|parent_index| symbol_ids.get(parent_index).copied());
                tx.execute(
                    "INSERT INTO code_symbol(file_id, kind, name, qualified_name, signature, \
                       start_line, end_line, start_byte, end_byte, parent_id, visibility)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        file_id,
                        symbol.kind,
                        symbol.name,
                        symbol.qualified_name,
                        symbol.signature,
                        symbol.start_line as i64,
                        symbol.end_line as i64,
                        symbol.start_byte as i64,
                        symbol.end_byte as i64,
                        parent_id,
                        symbol.visibility,
                    ],
                )?;
                symbol_ids.push(tx.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?);
            }
            for (index, symbol) in parsed.symbols.iter().enumerate() {
                tx.execute(
                    "INSERT INTO code_occurrence(file_id, symbol_id, role, name, start_line, \
                       start_col, end_line, end_col)
                     VALUES (?1, ?2, 'def', ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        file_id,
                        symbol_ids.get(index).copied(),
                        symbol.name,
                        symbol.def_start_line as i64,
                        symbol.def_start_col as i64,
                        symbol.def_end_line as i64,
                        symbol.def_end_col as i64,
                    ],
                )?;
            }
            for reference in &parsed.refs {
                let symbol_id = reference
                    .symbol_index
                    .and_then(|index| symbol_ids.get(index).copied());
                tx.execute(
                    "INSERT INTO code_occurrence(file_id, symbol_id, role, name, start_line, \
                       start_col, end_line, end_col)
                     VALUES (?1, ?2, 'ref', ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        file_id,
                        symbol_id,
                        reference.name,
                        reference.start_line as i64,
                        reference.start_col as i64,
                        reference.end_line as i64,
                        reference.end_col as i64,
                    ],
                )?;
            }
            inserted_symbols += parsed.symbols.len();
            inserted_occurrences += parsed.symbols.len() + parsed.refs.len();

            // Chunk by symbol; windowed chunks only for top-level code with
            // no symbol, tagged `window` (§2 S4).
            for draft in chunk::chunks_for(&draft.content, parsed) {
                let symbol_id = draft
                    .symbol_index
                    .and_then(|index| symbol_ids.get(index).copied());
                tx.execute(
                    "INSERT INTO code_chunk(file_id, symbol_id, kind, text, token_estimate, \
                       content_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        file_id,
                        symbol_id,
                        draft.kind,
                        draft.text,
                        draft.token_estimate as i64,
                        draft.content_hash,
                    ],
                )?;
                let chunk_id: i64 =
                    tx.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))?;
                to_embed.push((chunk_id, draft.text));
                inserted_chunks += 1;
            }
        }
        if has_errors {
            inserted_error_files += 1;
        }
        if let Some(hook) = hook {
            hook(file_index)?;
        }
    }

    // Vectors through the port (P-5): one embed pass over every chunk, then
    // the vec rows — f32 little-endian blobs, the compact exact form.
    if !to_embed.is_empty() {
        let texts: Vec<String> = to_embed.iter().map(|(_, text)| text.clone()).collect();
        let vectors = encoder.embed(&texts)?;
        if vectors.len() != to_embed.len() {
            return Err(CodeIndexError::Refusal(format!(
                "the encoder returned {} vectors for {} chunks — refusing the mismatch",
                vectors.len(),
                to_embed.len()
            )));
        }
        for ((chunk_id, _), vector) in to_embed.iter().zip(vectors) {
            if vector.len() != encoder.dim() {
                return Err(CodeIndexError::Refusal(format!(
                    "a vector of width {} left the port; the port declared dim {}",
                    vector.len(),
                    encoder.dim()
                )));
            }
            let mut blob = Vec::with_capacity(vector.len() * 4);
            for value in &vector {
                blob.extend_from_slice(&value.to_le_bytes());
            }
            tx.execute(
                "INSERT INTO code_chunk_vec(chunk_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![chunk_id, blob],
            )?;
        }
    }

    // The coverage table: per extension, seen / parsed / with errors, derived
    // from the store itself (the `parsed` column keeps the fact on the row),
    // plus the extensions the index never parses (R-4/P-6) from the
    // inventory. The schema's language names map to the table's extensions.
    let mut coverage: Vec<ExtCoverage> = Vec::new();
    for (ext, lang_key) in [("rs", "rust"), ("ts", "typescript"), ("tsx", "tsx")] {
        let row = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(parsed), 0), COALESCE(SUM(has_errors), 0) \
             FROM code_file WHERE repo_id = ?1 AND lang = ?2",
            rusqlite::params![repo_id, lang_key],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )?;
        coverage.push(ExtCoverage {
            ext: ext.to_string(),
            seen: row.0 as usize,
            parsed: row.1 as usize,
            errors: row.2 as usize,
        });
    }
    for (ext, count) in &inv.unparsed {
        coverage.push(ExtCoverage {
            ext: ext.clone(),
            seen: *count,
            parsed: 0,
            errors: 0,
        });
    }

    // Totals for the run row: the store's post-write state, derived
    // independently of the verification SELECTs below (pin 12 stays honest):
    // before − removed + inserted.
    let report = IngestReport {
        commit: String::new(),
        files_indexed: inv.files.len(),
        files_changed,
        symbols: (before.0 - removed.0 + inserted_symbols as i64) as usize,
        occurrences: (before.1 - removed.1 + inserted_occurrences as i64) as usize,
        chunks: (before.2 - removed.2 + inserted_chunks as i64) as usize,
        unparsed_files: inv.unparsed.iter().map(|(_, n)| n).sum(),
        files_with_errors: (before.3 - removed.3 + inserted_error_files as i64) as usize,
        coverage,
    };

    // The run counts must equal the tables (pin 12's mechanism — a drill
    // that hard-codes counts is red by construction against this). Verified
    // INSIDE the transaction, before the commit.
    let stored_files: i64 = tx.query_row(
        "SELECT COUNT(*) FROM code_file WHERE repo_id = ?1",
        [repo_id],
        |r| r.get(0),
    )?;
    let stored_symbols: i64 = tx.query_row(
        "SELECT COUNT(*) FROM code_symbol s JOIN code_file f ON f.id = s.file_id \
         WHERE f.repo_id = ?1",
        [repo_id],
        |r| r.get(0),
    )?;
    let stored_occurrences: i64 = tx.query_row(
        "SELECT COUNT(*) FROM code_occurrence o JOIN code_file f ON f.id = o.file_id \
         WHERE f.repo_id = ?1",
        [repo_id],
        |r| r.get(0),
    )?;
    let stored_chunks: i64 = tx.query_row(
        "SELECT COUNT(*) FROM code_chunk c JOIN code_file f ON f.id = c.file_id \
         WHERE f.repo_id = ?1",
        [repo_id],
        |r| r.get(0),
    )?;
    let stored_vecs: i64 = tx.query_row("SELECT COUNT(*) FROM code_chunk_vec", [], |r| r.get(0))?;
    if stored_files != report.files_indexed as i64
        || stored_symbols != report.symbols as i64
        || stored_occurrences != report.occurrences as i64
        || stored_chunks != report.chunks as i64
        || stored_vecs != report.chunks as i64
    {
        return Err(CodeIndexError::Refusal(format!(
            "run counts disagree with the tables after ingest (run {run_id}): \
             files {stored_files} vs {}, symbols {stored_symbols} vs {}, \
             occurrences {stored_occurrences} vs {}, chunks {stored_chunks} vs {} (vec {stored_vecs})",
            report.files_indexed, report.symbols, report.occurrences, report.chunks
        )));
    }

    tx.commit()?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::fake::FakeEncoder;
    use crate::host::Vec0Host;
    use std::path::PathBuf;
    use std::process::Command;

    struct ShimHost;

    impl Vec0Host for ShimHost {
        fn ensure_vec0_registered(&self) -> Result<(), CodeIndexError> {
            brain_code_vec0::register_sqlite_vec();
            Ok(())
        }
    }

    /// A scratch git repository exercising every ingest branch: rust sources
    /// (one clean, one ambiguous, one broken), a generated file, a test file,
    /// a ts file, and one unparsed extension.
    fn fixture_repo(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "brain-code-core-s3-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn helper() {}\n\nfn caller() {\n    let _ = helper();\n}\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/broken.rs"), "fn fine() {}\nfn broken( {\n").unwrap();
        std::fs::write(
            dir.join("tests/it_works.rs"),
            "#[test]\nfn it_works() {\n    assert_eq!(1 + 1, 2);\n}\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/schema.d.ts"),
            "export interface Generated { field: string; }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/app.ts"),
            "export function boot(target: string): void {}\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/App.svelte"), "<p>quiet surface</p>\n").unwrap();
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?} failed in the fixture"
            );
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

    fn scratch_index(label: &str) -> (CodeIndex, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "brain-code-core-s3-index-{label}-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let index = CodeIndex::open(&path, &ShimHost).unwrap();
        index.migrate(8).unwrap();
        (index, path)
    }

    #[test]
    fn an_ingest_reports_the_coverage_table_including_the_unparsed() {
        let repo = fixture_repo("coverage");
        let (index, index_path) = scratch_index("coverage");
        let report = ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let ext = |name: &str| {
            report
                .coverage
                .iter()
                .find(|c| c.ext == name)
                .cloned()
                .unwrap_or_else(|| panic!("no {name} row in the coverage table"))
        };
        let svelte = ext("svelte");
        assert_eq!((svelte.seen, svelte.parsed, svelte.errors), (1, 0, 0));
        let rs = ext("rs");
        assert_eq!(rs.seen, 3, "main.rs, broken.rs, tests/it_works.rs");
        assert_eq!(
            rs.parsed, 3,
            "a file with errors is still parsed for clean symbols"
        );
        assert_eq!(rs.errors, 1, "broken.rs is counted, not skipped");
        let ts = ext("ts");
        assert_eq!(
            (ts.seen, ts.parsed, ts.errors),
            (2, 1, 0),
            "app.ts parses; the generated schema is inventoried, never parsed"
        );
        assert_eq!(report.files_with_errors, 1);
        assert_eq!(report.files_indexed, 5);
        assert_eq!(report.unparsed_files, 1);
        assert_eq!(report.commit.len(), 40);
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn the_run_counts_agree_with_the_tables() {
        let repo = fixture_repo("counts");
        let (index, index_path) = scratch_index("counts");
        let report = ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let conn = index.conn();
        let files: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_file", [], |r| r.get(0))
            .unwrap();
        let symbols: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_symbol", [], |r| r.get(0))
            .unwrap();
        let occurrences: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_occurrence", [], |r| r.get(0))
            .unwrap();
        assert_eq!(files, report.files_indexed as i64);
        assert_eq!(symbols, report.symbols as i64);
        assert_eq!(occurrences, report.occurrences as i64);
        let chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_chunk", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            chunks, report.chunks as i64,
            "the run's chunk count equals the table"
        );
        let vecs: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_chunk_vec", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vecs, chunks, "every chunk carries exactly one vector");
        let windows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_chunk WHERE kind = 'window'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            windows > 0,
            "windowed chunks are tagged so they can be counted"
        );
        let (embed_dim, embed_profile): (i64, String) = conn
            .query_row("SELECT embed_dim, embed_profile FROM code_repo", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            embed_dim, 8,
            "the recorded dim is the PORT's width (pin 13)"
        );
        assert_eq!(embed_profile, "test");
        let status: String = conn
            .query_row("SELECT status FROM code_index_run", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "ok");
        let aligned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_occurrence WHERE symbol_id IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let unaligned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_occurrence WHERE symbol_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(aligned > 0, "definitions and unambiguous refs are aligned");
        assert!(unaligned > 0, "cross-file and ambiguous names stay NULL");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn a_generated_file_yields_no_symbols() {
        let repo = fixture_repo("generated");
        let (index, index_path) = scratch_index("generated");
        ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let conn = index.conn();
        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_file WHERE is_generated = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "the generated file has a row (file-lexical leg)");
        let symbols: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_symbol s JOIN code_file f ON f.id = s.file_id \n                 WHERE f.is_generated = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            symbols, 0,
            "a generated file never yields symbol work (pin 9)"
        );
        let chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_chunk c JOIN code_file f ON f.id = c.file_id \n                 WHERE f.is_generated = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            chunks, 0,
            "a generated file never yields a symbol chunk (pin 9)"
        );
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn a_failed_ingest_writes_no_partial_data_and_records_failed() {
        let repo = fixture_repo("failed");
        let (index, index_path) = scratch_index("failed");
        let bail: FileHook = Some(&|file_index| {
            if file_index == 1 {
                Err(CodeIndexError::Refusal(
                    "pin-4 drill: injected failure".to_string(),
                ))
            } else {
                Ok(())
            }
        });
        let refused = ingest(&index, &repo, &FakeEncoder::new("test", 8), bail);
        assert!(refused.is_err(), "the injected failure surfaces");
        let conn = index.conn();
        let symbols: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_symbol", [], |r| r.get(0))
            .unwrap();
        let files: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_file", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            (symbols, files),
            (0, 0),
            "one transaction or nothing (pin 4)"
        );
        let (status, detail): (String, String) = conn
            .query_row("SELECT status, detail FROM code_index_run", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(status, "failed", "the run row says failed");
        assert!(
            detail.contains("injected failure"),
            "the failure names itself: {detail}"
        );
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn an_unreadable_git_dir_refuses_and_writes_nothing() {
        let bare = std::env::temp_dir().join(format!(
            "brain-code-core-s3-nogit-ingest-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&bare).unwrap();
        let (index, index_path) = scratch_index("nogit");
        let refused = ingest(&index, &bare, &FakeEncoder::new("test", 8), None);
        assert!(refused.is_err(), "ingest refuses (pin 16)");
        let conn = index.conn();
        let repos: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_repo", [], |r| r.get(0))
            .unwrap();
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_index_run", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            (repos, runs),
            (0, 0),
            "an empty index is never written for an unreadable repo"
        );
        let _ = std::fs::remove_dir_all(&bare);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn re_ingesting_the_same_commit_adds_one_run_row_and_zero_data_rows() {
        let repo = fixture_repo("noop");
        let (index, index_path) = scratch_index("noop");
        ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let conn = index.conn();
        let counts_before: Vec<i64> = ["code_file", "code_symbol", "code_occurrence", "code_chunk"]
            .iter()
            .map(|t| {
                conn.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                    .unwrap()
            })
            .collect();
        let max_ids_before: Vec<i64> = ["code_file", "code_symbol", "code_chunk"]
            .iter()
            .map(|t| {
                conn.query_row(&format!("SELECT COALESCE(MAX(id), 0) FROM {t}"), [], |r| {
                    r.get(0)
                })
                .unwrap()
            })
            .collect();

        let report = ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        assert_eq!(
            report.files_changed, 0,
            "an unchanged re-ingest moves nothing"
        );
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_index_run", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, 2, "the re-run adds exactly one run row");
        for (table, before) in ["code_file", "code_symbol", "code_occurrence", "code_chunk"]
            .iter()
            .zip(counts_before)
        {
            let after: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                after, before,
                "{table} gains zero rows on re-ingest (pin 5)"
            );
        }
        for (table, before) in ["code_file", "code_symbol", "code_chunk"]
            .iter()
            .zip(max_ids_before)
        {
            let after: i64 = conn
                .query_row(
                    &format!("SELECT COALESCE(MAX(id), 0) FROM {table}"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                after, before,
                "{table} rowids untouched — no delete+rewrite happened"
            );
        }
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn a_ten_file_change_reparses_exactly_ten_files() {
        let repo = fixture_repo("tenchange");
        // Grow the fixture to 12 rust files, all tracked in one commit.
        for i in 0..11 {
            std::fs::write(
                repo.join(format!("src/unit_{i}.rs")),
                format!("pub fn unit_{i}() -> u8 {{\n    {i}\n}}\n"),
            )
            .unwrap();
        }
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        };
        git(&["add", "-A"]);
        git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "12 files",
        ]);

        let (index, index_path) = scratch_index("tenchange");
        ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let conn = index.conn();
        let id_of = |path: &str| -> i64 {
            conn.query_row("SELECT id FROM code_file WHERE path = ?1", [path], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let untouched_before = id_of("src/unit_0.rs");
        let moved_before: Vec<i64> = (1..=10)
            .map(|i| id_of(&format!("src/unit_{i}.rs")))
            .collect();

        // Move exactly ten files; unit_0 stays; unit_11 stays.
        for i in 1..=10 {
            std::fs::write(
                repo.join(format!("src/unit_{i}.rs")),
                format!("pub fn unit_{i}() -> u8 {{\n    {i} + 100\n}}\n"),
            )
            .unwrap();
        }
        git(&["add", "-A"]);
        git(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "move ten",
        ]);

        let report = ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        assert_eq!(
            report.files_changed, 10,
            "exactly the ten movers were rebuilt (pin 6)"
        );
        assert_eq!(
            id_of("src/unit_0.rs"),
            untouched_before,
            "an unmoved file keeps its row untouched"
        );
        for (i, before) in (1..=10).zip(moved_before) {
            let after = id_of(&format!("src/unit_{i}.rs"));
            assert_ne!(
                after, before,
                "unit_{i} was deleted and rebuilt, not edited in place"
            );
        }
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }

    #[test]
    fn a_profile_change_refuses_until_rebuilt() {
        let repo = fixture_repo("profile");
        let (index, index_path) = scratch_index("profile");
        ingest(&index, &repo, &FakeEncoder::new("test", 8), None).unwrap();
        let refused = ingest(&index, &repo, &FakeEncoder::new("other", 16), None);
        match refused {
            Err(CodeIndexError::Refusal(reason)) => {
                assert!(
                    reason.contains("8") && reason.contains("16") && reason.contains("test"),
                    "the refusal names both profiles: {reason}"
                );
            }
            other => panic!("expected the profile-change refusal, got {other:?}"),
        }
        let conn = index.conn();
        let runs: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_index_run", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, 1, "the refusal happened before any run row");
        let _ = std::fs::remove_dir_all(&repo);
        let _ = std::fs::remove_file(&index_path);
    }
}
