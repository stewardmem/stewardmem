//! The three registry tables' DML: the inserts, the reads a sign re-derives
//! from, and the sign's updates. Every function takes the CALLER's
//! connection — the crate opens no transaction of its own and holds no DDL
//! (the server's migration owns the tables); the service core composes these
//! inside its own boundaries and audits inside them.

use rusqlite::{Connection, params};

use crate::vocab::{ScaffoldKind, ScaffoldStatus, Split, Warrant};

/// A storage failure. `Database` keeps the typed rusqlite error so a caller
/// can match it (the sign core matches the partial unique index's violation,
/// and the no-rows case is matched here); `Vocabulary` is a row whose closed
/// vocabulary failed to parse back — unreachable while the CHECK constraints
/// hold, and loud if they ever do not.
#[derive(Debug)]
pub enum ScaffoldError {
    Database(rusqlite::Error),
    Vocabulary(String),
}

impl std::fmt::Display for ScaffoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScaffoldError::Database(e) => write!(f, "scaffold store: {e}"),
            ScaffoldError::Vocabulary(value) => write!(
                f,
                "scaffold store: a closed vocabulary failed to parse back: {value}"
            ),
        }
    }
}

impl std::error::Error for ScaffoldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScaffoldError::Database(e) => Some(e),
            ScaffoldError::Vocabulary(_) => None,
        }
    }
}

impl From<rusqlite::Error> for ScaffoldError {
    fn from(e: rusqlite::Error) -> Self {
        ScaffoldError::Database(e)
    }
}

/// One scaffold row — the read's shape: the kind, the status, the parent a
/// sign walks to reach the parent's bench runs.
#[derive(Debug, Clone, PartialEq)]
pub struct ScaffoldRow {
    pub id: i64,
    pub kind: ScaffoldKind,
    pub status: ScaffoldStatus,
    pub parent_id: Option<i64>,
}

/// One proposal row: what a sign re-derives from.
#[derive(Debug, Clone, PartialEq)]
pub struct ProposalRow {
    pub id: i64,
    pub scaffold_id: i64,
    pub diff: String,
    pub rationale: String,
    pub cites_trace: Option<String>,
    pub warrant: Warrant,
    pub recorded_by: String,
    pub signed_by: Option<String>,
    pub signed_at: Option<String>,
}

/// One bench-run row: the paired evidence a sign re-derives the verdict
/// from. `raw_output` lives in the row so `raw_sha256` can be re-verified
/// against it.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchRunRow {
    pub id: i64,
    pub scaffold_id: i64,
    pub split: Split,
    pub corpus_hash: String,
    pub commit: String,
    pub metric: String,
    pub value: f64,
    pub raw_output: String,
    pub raw_sha256: String,
}

/// Record a candidate scaffold. `body_hash` is computed by the caller — the
/// service core hashes in Rust; the crate carries no hash of its own.
pub fn insert_candidate(
    conn: &Connection,
    kind: ScaffoldKind,
    body: &str,
    body_hash: &str,
    parent_id: Option<i64>,
    created_by: &str,
) -> Result<i64, ScaffoldError> {
    conn.execute(
        "INSERT INTO scaffold(kind, body, body_hash, parent_id, status, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            kind.as_str(),
            body,
            body_hash,
            parent_id,
            ScaffoldStatus::Candidate.as_str(),
            created_by
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// One bench run to store: the paired evidence a sign re-derives the verdict
/// from. `raw_sha256` is computed by the caller over the raw bytes; the sign
/// re-verifies it against the stored output.
pub struct BenchRunInput<'a> {
    pub scaffold_id: i64,
    pub split: Split,
    pub corpus_hash: &'a str,
    pub commit: &'a str,
    pub metric: &'a str,
    pub value: f64,
    pub raw_output: &'a str,
    pub raw_sha256: &'a str,
}

/// Store one bench run.
pub fn insert_bench_run(conn: &Connection, run: &BenchRunInput<'_>) -> Result<i64, ScaffoldError> {
    conn.execute(
        "INSERT INTO bench_run(scaffold_id, split, corpus_hash, \"commit\", metric, value, raw_output, raw_sha256)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            run.scaffold_id,
            run.split.as_str(),
            run.corpus_hash,
            run.commit,
            run.metric,
            run.value,
            run.raw_output,
            run.raw_sha256
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Record a proposal against a candidate.
pub fn insert_proposal(
    conn: &Connection,
    scaffold_id: i64,
    diff: &str,
    rationale: &str,
    cites_trace: Option<&str>,
    warrant: Warrant,
    recorded_by: &str,
) -> Result<i64, ScaffoldError> {
    conn.execute(
        "INSERT INTO scaffold_proposal(scaffold_id, diff, rationale, cites_trace, warrant, recorded_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![scaffold_id, diff, rationale, cites_trace, warrant.as_str(), recorded_by],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The proposal a sign acts on, by id; `None` when the id is not there.
pub fn proposal_by_id(conn: &Connection, id: i64) -> Result<Option<ProposalRow>, ScaffoldError> {
    match conn.query_row_and_then(
        "SELECT id, scaffold_id, diff, rationale, cites_trace, warrant, recorded_by, signed_by, signed_at
         FROM scaffold_proposal WHERE id = ?1",
        params![id],
        row_proposal,
    ) {
        Ok(row) => Ok(Some(row)),
        Err(ScaffoldError::Database(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// One scaffold row by id; `None` when the id is not there.
pub fn scaffold_by_id(conn: &Connection, id: i64) -> Result<Option<ScaffoldRow>, ScaffoldError> {
    match conn.query_row_and_then(
        "SELECT id, kind, status, parent_id FROM scaffold WHERE id = ?1",
        params![id],
        row_scaffold,
    ) {
        Ok(row) => Ok(Some(row)),
        Err(ScaffoldError::Database(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Every bench run recorded for one scaffold, oldest first — the sign pairs
/// the candidate's runs against its parent's.
pub fn bench_runs_for(
    conn: &Connection,
    scaffold_id: i64,
) -> Result<Vec<BenchRunRow>, ScaffoldError> {
    let mut stmt = conn.prepare(
        "SELECT id, scaffold_id, split, corpus_hash, \"commit\", metric, value, raw_output, raw_sha256
         FROM bench_run WHERE scaffold_id = ?1 ORDER BY id",
    )?;
    let mut rows = stmt.query(params![scaffold_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(row_bench_run(row)?);
    }
    Ok(out)
}

/// Retire one scaffold. The sign retires the PARENT; nothing is ever
/// deleted — retired is the only exit.
pub fn retire_scaffold(conn: &Connection, id: i64) -> Result<usize, ScaffoldError> {
    Ok(conn.execute(
        "UPDATE scaffold SET status = ?1 WHERE id = ?2",
        params![ScaffoldStatus::Retired.as_str(), id],
    )?)
}

/// Set one candidate active. The partial unique index (one active row per
/// kind) is the last line of defence: a second active row cannot exist.
pub fn activate_scaffold(conn: &Connection, id: i64) -> Result<usize, ScaffoldError> {
    Ok(conn.execute(
        "UPDATE scaffold SET status = ?1 WHERE id = ?2",
        params![ScaffoldStatus::Active.as_str(), id],
    )?)
}

/// Write the sign's identity onto the proposal. The signer's label was
/// checked against the recorder's in the service core; this write records it.
pub fn mark_proposal_signed(
    conn: &Connection,
    id: i64,
    signed_by: &str,
    signed_at: &str,
) -> Result<usize, ScaffoldError> {
    Ok(conn.execute(
        "UPDATE scaffold_proposal SET signed_by = ?1, signed_at = ?2 WHERE id = ?3",
        params![signed_by, signed_at, id],
    )?)
}

fn row_proposal(row: &rusqlite::Row<'_>) -> Result<ProposalRow, ScaffoldError> {
    let warrant_text: String = row.get("warrant")?;
    Ok(ProposalRow {
        id: row.get("id")?,
        scaffold_id: row.get("scaffold_id")?,
        diff: row.get("diff")?,
        rationale: row.get("rationale")?,
        cites_trace: row.get("cites_trace")?,
        warrant: parse_warrant(&warrant_text)?,
        recorded_by: row.get("recorded_by")?,
        signed_by: row.get("signed_by")?,
        signed_at: row.get("signed_at")?,
    })
}

fn row_scaffold(row: &rusqlite::Row<'_>) -> Result<ScaffoldRow, ScaffoldError> {
    let kind_text: String = row.get("kind")?;
    let status_text: String = row.get("status")?;
    Ok(ScaffoldRow {
        id: row.get("id")?,
        kind: parse_kind(&kind_text)?,
        status: parse_status(&status_text)?,
        parent_id: row.get("parent_id")?,
    })
}

fn row_bench_run(row: &rusqlite::Row<'_>) -> Result<BenchRunRow, ScaffoldError> {
    let split_text: String = row.get("split")?;
    Ok(BenchRunRow {
        id: row.get("id")?,
        scaffold_id: row.get("scaffold_id")?,
        split: parse_split(&split_text)?,
        corpus_hash: row.get("corpus_hash")?,
        commit: row.get("commit")?,
        metric: row.get("metric")?,
        value: row.get("value")?,
        raw_output: row.get("raw_output")?,
        raw_sha256: row.get("raw_sha256")?,
    })
}

fn parse_kind(value: &str) -> Result<ScaffoldKind, ScaffoldError> {
    ScaffoldKind::try_from(value).map_err(|_| ScaffoldError::Vocabulary(value.to_string()))
}

fn parse_status(value: &str) -> Result<ScaffoldStatus, ScaffoldError> {
    match value {
        "candidate" => Ok(ScaffoldStatus::Candidate),
        "active" => Ok(ScaffoldStatus::Active),
        "retired" => Ok(ScaffoldStatus::Retired),
        other => Err(ScaffoldError::Vocabulary(other.to_string())),
    }
}

fn parse_split(value: &str) -> Result<Split, ScaffoldError> {
    match value {
        "dev" => Ok(Split::Dev),
        "held_out" => Ok(Split::HeldOut),
        other => Err(ScaffoldError::Vocabulary(other.to_string())),
    }
}

fn parse_warrant(value: &str) -> Result<Warrant, ScaffoldError> {
    match value {
        "cited" => Ok(Warrant::Cited),
        "not_applicable" => Ok(Warrant::NotApplicable),
        other => Err(ScaffoldError::Vocabulary(other.to_string())),
    }
}
