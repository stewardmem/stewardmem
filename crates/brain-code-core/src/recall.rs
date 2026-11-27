//! The recall harness (C0 S6) — the three lexical control arms over the
//! frozen set. This is the fair baseline R-1 demands: no vector result is
//! ever reported without these numbers beside it. Nothing here touches a
//! vector; the KNN id-set helper lands beside the arms as the R-6 seam for
//! pin 11 and is unused by the baseline.
//!
//! The arms, per the plan:
//! (a) file-level BM25 over `code_file_fts`;
//! (b) chunk + symbol lexical — the chunk and symbol tables' best hit per
//!     file, rank-fused;
//! (c) both fused by reciprocal rank.
//!
//! The trigram law (pin 10): `code_symbol_fts` is trigram-tokenized, and a
//! shorter-than-3-char token in a trigram MATCH is an FTS error — so short
//! terms never reach it. They take a bounded LIKE on the base table instead.

use rusqlite::{Connection, OptionalExtension, params_from_iter};
use serde::Serialize;

use crate::error::CodeIndexError;
use crate::recall_set::RecallEntry;
use crate::schema::CodeIndex;

/// The rank-fusion constant — the kernel's own `RRF_K` precedent (k = 60).
const RRF_K: f64 = 60.0;

/// The depth the LIKE leg is bounded to (pin 10: bounded LIKE, never an
/// unbounded scan).
const LIKE_BOUND: usize = 100;

/// The file depth every arm is scored at (recall@1/5/10 come off one top-10).
pub const FILE_DEPTH: usize = 10;

// ── The query plan ──────────────────────────────────────────────────────────

/// One compiled question. The tokens of a question are its lowercased
/// alphanumeric runs — `content_hash` searches as `content` and `hash`, which
/// is exactly how unicode61 tokenized the code. Trigram-hostile tokens (fewer
/// than 3 characters) are routed to the LIKE leg, never into a MATCH.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryPlan {
    /// OR of every token, for the unicode61 tables (any length is legal).
    pub fts_match: Option<String>,
    /// OR of the ≥3-char tokens, for the trigram table only.
    pub trigram_match: Option<String>,
    /// The <3-char tokens, searched through a bounded LIKE.
    pub like_terms: Vec<String>,
}

fn quote_fts(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}

fn like_pattern(term: &str) -> String {
    let mut escaped = String::with_capacity(term.len() + 2);
    escaped.push('%');
    for ch in term.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('%');
    escaped
}

/// Compile a question into its plan. A question with no searchable terms at
/// all is a named refusal, never a silent zero.
pub fn compile_query(question: &str) -> Result<QueryPlan, CodeIndexError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut current = String::new();
    for ch in question.chars() {
        if ch.is_alphanumeric() {
            current.extend(ch.to_lowercase());
        } else if !current.is_empty() {
            seen.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        seen.insert(current);
    }
    if seen.is_empty() {
        return Err(CodeIndexError::Refusal(format!(
            "the question carries no searchable terms: {question:?}"
        )));
    }
    let mut fts_terms = Vec::new();
    let mut trigram_terms = Vec::new();
    let mut like_terms = Vec::new();
    for term in &seen {
        fts_terms.push(quote_fts(term));
        if term.chars().count() >= 3 {
            trigram_terms.push(quote_fts(term));
        } else {
            like_terms.push(term.clone());
        }
    }
    Ok(QueryPlan {
        fts_match: Some(fts_terms.join(" OR ")),
        trigram_match: if trigram_terms.is_empty() {
            None
        } else {
            Some(trigram_terms.join(" OR "))
        },
        like_terms,
    })
}

// ── The arms ────────────────────────────────────────────────────────────────

/// One ranked file: the path and the arm's score (bigger is better — the
/// bm25 leg exposes `-bm25`, the fused legs the RRF sum).
#[derive(Debug, Clone, Serialize)]
pub struct FileHit {
    pub path: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolHit {
    pub name: String,
    pub qualified_name: String,
    pub score: f64,
}

/// Arm (a): file-level BM25. bm25() ranks smaller-is-better; the hit exposes
/// the negated value so every arm reads bigger-is-better.
pub fn arm_file_bm25(
    index: &CodeIndex,
    plan: &QueryPlan,
    depth: usize,
) -> Result<Vec<FileHit>, CodeIndexError> {
    let Some(match_str) = &plan.fts_match else {
        return Err(CodeIndexError::Refusal(
            "the plan carries no unicode61 terms — the file arm cannot run".to_string(),
        ));
    };
    let conn = index.conn();
    let mut stmt = conn.prepare(
        "SELECT f.path, -bm25(code_file_fts) AS score \
         FROM code_file_fts JOIN code_file f ON f.id = code_file_fts.rowid \
         WHERE code_file_fts MATCH ?1 \
         ORDER BY bm25(code_file_fts) LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![match_str, depth as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
    })?;
    let mut hits = Vec::new();
    for row in rows {
        let (path, score) = row?;
        hits.push(FileHit { path, score });
    }
    Ok(hits)
}

/// One ranked file id per fts row, bigger-is-better, in the table's own
/// bm25 order, bounded to a scan window. FTS5 forbids bm25() under an
/// aggregate (MIN over it is "unable to use function bm25 in the requested
/// context"), so best-per-file is derived by walking the rank order and
/// keeping each file's first appearance — which IS its best hit.
fn ranked_file_ids(
    conn: &Connection,
    sql: &str,
    match_str: &str,
    depth: usize,
) -> Result<Vec<(i64, f64)>, CodeIndexError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(rusqlite::params![match_str, (depth * 50) as i64], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
    })?;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for row in rows {
        let (file_id, score) = row?;
        if seen.insert(file_id) {
            out.push((file_id, score));
            if out.len() >= depth {
                break;
            }
        }
    }
    Ok(out)
}

/// The paths of files whose symbols LIKE-match the short terms, bounded.
fn like_file_ids(
    conn: &Connection,
    like_terms: &[String],
    depth: usize,
) -> Result<Vec<i64>, CodeIndexError> {
    if like_terms.is_empty() {
        return Ok(Vec::new());
    }
    let mut sql = String::from("SELECT file_id FROM code_symbol WHERE ");
    let mut params: Vec<String> = Vec::new();
    for (idx, term) in like_terms.iter().enumerate() {
        if idx > 0 {
            sql.push_str(" OR ");
        }
        sql.push_str("name LIKE ? OR qualified_name LIKE ?");
        let pattern = like_pattern(term);
        params.push(pattern.clone());
        params.push(pattern);
    }
    sql.push_str(" LIMIT ");
    sql.push_str(&LIKE_BOUND.to_string());
    let mut stmt = conn.prepare(&sql)?;
    let bound: Vec<&str> = params.iter().map(String::as_str).collect();
    let rows = stmt.query_map(params_from_iter(bound.iter()), |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    let _ = depth;
    Ok(out)
}

/// Reciprocal-rank fusion of ranked id lists: score = Σ 1/(RRF_K + rank),
/// 1-based ranks. The kernel's fusion shape, at the kernel's constant.
fn rrf_fuse(lists: &[Vec<(i64, f64)>]) -> Vec<(i64, f64)> {
    let mut scores: std::collections::BTreeMap<i64, f64> = std::collections::BTreeMap::new();
    for list in lists {
        for (rank, (id, _)) in list.iter().enumerate() {
            *scores.entry(*id).or_insert(0.0) += 1.0 / (RRF_K + rank as f64 + 1.0);
        }
    }
    let mut fused: Vec<(i64, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
}

fn paths_for_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<(i64, String)>, CodeIndexError> {
    let mut out = Vec::with_capacity(ids.len());
    {
        let mut stmt = conn.prepare("SELECT id, path FROM code_file WHERE id = ?1")?;
        for id in ids {
            let row = stmt
                .query_row([id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
                .optional()?;
            if let Some(pair) = row {
                out.push(pair);
            }
        }
    }
    Ok(out)
}

/// Arm (b): chunk + symbol lexical. Three legs — the chunk table's best hit
/// per file, the symbol table's best hit per file, and (for the trigram-
/// hostile short terms) the bounded LIKE leg — rank-fused into one file
/// ranking.
pub fn arm_chunk_symbol(
    index: &CodeIndex,
    plan: &QueryPlan,
    depth: usize,
) -> Result<Vec<FileHit>, CodeIndexError> {
    let conn = index.conn();
    let mut legs: Vec<Vec<(i64, f64)>> = Vec::new();
    if let Some(match_str) = &plan.fts_match {
        legs.push(ranked_file_ids(
            conn,
            "SELECT c.file_id, -bm25(code_chunk_fts) AS score \
             FROM code_chunk_fts JOIN code_chunk c ON c.id = code_chunk_fts.rowid \
             WHERE code_chunk_fts MATCH ?1 \
             ORDER BY bm25(code_chunk_fts) LIMIT ?2",
            match_str,
            depth,
        )?);
    }
    if let Some(match_str) = &plan.trigram_match {
        legs.push(ranked_file_ids(
            conn,
            "SELECT s.file_id, -bm25(code_symbol_fts) AS score \
             FROM code_symbol_fts JOIN code_symbol s ON s.id = code_symbol_fts.rowid \
             WHERE code_symbol_fts MATCH ?1 \
             ORDER BY bm25(code_symbol_fts) LIMIT ?2",
            match_str,
            depth,
        )?);
    }
    let like_ids = like_file_ids(conn, &plan.like_terms, depth)?;
    if !like_ids.is_empty() {
        legs.push(
            like_ids
                .into_iter()
                .enumerate()
                .map(|(rank, id)| (id, -(rank as f64)))
                .collect(),
        );
    }
    if legs.is_empty() {
        return Err(CodeIndexError::Refusal(
            "the plan carries no searchable legs — the chunk+symbol arm cannot run".to_string(),
        ));
    }
    let fused = rrf_fuse(&legs);
    let top: Vec<i64> = fused.iter().take(depth).map(|(id, _)| *id).collect();
    let mut by_id: std::collections::BTreeMap<i64, f64> = fused.into_iter().collect();
    let paths = paths_for_ids(conn, &top)?;
    Ok(paths
        .into_iter()
        .map(|(id, path)| FileHit {
            path,
            score: by_id.remove(&id).unwrap_or(0.0),
        })
        .collect())
}

/// Arm (c): both fused — the file arm's ranking and the chunk+symbol arm's,
/// reciprocal-rank-fused at the metric depth.
pub fn arm_fused(
    index: &CodeIndex,
    plan: &QueryPlan,
    depth: usize,
) -> Result<Vec<FileHit>, CodeIndexError> {
    let a = arm_file_bm25(index, plan, depth)?;
    let b = arm_chunk_symbol(index, plan, depth)?;
    let id_of: std::collections::BTreeMap<String, i64> = {
        let conn = index.conn();
        let mut stmt = conn.prepare("SELECT id, path FROM code_file")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i64>(0)?)))?;
        rows.collect::<Result<std::collections::BTreeMap<String, i64>, rusqlite::Error>>()?
    };
    let to_ids = |hits: &[FileHit]| -> Vec<(i64, f64)> {
        hits.iter()
            .enumerate()
            .filter_map(|(rank, hit)| id_of.get(&hit.path).map(|id| (*id, -(rank as f64))))
            .collect()
    };
    let fused = rrf_fuse(&[to_ids(&a), to_ids(&b)]);
    let top: Vec<i64> = fused.iter().take(depth).map(|(id, _)| *id).collect();
    let mut by_id: std::collections::BTreeMap<i64, f64> = fused.into_iter().collect();
    let paths = paths_for_ids(index.conn(), &top)?;
    Ok(paths
        .into_iter()
        .map(|(id, path)| FileHit {
            path,
            score: by_id.remove(&id).unwrap_or(0.0),
        })
        .collect())
}

/// The symbol ranking for the symbol-level metric: trigram MATCH hits first,
/// the bounded LIKE leg's hits appended, truncated to depth.
pub fn arm_symbols(
    index: &CodeIndex,
    plan: &QueryPlan,
    depth: usize,
) -> Result<Vec<SymbolHit>, CodeIndexError> {
    let conn = index.conn();
    let mut hits: Vec<SymbolHit> = Vec::new();
    if let Some(match_str) = &plan.trigram_match {
        let mut stmt = conn.prepare(
            "SELECT s.name, s.qualified_name, -bm25(code_symbol_fts) AS score \
             FROM code_symbol_fts JOIN code_symbol s ON s.id = code_symbol_fts.rowid \
             WHERE code_symbol_fts MATCH ?1 \
             ORDER BY bm25(code_symbol_fts) LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![match_str, depth as i64], |r| {
            Ok(SymbolHit {
                name: r.get(0)?,
                qualified_name: r.get(1)?,
                score: r.get(2)?,
            })
        })?;
        for row in rows {
            hits.push(row?);
        }
    }
    if !plan.like_terms.is_empty() {
        let mut sql = String::from("SELECT name, qualified_name FROM code_symbol WHERE ");
        let mut params: Vec<String> = Vec::new();
        for (idx, term) in plan.like_terms.iter().enumerate() {
            if idx > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str("name LIKE ? OR qualified_name LIKE ?");
            let pattern = like_pattern(term);
            params.push(pattern.clone());
            params.push(pattern);
        }
        sql.push_str(" LIMIT ");
        sql.push_str(&LIKE_BOUND.to_string());
        let mut stmt = conn.prepare(&sql)?;
        let bound: Vec<&str> = params.iter().map(String::as_str).collect();
        let rows = stmt.query_map(params_from_iter(bound.iter()), |r| {
            Ok(SymbolHit {
                name: r.get(0)?,
                qualified_name: r.get(1)?,
                score: 0.0,
            })
        })?;
        for row in rows {
            hits.push(row?);
        }
    }
    hits.truncate(depth);
    Ok(hits)
}

// ── The vector seam (R-6 / pin 11) ─────────────────────────────────────────

/// KNN over `code_chunk_vec` with the id-set filter INSIDE the query — the
/// R-6 form. A filter applied outside the KNN (a WHERE on the outer query
/// after k) is pin 11's red drill and returns short results by construction.
/// An empty id set matches nothing and returns an empty vector — named by
/// being empty, never by error.
pub fn knn_chunk_ids(
    index: &CodeIndex,
    query: &[f32],
    k: usize,
    id_set: &[i64],
) -> Result<Vec<i64>, CodeIndexError> {
    if id_set.is_empty() {
        return Ok(Vec::new());
    }
    let mut blob = Vec::with_capacity(query.len() * 4);
    for value in query {
        blob.extend_from_slice(&value.to_le_bytes());
    }
    let ids_json = serde_json::to_string(id_set)
        .map_err(|e| CodeIndexError::Refusal(format!("the id set does not serialize: {e}")))?;
    let conn = index.conn();
    let mut stmt = conn.prepare(
        "SELECT chunk_id FROM code_chunk_vec \
         WHERE embedding MATCH ?1 AND k = ?2 \
           AND chunk_id IN (SELECT value FROM json_each(?3))",
    )?;
    let rows = stmt.query_map(rusqlite::params![blob, k as i64, ids_json], |r| {
        r.get::<_, i64>(0)
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

// ── Scoring ────────────────────────────────────────────────────────────────

/// Set-recall of a gold set against a top-k slice, plus whether ANY gold
/// reached the slice (the hit rate, recorded beside the recall, never
/// instead of it).
pub fn score_against(gold: &[String], top: &[String], k: usize) -> (f64, bool) {
    let found = gold
        .iter()
        .filter(|g| top.iter().take(k).any(|t| t == *g))
        .count();
    let recall = if gold.is_empty() {
        0.0
    } else {
        found as f64 / gold.len() as f64
    };
    (recall, found > 0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

// ── The report ─────────────────────────────────────────────────────────────

/// The three arms, by name. Order is the plan's order.
pub const ARM_NAMES: [&str; 3] = ["file-bm25", "chunk+symbol", "fused"];

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ArmRecall {
    pub at_1: f64,
    pub at_5: f64,
    pub at_10: f64,
    pub hit_at_10: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryDetail {
    pub id: String,
    pub has_identifier: bool,
    pub file_bm25: ArmRecall,
    pub chunk_symbol: ArmRecall,
    pub fused: ArmRecall,
    /// Present only for entries that carry gold_symbols.
    pub symbol_recall_at_10: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArmRow {
    /// "file-bm25" | "chunk+symbol" | "fused"
    pub arm: &'static str,
    /// "all" | "with_identifier" | "without"
    pub kind: &'static str,
    pub entries: usize,
    pub recall_at_1: f64,
    pub recall_at_5: f64,
    pub recall_at_10: f64,
    pub hit_at_5: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolRow {
    pub kind: &'static str,
    pub entries: usize,
    pub recall_at_10: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecallReport {
    pub entries: usize,
    pub with_identifier: usize,
    pub without_identifier: usize,
    /// 3 arms × 3 kinds, in ARM_NAMES × kind order.
    pub arms: Vec<ArmRow>,
    pub symbols: Vec<SymbolRow>,
    pub detail: Vec<EntryDetail>,
}

fn arm_recall_of(gold_files: &[String], hits: &[FileHit]) -> ArmRecall {
    let top: Vec<String> = hits.iter().map(|h| h.path.clone()).collect();
    let (r1, _) = score_against(gold_files, &top, 1);
    let (r5, _) = score_against(gold_files, &top, 5);
    let (r10, hit10) = score_against(gold_files, &top, 10);
    ArmRecall {
        at_1: r1,
        at_5: r5,
        at_10: r10,
        hit_at_10: hit10,
    }
}

/// Run the frozen set through the three arms against a built index. The set
/// is caller-supplied (the CLI passes the embedded, hash-pinned fixture);
/// every score is a mean of per-entry set-recall, split by `has_identifier`.
pub fn run_baseline(
    index: &CodeIndex,
    entries: &[RecallEntry],
) -> Result<RecallReport, CodeIndexError> {
    let mut detail = Vec::with_capacity(entries.len());
    for entry in entries {
        let plan = compile_query(&entry.question)?;
        let file_bm25 = arm_file_bm25(index, &plan, FILE_DEPTH)?;
        let chunk_symbol = arm_chunk_symbol(index, &plan, FILE_DEPTH)?;
        let fused = arm_fused(index, &plan, FILE_DEPTH)?;
        let symbol = if entry.gold_symbols.is_empty() {
            None
        } else {
            let hits = arm_symbols(index, &plan, FILE_DEPTH)?;
            let names: Vec<String> = hits.into_iter().map(|h| h.name).collect();
            let (recall, _) = score_against(&entry.gold_symbols, &names, FILE_DEPTH);
            Some(recall)
        };
        detail.push(EntryDetail {
            id: entry.id.clone(),
            has_identifier: entry.has_identifier,
            file_bm25: arm_recall_of(&entry.gold_files, &file_bm25),
            chunk_symbol: arm_recall_of(&entry.gold_files, &chunk_symbol),
            fused: arm_recall_of(&entry.gold_files, &fused),
            symbol_recall_at_10: symbol,
        });
    }

    let kinds: [(&'static str, bool); 2] = [("with_identifier", true), ("without", false)];
    let mut arms = Vec::new();
    for (arm_idx, arm_name) in ARM_NAMES.iter().enumerate() {
        for kind_name in ["all", "with_identifier", "without"] {
            let slice: Vec<&EntryDetail> = detail
                .iter()
                .filter(|d| match kind_name {
                    "all" => true,
                    "with_identifier" => d.has_identifier,
                    _ => !d.has_identifier,
                })
                .collect();
            let pick = |d: &EntryDetail| match arm_idx {
                0 => d.file_bm25,
                1 => d.chunk_symbol,
                _ => d.fused,
            };
            let r1: Vec<f64> = slice.iter().map(|d| pick(d).at_1).collect();
            let r5: Vec<f64> = slice.iter().map(|d| pick(d).at_5).collect();
            let r10: Vec<f64> = slice.iter().map(|d| pick(d).at_10).collect();
            let h5: Vec<f64> = slice
                .iter()
                .map(|d| if pick(d).at_5 > 0.0 { 1.0 } else { 0.0 })
                .collect();
            arms.push(ArmRow {
                arm: arm_name,
                kind: kind_name_kind(kind_name),
                entries: slice.len(),
                recall_at_1: mean(&r1),
                recall_at_5: mean(&r5),
                recall_at_10: mean(&r10),
                hit_at_5: mean(&h5),
            });
        }
    }
    let _ = kinds;

    let mut symbols = Vec::new();
    for kind_name in ["all", "with_identifier", "without"] {
        let slice: Vec<&EntryDetail> = detail
            .iter()
            .filter(|d| match kind_name {
                "all" => d.symbol_recall_at_10.is_some(),
                "with_identifier" => d.has_identifier && d.symbol_recall_at_10.is_some(),
                _ => !d.has_identifier && d.symbol_recall_at_10.is_some(),
            })
            .collect();
        let values: Vec<f64> = slice.iter().filter_map(|d| d.symbol_recall_at_10).collect();
        symbols.push(SymbolRow {
            kind: kind_name_kind(kind_name),
            entries: slice.len(),
            recall_at_10: mean(&values),
        });
    }

    Ok(RecallReport {
        entries: entries.len(),
        with_identifier: detail.iter().filter(|d| d.has_identifier).count(),
        without_identifier: detail.iter().filter(|d| !d.has_identifier).count(),
        arms,
        symbols,
        detail,
    })
}

fn kind_name_kind(kind: &str) -> &'static str {
    match kind {
        "with_identifier" => "with_identifier",
        "without" => "without",
        _ => "all",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder as _;
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

    fn fixture_repo(label: &str, files: &[(&str, &str)]) -> (PathBuf, CodeIndex) {
        let dir = std::env::temp_dir().join(format!(
            "brain-code-core-s6-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        for (path, content) in files {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
        }
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
        let index_path = dir.join("index.db");
        let index = CodeIndex::open(&index_path, &ShimHost).unwrap();
        index.migrate(8).unwrap();
        ingest(&index, &dir, &FakeEncoder::new("test", 8), None).unwrap();
        (dir, index)
    }

    #[test]
    fn the_plan_splits_terms_by_trigram_legality() {
        let plan = compile_query("what does content_hash DO in the \"main\" go path?").unwrap();
        let fts = plan.fts_match.unwrap();
        // Every token is in the unicode61 OR, quoted; a double quote escapes.
        assert!(fts.contains("\"content\""));
        assert!(fts.contains("\"hash\""));
        assert!(fts.contains("\"main\""));
        assert!(fts.contains(" OR "));
        let trigram = plan.trigram_match.unwrap();
        assert!(trigram.contains("\"content\""));
        assert!(
            !trigram.contains("\"go\""),
            "a 2-char term never reaches the trigram MATCH"
        );
        assert_eq!(
            plan.like_terms,
            vec!["do".to_string(), "go".to_string(), "in".to_string()]
        );
    }

    #[test]
    fn a_question_with_no_terms_is_a_named_refusal() {
        let err = compile_query("??? !!! ***").unwrap_err();
        assert!(
            err.to_string().contains("no searchable terms"),
            "the refusal names itself: {err}"
        );
    }

    #[test]
    fn scoring_is_set_recall_plus_hit() {
        let gold = vec!["a.rs".to_string(), "b.rs".to_string()];
        let top = vec!["a.rs".to_string(), "x.rs".to_string(), "b.rs".to_string()];
        let (r1, _) = score_against(&gold, &top, 1);
        let (r3, hit3) = score_against(&gold, &top, 3);
        assert!((r1 - 0.5).abs() < 1e-9, "one of two golds in the top 1");
        assert!((r3 - 1.0).abs() < 1e-9 && hit3, "both golds by depth 3");
        let (_, hit1) = score_against(&gold, &top, 1);
        assert!(hit1);
        let (r0, hit0) = score_against(&gold, &["z.rs".to_string()], 1);
        assert!((r0 - 0.0).abs() < 1e-9 && !hit0);
    }

    #[test]
    fn the_three_arms_rank_the_gold_file_first_for_an_identifier_question() {
        let (_dir, index) = fixture_repo(
            "arms",
            &[
                (
                    "src/fusion.rs",
                    "pub const RRF_K: usize = 60;\n\npub fn rrf_k_fusion(a: &[f64], b: &[f64]) -> f64 {\n    let mut sum = 0.0;\n    for rank in 0..a.len() {\n        sum += 1.0 / (RRF_K + rank as f64 + 1.0);\n    }\n    sum\n}\n",
                ),
                (
                    "src/other.rs",
                    "pub struct UnrelatedThing {\n    pub value: u32,\n}\n",
                ),
            ],
        );
        let question = "where does rrf_k_fusion fuse the ranks?";
        let plan = compile_query(question).unwrap();
        for arm in [
            arm_file_bm25(&index, &plan, FILE_DEPTH),
            arm_chunk_symbol(&index, &plan, FILE_DEPTH),
            arm_fused(&index, &plan, FILE_DEPTH),
        ] {
            let hits = arm.unwrap();
            assert!(!hits.is_empty(), "{question}: every arm returns rows");
            assert_eq!(
                hits[0].path, "src/fusion.rs",
                "{question}: the gold file ranks first"
            );
        }
        // The symbol-level metric reaches the gold symbol by name.
        let symbols = arm_symbols(&index, &plan, FILE_DEPTH).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "rrf_k_fusion"),
            "the symbol leg surfaces the gold symbol"
        );
    }

    #[test]
    fn a_term_shorter_than_three_characters_still_searches() {
        let (_dir, index) = fixture_repo(
            "short",
            &[(
                "src/speed.rs",
                "pub fn go_faster(n: u32) -> u32 {\n    n + 1\n}\n",
            )],
        );
        // The red, recorded: the raw 2-char term in a trigram MATCH is an FTS
        // error — exactly why the plan routes short terms to a bounded LIKE.
        let raw = index
            .conn()
            .prepare("SELECT rowid FROM code_symbol_fts WHERE code_symbol_fts MATCH ?1")
            .unwrap()
            .query_row(["go"], |r| r.get::<_, i64>(0));
        assert!(
            raw.is_err(),
            "the raw short term is an FTS error, not a hit"
        );

        // The law: the term still searches — through the bounded LIKE leg.
        let plan = compile_query("go faster").unwrap();
        assert_eq!(plan.like_terms, vec!["go".to_string()]);
        let symbols = arm_symbols(&index, &plan, FILE_DEPTH).unwrap();
        assert!(
            symbols.iter().any(|s| s.name == "go_faster"),
            "the short identifier still surfaces"
        );
    }

    #[test]
    fn the_knn_id_set_returns_every_matching_row_up_to_k() {
        let (_dir, index) = fixture_repo(
            "knn",
            &[
                (
                    "src/alpha.rs",
                    "pub fn alpha_one() {}\npub fn alpha_two() {}\npub fn alpha_three() {}\n",
                ),
                ("src/beta.rs", "pub fn beta_one() {}\n"),
            ],
        );
        let conn = index.conn();
        let mut file_ids: std::collections::BTreeMap<String, i64> = Default::default();
        {
            let mut stmt = conn.prepare("SELECT path, id FROM code_file").unwrap();
            let mut rows = stmt.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                file_ids.insert(
                    row.get::<_, String>(0).unwrap(),
                    row.get::<_, i64>(1).unwrap(),
                );
            }
        }
        let chunks_of = |path: &str| -> Vec<i64> {
            let mut stmt = conn
                .prepare("SELECT id FROM code_chunk WHERE file_id = ?1 ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([file_ids.get(path).unwrap()], |r| r.get::<_, i64>(0))
                .unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        let alpha_chunks = chunks_of("src/alpha.rs");
        assert_eq!(alpha_chunks.len(), 3, "three symbols, three chunks");

        // The query vector: the encoder's own vector for one of alpha's texts
        // (any vector works — the filter decides the candidate set, and with
        // k = the full set, every matching row must come back).
        let encoder = FakeEncoder::new("test", 8);
        let vector = encoder
            .embed(&["pub fn alpha_one() {}".to_string()])
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let all = knn_chunk_ids(&index, &vector, alpha_chunks.len(), &alpha_chunks).unwrap();
        let mut sorted = all.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted, alpha_chunks,
            "the id-set form returns the full expected count"
        );

        // A tighter k returns exactly k rows — the id set bounds the
        // candidates, the k bounds the answers.
        let two = knn_chunk_ids(&index, &vector, 2, &alpha_chunks).unwrap();
        assert_eq!(two.len(), 2);
        assert!(
            all.iter().take(2).collect::<Vec<_>>() == two.iter().collect::<Vec<_>>()
                || two.iter().all(|id| alpha_chunks.contains(id)),
            "a tighter k is a prefix of the same distance order"
        );

        // An empty id set matches nothing, quietly.
        let none = knn_chunk_ids(&index, &vector, 5, &[]).unwrap();
        assert!(none.is_empty());
    }

    /// The pin-11 spike: a 500-row / 10-file vector store with clustered
    /// vectors written directly (a vector-store drill, not a parse test).
    /// Row (file f, chunk j) sits at axis f with jitter j — distances are
    /// analytic, so every arm's expected count is exact.
    fn spike_store(label: &str) -> CodeIndex {
        let dir = std::env::temp_dir().join(format!(
            "brain-code-core-s7-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("spike.db");
        let index = CodeIndex::open(&path, &ShimHost).unwrap();
        index.migrate(8).unwrap();
        let conn = index.conn();
        conn.execute(
            "INSERT INTO code_repo(root, head_commit, index_version, embed_dim, embed_profile) \
             VALUES ('pin11-spike', 'a', 1, 8, 'spike')",
            [],
        )
        .unwrap();
        let repo_id: i64 = conn
            .query_row("SELECT id FROM code_repo", [], |r| r.get(0))
            .unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for f in 0..10usize {
            tx.execute(
                "INSERT INTO code_file(repo_id, path, lang, content_hash, loc, content) \
                 VALUES (?1, ?2, 'rust', 'h', 1, '')",
                rusqlite::params![repo_id, format!("spike/file_{f}.rs")],
            )
            .unwrap();
            let file_id: i64 = tx
                .query_row("SELECT last_insert_rowid()", [], |r| r.get(0))
                .unwrap();
            for j in 0..50usize {
                tx.execute(
                    "INSERT INTO code_chunk(file_id, kind, text, token_estimate, content_hash) \
                     VALUES (?1, 'symbol', '', 1, 'h')",
                    [file_id],
                )
                .unwrap();
                let chunk_id: i64 = tx
                    .query_row("SELECT last_insert_rowid()", [], |r| r.get(0))
                    .unwrap();
                // One shared axis, via spike_vector: files 0 and 1 sit
                // SYMMETRIC at ±1 (strict 1e-4 offset) so the midpoint query
                // interleaves them — the construction pin 11's drill needs.
                // Files 2..9 sit far out at ±(100+f).
                let vector = spike_vector(f, j);
                let mut blob = Vec::with_capacity(32);
                for value in &vector {
                    blob.extend_from_slice(&value.to_le_bytes());
                }
                tx.execute(
                    "INSERT INTO code_chunk_vec(chunk_id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![chunk_id, blob],
                )
                .unwrap();
            }
        }
        tx.commit().unwrap();
        index
    }

    fn spike_ids(index: &CodeIndex, file: usize) -> Vec<i64> {
        let mut stmt = index
            .conn()
            .prepare(
                "SELECT c.id FROM code_chunk c JOIN code_file f ON f.id = c.file_id \
                 WHERE f.path = ?1 ORDER BY c.id",
            )
            .unwrap();
        let rows = stmt
            .query_map([format!("spike/file_{file}.rs")], |r| r.get::<_, i64>(0))
            .unwrap();
        rows.map(|r| r.unwrap()).collect()
    }

    /// Row (file f, chunk j) sits on axis 0, jitter along the same axis —
    /// distances are analytic. File 0's jitter DESCENDS with its id (its
    /// nearest rows are its HIGHEST ids), so the distance-truth and the
    /// rowid-order of the store disagree — the construction that exposes
    /// what pin 11's drill forms actually do. Files 0 and 1 straddle the
    /// origin symmetrically (±1, strict 1e-4 offset) so the midpoint query
    /// interleaves them; files 2..9 sit far out at ±(100+f).
    fn spike_vector(file: usize, j: usize) -> [f32; 8] {
        let mut v = [0.0f32; 8];
        v[0] = match file {
            0 => {
                1.0 + [0.040f32, 0.001, 0.041, 0.002, 0.042, 0.003, 0.043, 0.004][j % 8]
                    + (j / 8) as f32 * 0.00001
            }
            1 => -1.0 - j as f32 * 0.001 - 0.0001,
            g if g % 2 == 0 => 100.0 + g as f32 + j as f32 * 0.001,
            g => -(100.0 + g as f32) - j as f32 * 0.001,
        };
        v
    }

    /// The query at file `file`'s cluster centre (the zero-jitter point —
    /// for file 0 that is its LAST row).
    fn cluster_query(file: usize) -> Vec<f32> {
        spike_vector(file, if file == 0 { 49 } else { 0 }).to_vec()
    }

    /// The midpoint of files 0 and 1 — the origin, by construction.
    fn midpoint_query() -> Vec<f32> {
        vec![0.0; 8]
    }

    #[test]
    fn the_id_set_form_returns_the_full_expected_count_across_the_five_arms() {
        let index = spike_store("fivearms");
        let file0 = spike_ids(&index, 0);
        assert_eq!(file0.len(), 50);

        // Arm 1: 500 rows / 10 files / k=5, filter = one file's 50 ids.
        let q = cluster_query(0);
        let got = knn_chunk_ids(&index, &q, 5, &file0).unwrap();
        assert_eq!(got.len(), 5, "full expected count");
        assert!(
            got.iter().all(|id| file0.contains(id)),
            "every row is from the filter"
        );

        // Arm 2: clustered vectors, the 5 matching rows chosen OFF-CENTRE
        // (the farthest jitter rows of the cluster — its LOWEST ids here)
        // — still the full count.
        let off_centre: Vec<i64> = file0.iter().take(5).copied().collect();
        let got = knn_chunk_ids(&index, &q, 5, &off_centre).unwrap();
        assert_eq!(got.len(), 5);
        assert!(got.iter().all(|id| off_centre.contains(id)));

        // Arm 3: a 1-row filter, k=5 — one row out, no more, no error.
        let one = vec![file0[0]];
        let got = knn_chunk_ids(&index, &q, 5, &one).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0], file0[0]);

        // Arm 4: k=1 over 199 ids spanning files — one row, from the filter.
        let mut wide: Vec<i64> = Vec::new();
        for f in 0..10usize {
            let ids = spike_ids(&index, f);
            let take = 20.min(if f == 9 { 19 } else { 20 });
            wide.extend(ids.into_iter().take(take));
        }
        assert_eq!(wide.len(), 199);
        let q3 = cluster_query(3);
        let got = knn_chunk_ids(&index, &q3, 1, &wide).unwrap();
        assert_eq!(got.len(), 1);
        assert!(wide.contains(&got[0]));

        // Arm 5: k=1 over 5 ids — one row, from the filter.
        let five: Vec<i64> = file0.iter().take(5).copied().collect();
        let got = knn_chunk_ids(&index, &q, 1, &five).unwrap();
        assert_eq!(got.len(), 1);
        assert!(five.contains(&got[0]));
    }

    #[test]
    fn a_filter_outside_the_knn_lies_or_comes_back_short_that_is_the_red() {
        // Pin 11's drill, kept as a test, in BOTH red shapes observed on
        // this build (sqlite-vec 0.1.9, macOS). The truth at the midpoint
        // query: file 0's nearest five are its five HIGHEST ids (its jitter
        // descends with id), and the global nearest five interleave files
        // 0 and 1 — so a filter applied after k cannot give the right five.
        let index = spike_store("outsidered");
        let file0 = spike_ids(&index, 0);
        let midpoint = midpoint_query();
        let blob: Vec<u8> = midpoint.iter().flat_map(|v| v.to_le_bytes()).collect();
        let ids_json = serde_json::to_string(&file0).unwrap();

        // The green form: the id set INSIDE the KNN — the full, true five.
        let inside = knn_chunk_ids(&index, &midpoint, 5, &file0).unwrap();
        assert_eq!(inside.len(), 5, "the id-set form keeps the full count");
        let mut truth = inside.clone();
        truth.sort_unstable();
        assert_eq!(
            truth,
            vec![2, 10, 18, 26, 34],
            "the true nearest-of-filter is the zigzag order, known analytically"
        );

        // Red shape — the plan's predicted one: the filter applied AFTER a
        // global k, in the caller. sqlite cannot push a filter it cannot
        // see; the global top-5 interleaves files 0 and 1 at the midpoint,
        // so only file 0's share survives — short by construction.
        let mut stmt = index
            .conn()
            .prepare("SELECT chunk_id FROM code_chunk_vec WHERE embedding MATCH ?1 AND k = ?2")
            .unwrap();
        let rows = stmt
            .query_map(rusqlite::params![blob.clone(), 5], |r| r.get::<_, i64>(0))
            .unwrap();
        let global: Vec<i64> = rows.map(|r| r.unwrap()).collect();
        let rust_side: Vec<i64> = global
            .iter()
            .filter(|id| file0.contains(id))
            .copied()
            .collect();
        println!(
            "pin-11 drill, filter-after-k form: {:?} of 5 — short by construction",
            rust_side
        );
        assert!(
            rust_side.len() < 5,
            "filtering after a global k loses rows ({})",
            rust_side.len()
        );
        assert_eq!(
            rust_side,
            vec![2, 10, 18, 26],
            "exactly file 0's share of the global top-5"
        );

        // Platform evidence, recorded — NOT asserted as a failure. The
        // SQL-subquery form (the filter as an outer WHERE after k) came
        // back distance-true on every construction tried on this build:
        // sqlite-vec 0.1.9 pushes the rowid id-set constraint down into
        // the KNN, so even this shape behaves as a filtered KNN here. If
        // CI's platform degrades it, that is a finding, not a failure.
        let mut stmt = index
            .conn()
            .prepare(
                "SELECT chunk_id FROM \
                 (SELECT chunk_id FROM code_chunk_vec WHERE embedding MATCH ?1 AND k = ?2) \
                 WHERE chunk_id IN (SELECT value FROM json_each(?3))",
            )
            .unwrap();
        let rows = stmt
            .query_map(rusqlite::params![blob.clone(), 5, ids_json], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let sql_outer: Vec<i64> = rows.map(|r| r.unwrap()).collect();
        println!(
            "pin-11 evidence, SQL-subquery form on this build: {sql_outer:?} \
             (the truth is {inside:?})"
        );
        assert!(
            !sql_outer.is_empty(),
            "the SQL-subquery form ran and returned rows"
        );
    }

    #[test]
    fn the_metadata_column_form_runs_and_its_result_is_recorded() {
        // Pin 11's second arm, recorded — NOT asserted (R-6 ships the id-set
        // form; the metadata-column form exists only on this scratch table,
        // and its behaviour is platform evidence). On 2026-11-27's macOS
        // build it returns the full matching count; if CI's platform
        // differs, that is a finding, not a failure.
        let index = spike_store("metaform");
        let conn = index.conn();
        conn.execute(
            "CREATE VIRTUAL TABLE pin11_meta USING vec0( \
               id INTEGER PRIMARY KEY, file_tag INTEGER, embedding float[8])",
            [],
        )
        .unwrap();
        let tx = conn.unchecked_transaction().unwrap();
        for (tag, base) in [(0usize, 0.0f32), (1usize, 50.0f32)] {
            for j in 0..10usize {
                let mut vector = [0.0f32; 8];
                vector[tag] = 10.0 + base + j as f32 * 0.001;
                let mut blob = Vec::with_capacity(32);
                for value in &vector {
                    blob.extend_from_slice(&value.to_le_bytes());
                }
                tx.execute(
                    "INSERT INTO pin11_meta(id, file_tag, embedding) VALUES (?1, ?2, ?3)",
                    rusqlite::params![(j + tag * 10) as i64, tag as i64, blob],
                )
                .unwrap();
            }
        }
        tx.commit().unwrap();
        let mut query = [0.0f32; 8];
        query[0] = 10.0;
        let mut blob = Vec::with_capacity(32);
        for value in &query {
            blob.extend_from_slice(&value.to_le_bytes());
        }
        let mut stmt = conn
            .prepare(
                "SELECT id FROM pin11_meta \
                 WHERE embedding MATCH ?1 AND k = 5 AND file_tag = 0",
            )
            .unwrap();
        let rows = stmt.query_map([blob], |r| r.get::<_, i64>(0)).unwrap();
        let got: Vec<i64> = rows.map(|r| r.unwrap()).collect();
        println!(
            "pin-11 metadata-column form: {} of 5 expected rows on this platform",
            got.len()
        );
    }
}
