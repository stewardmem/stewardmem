//! The C0 schema (S2): the eleven tables, their secondary indexes, and the
//! FTS sync triggers, created in the order §0.6 mandates — `code_repo` →
//! `code_package` → `code_file` → the rest — presence-guarded, with CHECK
//! constraints on every enum-like column and the house timestamp on every
//! written row.
//!
//! Corrections this schema carries against the study's §5 (do not transcribe
//! the study): `UNIQUE(repo_id, manifest_path)` on packages; `repo_id` on
//! runs; `doc_id` dropped until C4; the six secondary indexes; house
//! timestamps; `embed_dim`/`embed_profile` on `code_repo` (R-7/P-5 — the
//! dimension is a parameter of the migration, taken from the encoder port,
//! never a fixed 768); `code_chunk_vec` carries `chunk_id` only — no
//! metadata columns (R-6); `code_file.content` stays because the file-level
//! lexical leg needs whole-file text (R-1); C2–C4 tables are not created.
//!
//! The FTS tables are external-content, kept in sync by triggers (the
//! SQLite-documented pattern); the vector table is created with the
//! encoder's dimension baked into the DDL, because a profile change is a
//! rebuild, never a silent mix (P-5).

use rusqlite::OptionalExtension;

use crate::error::CodeIndexError;

/// The C0 index version. It moves only when the schema's shape moves.
pub const INDEX_VERSION: i64 = 1;

const TABLES: &[&str] = &[
    "code_repo",
    "code_package",
    "code_file",
    "code_symbol",
    "code_occurrence",
    "code_chunk",
    "code_file_fts",
    "code_chunk_fts",
    "code_symbol_fts",
    "code_chunk_vec",
    "code_index_run",
];

const INDEXES: &[&str] = &[
    "idx_code_symbol_file",
    "idx_code_symbol_qualified",
    "idx_code_chunk_file",
    "idx_code_chunk_content_hash",
    "idx_code_occurrence_file",
    "idx_code_occurrence_symbol",
];

const TRIGGERS: &[&str] = &[
    "code_file_fts_ai",
    "code_file_fts_ad",
    "code_file_fts_au",
    "code_chunk_fts_ai",
    "code_chunk_fts_ad",
    "code_chunk_fts_au",
    "code_symbol_fts_ai",
    "code_symbol_fts_ad",
    "code_symbol_fts_au",
];

pub struct CodeIndex {
    conn: rusqlite::Connection,
}

impl CodeIndex {
    /// Open (creating if absent) the per-repo index file. The host's vec0
    /// registration must happen before this call opens the connection —
    /// `sqlite3_auto_extension` applies to connections opened after it.
    pub fn open(
        path: &std::path::Path,
        host: &dyn crate::host::Vec0Host,
    ) -> Result<Self, CodeIndexError> {
        host.ensure_vec0_registered()?;
        let conn = rusqlite::Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    /// Open an in-memory index (tests).
    pub fn open_in_memory(host: &dyn crate::host::Vec0Host) -> Result<Self, CodeIndexError> {
        host.ensure_vec0_registered()?;
        let conn = rusqlite::Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self { conn })
    }

    /// The raw connection for the crate's own modules. The CLI adapter never
    /// sees it (pin 19: the adapter is wiring only).
    pub(crate) fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }

    /// Create every C0 object that is missing. Presence-guarded: a second
    /// call on a migrated file changes nothing. `embed_dim` is the encoder
    /// port's width (R-7); it is baked into the vec0 DDL because the table
    /// cannot be re-shaped in place — a profile change is a rebuild (P-5).
    pub fn migrate(&self, embed_dim: usize) -> Result<(), CodeIndexError> {
        if embed_dim == 0 {
            return Err(CodeIndexError::Refusal(
                "embed_dim must be at least 1 — the encoder port lied".to_string(),
            ));
        }
        let ddl = migration_sql(embed_dim);
        self.conn.execute_batch(&ddl)?;
        Ok(())
    }

    /// Every C0 object present? Migration, integrity, and foreign keys are
    /// checked together by the pins; this reports completeness by name.
    pub fn missing_objects(&self) -> Result<Vec<String>, CodeIndexError> {
        let mut missing = Vec::new();
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE name = ?1")?;
        for name in TABLES.iter().chain(INDEXES.iter()).chain(TRIGGERS.iter()) {
            let found: Option<String> = stmt.query_row([name], |row| row.get(0)).optional()?;
            if found.is_none() {
                missing.push((*name).to_string());
            }
        }
        Ok(missing)
    }

    /// `PRAGMA foreign_key_check` — empty when the store is consistent.
    pub fn foreign_key_violations(&self) -> Result<usize, CodeIndexError> {
        let mut stmt = self.conn.prepare("PRAGMA foreign_key_check")?;
        let rows = stmt.query_map([], |_| Ok(()))?;
        Ok(rows.count())
    }

    /// `PRAGMA integrity_check` — "ok" when the store is consistent.
    pub fn integrity_check(&self) -> Result<String, CodeIndexError> {
        let result: String = self
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        Ok(result)
    }
}

fn migration_sql(embed_dim: usize) -> String {
    // The house timestamp on every written row; CHECKs on every enum-like
    // column; create order repo → package → file → the rest (§0.6).
    format!(
        r#"
CREATE TABLE IF NOT EXISTS code_repo (
  id            INTEGER PRIMARY KEY,
  root          TEXT NOT NULL UNIQUE,
  vcs           TEXT NOT NULL DEFAULT 'git' CHECK (vcs IN ('git')),
  head_commit   TEXT NOT NULL,
  index_version INTEGER NOT NULL,
  embed_dim     INTEGER NOT NULL CHECK (embed_dim > 0),
  embed_profile TEXT NOT NULL,
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE IF NOT EXISTS code_package (
  id            INTEGER PRIMARY KEY,
  repo_id       INTEGER NOT NULL REFERENCES code_repo(id) ON DELETE CASCADE,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL CHECK (kind IN ('crate','workspace','npm','tsconfig')),
  manifest_path TEXT NOT NULL,
  deps_json     TEXT NOT NULL DEFAULT '[]',
  UNIQUE(repo_id, manifest_path)
);

CREATE TABLE IF NOT EXISTS code_file (
  id           INTEGER PRIMARY KEY,
  repo_id      INTEGER NOT NULL REFERENCES code_repo(id) ON DELETE CASCADE,
  package_id   INTEGER REFERENCES code_package(id) ON DELETE SET NULL,
  path         TEXT NOT NULL,
  lang         TEXT NOT NULL CHECK (lang IN ('rust','typescript','tsx')),
  content_hash TEXT NOT NULL,
  loc          INTEGER NOT NULL CHECK (loc >= 0),
  content      TEXT NOT NULL,
  is_generated INTEGER NOT NULL DEFAULT 0 CHECK (is_generated IN (0,1)),
  is_test      INTEGER NOT NULL DEFAULT 0 CHECK (is_test IN (0,1)),
  has_errors   INTEGER NOT NULL DEFAULT 0 CHECK (has_errors IN (0,1)),
  parsed       INTEGER NOT NULL DEFAULT 0 CHECK (parsed IN (0,1)),
  UNIQUE(repo_id, path)
);

CREATE TABLE IF NOT EXISTS code_symbol (
  id             INTEGER PRIMARY KEY,
  file_id        INTEGER NOT NULL REFERENCES code_file(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL CHECK (kind IN (
                   'fn','struct','enum','union','trait','impl','type','const','static','mod',
                   'class','interface','type_alias','method','module','ambient')),
  name           TEXT NOT NULL,
  qualified_name TEXT NOT NULL,
  signature      TEXT,
  start_line     INTEGER NOT NULL CHECK (start_line >= 1),
  end_line       INTEGER NOT NULL CHECK (end_line >= start_line),
  start_byte     INTEGER NOT NULL CHECK (start_byte >= 0),
  end_byte       INTEGER NOT NULL CHECK (end_byte >= start_byte),
  parent_id      INTEGER REFERENCES code_symbol(id) ON DELETE CASCADE,
  visibility     TEXT CHECK (visibility IS NULL OR visibility IN ('public','crate','private'))
);

CREATE TABLE IF NOT EXISTS code_occurrence (
  id         INTEGER PRIMARY KEY,
  file_id    INTEGER NOT NULL REFERENCES code_file(id) ON DELETE CASCADE,
  symbol_id  INTEGER REFERENCES code_symbol(id) ON DELETE SET NULL,
  role       TEXT NOT NULL CHECK (role IN ('def','ref')),
  name       TEXT NOT NULL,
  start_line INTEGER NOT NULL CHECK (start_line >= 1),
  start_col  INTEGER NOT NULL CHECK (start_col >= 0),
  end_line   INTEGER NOT NULL CHECK (end_line >= start_line),
  end_col    INTEGER NOT NULL CHECK (end_col >= 0)
);

CREATE TABLE IF NOT EXISTS code_chunk (
  id             INTEGER PRIMARY KEY,
  file_id        INTEGER NOT NULL REFERENCES code_file(id) ON DELETE CASCADE,
  symbol_id      INTEGER REFERENCES code_symbol(id) ON DELETE CASCADE,
  kind           TEXT NOT NULL CHECK (kind IN ('symbol','window')),
  text           TEXT NOT NULL,
  token_estimate INTEGER NOT NULL CHECK (token_estimate >= 0),
  content_hash   TEXT NOT NULL
);

-- R-1: whole-file text backs the file-level lexical leg. The duplication
-- with code_chunk.text is deliberate; its cost is measured (S8).
CREATE VIRTUAL TABLE IF NOT EXISTS code_file_fts USING fts5(
  content,
  content='code_file',
  content_rowid='id',
  tokenize='unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS code_chunk_fts USING fts5(
  text,
  content='code_chunk',
  content_rowid='id',
  tokenize='unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS code_symbol_fts USING fts5(
  name,
  qualified_name,
  signature,
  content='code_symbol',
  content_rowid='id',
  tokenize='trigram'
);

-- R-6: chunk_id only — no metadata columns. Filtering goes through an id
-- set inside the KNN, never a metadata column.
CREATE VIRTUAL TABLE IF NOT EXISTS code_chunk_vec USING vec0(
  chunk_id INTEGER PRIMARY KEY,
  embedding float[{embed_dim}]
);

CREATE TABLE IF NOT EXISTS code_index_run (
  id                INTEGER PRIMARY KEY,
  repo_id           INTEGER NOT NULL REFERENCES code_repo(id) ON DELETE CASCADE,
  head_commit       TEXT NOT NULL,
  started           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  finished          TEXT,
  status            TEXT NOT NULL CHECK (status IN ('running','ok','failed')),
  files             INTEGER NOT NULL DEFAULT 0 CHECK (files >= 0),
  symbols           INTEGER NOT NULL DEFAULT 0 CHECK (symbols >= 0),
  occurrences       INTEGER NOT NULL DEFAULT 0 CHECK (occurrences >= 0),
  chunks            INTEGER NOT NULL DEFAULT 0 CHECK (chunks >= 0),
  unparsed_files    INTEGER NOT NULL DEFAULT 0 CHECK (unparsed_files >= 0),
  files_with_errors INTEGER NOT NULL DEFAULT 0 CHECK (files_with_errors >= 0),
  detail            TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_code_symbol_file ON code_symbol(file_id);
CREATE INDEX IF NOT EXISTS idx_code_symbol_qualified ON code_symbol(qualified_name);
CREATE INDEX IF NOT EXISTS idx_code_chunk_file ON code_chunk(file_id);
CREATE INDEX IF NOT EXISTS idx_code_chunk_content_hash ON code_chunk(content_hash);
CREATE INDEX IF NOT EXISTS idx_code_occurrence_file ON code_occurrence(file_id);
CREATE INDEX IF NOT EXISTS idx_code_occurrence_symbol ON code_occurrence(symbol_id);

-- External-content FTS stays in sync through triggers (the SQLite-documented
-- pattern): insert on content insert, 'delete' with the old values on
-- delete/update.
CREATE TRIGGER IF NOT EXISTS code_file_fts_ai AFTER INSERT ON code_file BEGIN
  INSERT INTO code_file_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS code_file_fts_ad AFTER DELETE ON code_file BEGIN
  INSERT INTO code_file_fts(code_file_fts, rowid, content) VALUES ('delete', old.id, old.content);
END;
CREATE TRIGGER IF NOT EXISTS code_file_fts_au AFTER UPDATE ON code_file BEGIN
  INSERT INTO code_file_fts(code_file_fts, rowid, content) VALUES ('delete', old.id, old.content);
  INSERT INTO code_file_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS code_chunk_fts_ai AFTER INSERT ON code_chunk BEGIN
  INSERT INTO code_chunk_fts(rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER IF NOT EXISTS code_chunk_fts_ad AFTER DELETE ON code_chunk BEGIN
  INSERT INTO code_chunk_fts(code_chunk_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER IF NOT EXISTS code_chunk_fts_au AFTER UPDATE ON code_chunk BEGIN
  INSERT INTO code_chunk_fts(code_chunk_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO code_chunk_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TRIGGER IF NOT EXISTS code_symbol_fts_ai AFTER INSERT ON code_symbol BEGIN
  INSERT INTO code_symbol_fts(rowid, name, qualified_name, signature)
  VALUES (new.id, new.name, new.qualified_name, COALESCE(new.signature, ''));
END;
CREATE TRIGGER IF NOT EXISTS code_symbol_fts_ad AFTER DELETE ON code_symbol BEGIN
  INSERT INTO code_symbol_fts(code_symbol_fts, rowid, name, qualified_name, signature)
  VALUES ('delete', old.id, old.name, old.qualified_name, COALESCE(old.signature, ''));
END;
CREATE TRIGGER IF NOT EXISTS code_symbol_fts_au AFTER UPDATE ON code_symbol BEGIN
  INSERT INTO code_symbol_fts(code_symbol_fts, rowid, name, qualified_name, signature)
  VALUES ('delete', old.id, old.name, old.qualified_name, COALESCE(old.signature, ''));
  INSERT INTO code_symbol_fts(rowid, name, qualified_name, signature)
  VALUES (new.id, new.name, new.qualified_name, COALESCE(new.signature, ''));
END;
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Vec0Host;
    use std::path::PathBuf;

    /// The tests' host: the audited shim, the same function the kernel's CLI
    /// injects in production (its own copy of the same registration).
    struct ShimHost;

    impl Vec0Host for ShimHost {
        fn ensure_vec0_registered(&self) -> Result<(), CodeIndexError> {
            brain_code_vec0::register_sqlite_vec();
            Ok(())
        }
    }

    fn host() -> ShimHost {
        ShimHost
    }

    fn scratch_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "brain-code-core-s2-{label}-{}-{nanos}.db",
            std::process::id()
        ))
    }

    #[test]
    fn migration_is_green_on_a_fresh_file() {
        let path = scratch_path("fresh");
        let index = CodeIndex::open(&path, &host()).unwrap();
        index.migrate(8).unwrap();
        assert!(
            index.missing_objects().unwrap().is_empty(),
            "every C0 object present after migration"
        );
        assert_eq!(index.foreign_key_violations().unwrap(), 0);
        assert_eq!(index.integrity_check().unwrap(), "ok");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migration_rerun_is_a_noop() {
        let path = scratch_path("rerun");
        let index = CodeIndex::open(&path, &host()).unwrap();
        index.migrate(8).unwrap();
        let master_before: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
            .unwrap();
        index.migrate(8).unwrap();
        let master_after: i64 = index
            .conn()
            .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| row.get(0))
            .unwrap();
        assert_eq!(master_before, master_after, "a re-run creates nothing");
        assert_eq!(index.integrity_check().unwrap(), "ok");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_vec_table_has_no_metadata_columns() {
        // R-6, structural: the DDL carries chunk_id and nothing else.
        let index = CodeIndex::open_in_memory(&host()).unwrap();
        index.migrate(8).unwrap();
        let ddl: String = index
            .conn()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'code_chunk_vec'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(ddl.contains("chunk_id"), "vec table keyed by chunk_id");
        assert!(!ddl.contains("repo_id"), "no repo_id metadata column (R-6)");
        assert!(!ddl.contains("lang TEXT"), "no lang metadata column (R-6)");
        assert!(!ddl.contains("kind TEXT"), "no kind metadata column (R-6)");
    }

    #[test]
    fn the_enum_checks_reject_bad_values() {
        let index = CodeIndex::open_in_memory(&host()).unwrap();
        index.migrate(8).unwrap();
        let conn = index.conn();
        conn.execute_batch(
            "INSERT INTO code_repo(root, head_commit, index_version, embed_dim, embed_profile)
             VALUES ('/r', 'c0ffee', 1, 8, 'test');
             INSERT INTO code_package(repo_id, name, kind, manifest_path)
             VALUES (1, 'p', 'crate', '/r/Cargo.toml');
             INSERT INTO code_file(repo_id, path, lang, content_hash, loc, content)
             VALUES (1, '/r/a.rs', 'rust', 'h1', 1, 'fn a() {}');",
        )
        .unwrap();
        let bad_symbol = conn.execute(
            "INSERT INTO code_symbol(file_id, kind, name, qualified_name, start_line, end_line, start_byte, end_byte)
             VALUES (1, 'bogus', 'a', 'a', 1, 1, 0, 8)",
            [],
        );
        assert!(bad_symbol.is_err(), "a bogus symbol kind is refused");
        let bad_role = conn.execute(
            "INSERT INTO code_occurrence(file_id, role, name, start_line, start_col, end_line, end_col)
             VALUES (1, 'write', 'a', 1, 0, 1, 1)",
            [],
        );
        assert!(
            bad_role.is_err(),
            "a 'write' role is outside the C0 vocabulary"
        );
        let bad_status = conn.execute(
            "INSERT INTO code_index_run(repo_id, head_commit, status) VALUES (1, 'c0ffee', 'meh')",
            [],
        );
        assert!(bad_status.is_err(), "a bogus run status is refused");
        let good_ref = conn.execute(
            "INSERT INTO code_occurrence(file_id, role, name, start_line, start_col, end_line, end_col)
             VALUES (1, 'ref', 'a', 1, 3, 1, 4)",
            [],
        );
        assert!(good_ref.is_ok(), "def/ref are the C0 roles");
    }

    #[test]
    fn the_fts_triggers_keep_external_content_in_sync() {
        let index = CodeIndex::open_in_memory(&host()).unwrap();
        index.migrate(8).unwrap();
        let conn = index.conn();
        conn.execute_batch(
            "INSERT INTO code_repo(root, head_commit, index_version, embed_dim, embed_profile)
             VALUES ('/r', 'c0ffee', 1, 8, 'test');
             INSERT INTO code_file(repo_id, path, lang, content_hash, loc, content)
             VALUES (1, '/r/fence.rs', 'rust', 'h', 3, 'the fence holds at RELAYED by measurement');",
        )
        .unwrap();
        let hits: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_file_fts WHERE code_file_fts MATCH 'RELAYED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "the insert trigger indexed the content");
        conn.execute("DELETE FROM code_file WHERE path = '/r/fence.rs'", [])
            .unwrap();
        let hits_after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM code_file_fts WHERE code_file_fts MATCH 'RELAYED'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits_after, 0, "the delete trigger deindexed the content");
        assert_eq!(index.integrity_check().unwrap(), "ok");
    }

    #[test]
    fn a_zero_embed_dim_is_refused_by_name() {
        let index = CodeIndex::open_in_memory(&host()).unwrap();
        let refused = index.migrate(0);
        match refused {
            Err(CodeIndexError::Refusal(reason)) => {
                assert!(
                    reason.contains("embed_dim"),
                    "the refusal names the cause: {reason}"
                );
            }
            other => panic!("expected a named refusal, got {other:?}"),
        }
    }
}
