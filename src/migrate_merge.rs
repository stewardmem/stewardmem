//! `migrate_merge` — the workspace import core (the migrate tool's `merge`
//! mode, and `brain-server --import`). KERNEL-SIDE on purpose: the schema
//! (the current migration set), the content-hash dedupe (the kernel's own
//! UNIQUE index), the tombstone registry, the provenance upserts and the
//! audit chain are the kernel's own facts — a mapper outside would
//! re-implement them (the lie-prone copy). Both binaries are thin argv
//! wrappers over [`run_import`]; every surface prints ONLY the JSON report
//! on stdout (progress on stderr).
//!
//! The laws (the import round, W-I1), one line each: the source opens
//! READ-ONLY by construction; the report lands nothing; `--commit-batch`
//! must carry the report's own (plan-hashed) batch id; a tombstone's
//! content-hash bind means deleted memory does not resurrect; a duplicate
//! lands once or not at all; every landed row carries
//! `origin = 'imported:<batch_id>'` and `--revert-batch` tombstones exactly
//! those rows; ONE audit row per act, the kernel's own chain machinery
//! writing it; embeddings/FTS vocab never cross (the target re-embeds with
//! its own model).

use anyhow::{Context, Result, bail};
use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use crate::audit::{self, AuditKind, AuditStatus};
use crate::storage_layout::refuse_newer_schema;
// (LATEST_KNOWN_SCHEMA is referenced fully-qualified below; kept for the reader)

// ── merge phase (the workspace import) ──────────────────────────────────────
//
//  `merge` — the workspace import: an existing brain's memories come into a
//  TARGET kernel store through an explicit, named, reversible act.
//
//  The laws (the import round, W-I1):
//  - **The source opens READ-ONLY by construction.** [`SourceHandle`] is the
//    only source type in this section: the connection field is private and
//    every method is a read. No `execute`/`commit` is named on any
//    source-side binding (the source-scan pin enforces the text; the
//    behavioral pin hashes the source file before and after a real merge and
//    refuses any delta). The read rides ONE snapshot transaction, so the
//    report describes a single consistent state even if a writer commits
//    mid-read.
//  - **Report first.** Without `--commit-batch` the merge prints the JSON
//    report and lands NOTHING. `--commit-batch <id>` must carry the report's
//    own batch id — the id is a hash of the plan, recomputed at commit time
//    from a fresh snapshot; any drift refuses by name (the act that lands
//    bytes is the act the human saw described).
//  - **The tombstone law.** A tombstone — the source's or the TARGET's —
//    whose `content_hash` equals a row's skips it: deleted memory does not
//    resurrect. Tombstones without a usable digest cannot bind content; they
//    are NAMED in the report (never silently ignored). Ids alone never bind:
//    rowids are reused after deletes (measured), so a kid-only match would
//    false-skip live rows.
//  - **The dedupe.** `(content_hash, domain)` against the target's live
//    rows — the same key the target's own UNIQUE index enforces — so a
//    duplicate lands once or not at all, and the report says which.
//  - **The batch identity.** Every landed knowledge row carries
//    `origin = 'imported:<batch_id>'`; `--revert-batch <id>` tombstones
//    exactly those rows (the forget-path way: digest preserved, a named
//    reason) and removes exactly their edges — one audit row per act, the
//    kernel's own chain machinery writing it.
//  - **Scope.** `knowledge` (+ its `sources`/`source_revisions` provenance),
//    `entities`, `relationships`. The source's embeddings (vec0) and FTS
//    vocab are NOT imported — the target re-embeds and re-indexes with its
//    own model (the FTS triggers index the inserted rows automatically; the
//    kernel's own re-embed flow covers the landed rows). The source's audit
//    chain stays with the source; the import writes ONE audit row in the
//    TARGET. Proposals/recall traces/chat turns are session state, not
//    memory — out of scope.
//  - **governance_hash is recomputed per landed row** from the row's own
//    `(owner, access_scope, domain)` via the kernel's own
//    `service::ingest::governance_hash` — the register's every-INSERT-names-
//    the-hash law applies to the import like to every other write path.

/// The tables the merge maps. The schema-delta check re-derives their
/// columns from BOTH live connections at run time and refuses by name on any
/// drift — the mapping is the identity, held by measurement, not by hope.
const MERGE_TABLES: &[&str] = &[
    "knowledge",
    "entities",
    "relationships",
    "sources",
    "source_revisions",
];

/// The source rows the merge reads, as tuples (clippy type-alias shape).
type SourceSourceRow = (
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);
type SourceRevisionRow = (
    i64,
    i64,
    String,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<String>,
    Option<String>,
);
type SourceEdgeRow = (
    i64,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);
type SourceEntityRow = (i64, String, Option<String>, Option<String>);

/// One import act, fully named: the source store (read-only, by name), the
/// target store (chosen by name — never a layout default), and the batch
/// decision (None = the report; Some = the commit; `revert_batch` = the
/// undo). The argv parsers in both binaries enforce "both paths explicit";
/// this type carries the parsed act.
#[derive(Debug)]
pub struct ImportInvocation {
    /// `None` for a revert (it reads only the target).
    pub source: Option<PathBuf>,
    pub dest: PathBuf,
    pub commit_batch: Option<String>,
    pub revert_batch: Option<String>,
}

/// The argv parser for `brain-server --import …` (and the migrate tool's
/// own merge arm shares the shape): both paths REQUIRED — the import target
/// is chosen by name, never a layout default. `--import` itself and
/// `--dry-run` are accepted and inert (the report IS the dry run).
pub fn parse_import_argv(argv: &[String]) -> Result<ImportInvocation> {
    let mut source: Option<PathBuf> = None;
    let mut dest: Option<PathBuf> = None;
    let mut commit_batch: Option<String> = None;
    let mut revert_batch: Option<String> = None;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--import" | "--dry-run" => {}
            "--source" => {
                i += 1;
                source = Some(argv.get(i).context("--source requires a value")?.into());
            }
            "--dest" => {
                i += 1;
                dest = Some(argv.get(i).context("--dest requires a value")?.into());
            }
            "--commit-batch" => {
                i += 1;
                commit_batch = Some(
                    argv.get(i)
                        .context("--commit-batch requires a value")?
                        .clone(),
                );
            }
            "--revert-batch" => {
                i += 1;
                revert_batch = Some(
                    argv.get(i)
                        .context("--revert-batch requires a value")?
                        .clone(),
                );
            }
            other => bail!("unknown import flag: {other}"),
        }
        i += 1;
    }
    if revert_batch.is_some() && commit_batch.is_some() {
        bail!("--revert-batch runs alone: it needs --dest and nothing else");
    }
    // The argv contract: the merge names its store; only a revert may omit
    // --source (it reads the target alone). run_import re-checks — defense
    // in depth, but THIS is where the operator's typo gets named.
    if source.is_none() && revert_batch.is_none() {
        bail!("--source is required — the import store is chosen by name, never discovered");
    }
    Ok(ImportInvocation {
        source,
        dest: dest.context(
            "--dest is required — the import target is chosen by name, never a layout default",
        )?,
        commit_batch,
        revert_batch,
    })
}

/// The schema-refusal sentinel: the report (with the named refusal) is
/// already on stdout; the caller maps this to exit code 1 without printing
/// a second reason.
pub const SCHEMA_REFUSED: &str = "schema-refused";

/// The shared entry point: run the invocation, print the JSON on stdout,
/// return the process exit code (0 = the act completed as described, 1 =
/// refused or failed — the reason is on stderr AND, for a schema refusal,
/// in the report's own `verdict`/`refusal` fields).
pub fn run_import(inv: &ImportInvocation) -> Result<i32> {
    if let Some(id) = inv.revert_batch.as_deref() {
        return match phase_revert_batch(&inv.dest, id) {
            Ok(()) => Ok(0),
            Err(e) => {
                eprintln!("import: {e:#}");
                Ok(1)
            }
        };
    }
    if inv.source.is_none() {
        bail!(
            "--source is required for a merge — the import store is chosen by name, never discovered"
        );
    }
    match phase_merge(inv, inv.commit_batch.as_deref()) {
        Ok(()) => Ok(0),
        Err(e) => {
            if e.to_string() != SCHEMA_REFUSED {
                eprintln!("import: {e:#}");
            }
            Ok(1)
        }
    }
}

/// The source store's ONLY handle in the merge section: the field is private
/// and every method is a read. The read-only law is a TYPE here, not a
/// convention — the merge cannot write the source because this type offers
/// no write. `open_read_only` is the only constructor; the behavioral pin
/// hashes the source file before and after a real merge.
pub struct SourceHandle<'a> {
    conn: &'a Connection,
}

impl<'a> SourceHandle<'a> {
    /// The ONLY way a source store is opened in the merge: the OS-level
    /// read-only open. Deliberately NOT `immutable=1` — a live store's WAL
    /// must stay visible (an immutable read silently misses committed rows);
    /// snapshot consistency comes from the one read transaction instead.
    fn open_read_only(path: &Path) -> Result<Connection> {
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("open source READ-ONLY {}", path.display()))
    }

    fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Every read the merge makes rides ONE snapshot transaction: the whole
    /// plan describes a single consistent state of the source. The
    /// transaction is ended by drop — a read transaction holds no writes, so
    /// `commit` is deliberately never named on the source side. DEFERRED is
    /// correct here BY LAW OF USE: this transaction never writes, so it can
    /// never upgrade and never deadlock (the write-discipline inventory
    /// carries this file's single read-snapshot site).
    fn snapshot<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let tx = self.conn.unchecked_transaction()?;
        let out = f(tx.deref())?;
        drop(tx);
        Ok(out)
    }

    fn count(&self, table: &str) -> Result<i64> {
        // table names come from MERGE_TABLES / the fixture tests — never input.
        let sql = format!("SELECT COUNT(*) FROM {table}");
        self.snapshot(|c| Ok(c.query_row(&sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)))
    }

    /// `(name, type)` per column, from the live schema — the merge maps by
    /// NAME, so this is both the delta check's input and the INSERT's shape.
    fn columns(&self, table: &str) -> Result<Vec<(String, String)>> {
        let sql = format!("PRAGMA table_info({table})");
        self.snapshot(|c| {
            let mut stmt = c.prepare(&sql)?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
            Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
        })
    }

    fn schema_version(&self) -> Option<String> {
        crate::storage_layout::schema_version(self.conn)
    }
}

// ── merge report shapes (the JSON the shell parses — the only surface) ─────

#[derive(Serialize)]
pub struct MergeReport {
    mode: &'static str,
    verdict: &'static str, // "planned" | "refused" | "committed" | "reverted"
    source: StoreFacts,
    target: StoreFacts,
    schema_delta: SchemaDelta,
    plan: Option<MergePlan>,
    outcome: Option<LandOutcome>,
    notes: Vec<&'static str>,
}

#[derive(Serialize)]
pub struct StoreFacts {
    path: String,
    schema_version: Option<String>,
    size_bytes: u64,
    counts: BTreeMap<String, i64>,
}

#[derive(Serialize)]
pub struct SchemaDelta {
    /// "identity" (every source column maps by name and type) | "refused".
    verdict: &'static str,
    /// table → the source columns the merge would carry over (by name).
    mapping: BTreeMap<String, Vec<String>>,
    refusal: Option<NamedRefusal>,
}

#[derive(Serialize)]
pub struct NamedRefusal {
    table: String,
    column: String,
    reason: String,
}

#[derive(Serialize)]
pub struct MergePlan {
    batch_id: String,
    knowledge: KnowledgePlan,
    entities: EntityPlan,
    relationships: RelPlan,
    sources: SourcePlan,
    source_revisions: RevisionPlan,
}

#[derive(Serialize)]
pub struct KnowledgePlan {
    source_total: usize,
    to_land: usize,
    without_content_hash: usize,
    dedupe_live: usize,
    dedupe_target_tombstone: usize,
    source_tombstoned: usize,
    quarantined_to_land: usize,
    /// Named, cannot bind: tombstones with no digest at all.
    tombstones_without_digest: i64,
    /// Named, cannot bind: tombstones whose digest is not the knowledge
    /// content-hash scheme (the purge path's sha256).
    tombstone_digest_scheme_mismatch: i64,
    /// Named: tombstoned ids that ALSO name a live row (rowid reuse) — the
    /// class a kid-only match would false-skip.
    tombstone_ids_reused_live: i64,
}

#[derive(Serialize)]
pub struct EntityPlan {
    referenced_by_landed_edges: usize,
}

#[derive(Serialize)]
pub struct RelPlan {
    source_total: usize,
    to_land: usize,
    skipped_chunk_not_landed: usize,
}

#[derive(Serialize)]
pub struct SourcePlan {
    referenced_by_landed_rows: usize,
}

#[derive(Serialize)]
pub struct RevisionPlan {
    referenced_by_landed_rows: usize,
}

#[derive(Serialize)]
pub struct LandOutcome {
    knowledge_landed: usize,
    knowledge_unique_conflicts: usize,
    entities_landed: usize,
    entities_reused: usize,
    relationships_landed: usize,
    relationships_existing: usize,
    sources_landed: usize,
    sources_existing: usize,
    revisions_landed: usize,
    revisions_existing: usize,
    audit_row: Option<i64>,
    source_sha256_unchanged: bool,
}

/// The plan's private carry-over: what lands, by source id — plus the land
/// set's content hashes, which the commit re-checks against its own fetches
/// (the source is live; a row changed between the plan's snapshot and the
/// commit's fetch must refuse, not land different bytes under the same id).
pub struct PlanData {
    land_ids: Vec<i64>,
    land_hashes: Vec<String>,
    source_ids_needed: Vec<i64>,
    revision_ids_needed: Vec<i64>,
}

// ── merge dispatch ──────────────────────────────────────────────────────────

pub fn phase_merge(args: &ImportInvocation, commit_batch: Option<&str>) -> Result<()> {
    let source: &Path = args
        .source
        .as_deref()
        .context("--source is required for a merge")?;
    confirm_source_exists(source)?;
    if source == args.dest || same_file(source, &args.dest) {
        bail!(
            "merge refuses a target that IS the source ({})",
            args.dest.display()
        );
    }
    confirm_source_exists(&args.dest).with_context(|| {
        format!(
            "target not found: {} — boot the kernel once to create its store; \
             the merge never creates a target",
            args.dest.display()
        )
    })?;

    // The source: read-only by construction, one snapshot for everything.
    let src_conn = SourceHandle::open_read_only(source)?;
    let src = SourceHandle::new(&src_conn);
    let src_ver = src.schema_version();
    refuse_newer_schema(src_ver.as_deref()).with_context(|| {
        format!(
            "refusing to merge {} (stamped {})",
            source.display(),
            src_ver.as_deref().unwrap_or("unstamped")
        )
    })?;

    // The target: read-write (the merge lands there), schema-gated.
    let mut tgt_conn = Connection::open(&args.dest)
        .with_context(|| format!("open target {}", args.dest.display()))?;
    tgt_conn
        .pragma_update(None, "foreign_keys", "ON")
        .context("enable foreign_keys on target")?;
    let tgt_ver = crate::storage_layout::schema_version(&tgt_conn);
    refuse_newer_schema(tgt_ver.as_deref()).with_context(|| {
        format!(
            "refusing target {} (stamped {})",
            args.dest.display(),
            tgt_ver.as_deref().unwrap_or("unstamped")
        )
    })?;
    if tgt_ver.as_deref() != Some(crate::storage_layout::LATEST_KNOWN_SCHEMA) {
        bail!(
            "target {} is stamped {:?}, not the current {} — boot the kernel once \
             to bring it current; the merge never migrates the target itself",
            args.dest.display(),
            tgt_ver,
            crate::storage_layout::LATEST_KNOWN_SCHEMA
        );
    }

    // ── the schema-delta verdict (pins first; the refusal is fail-closed) ──
    let (delta, mapping) = schema_delta(&src, &tgt_conn)?;
    if delta.verdict == "refused" {
        let report = MergeReport {
            mode: "merge",
            verdict: "refused",
            source: store_facts(source, &src, &[])?,
            target: target_facts(&args.dest, &tgt_conn)?,
            schema_delta: delta,
            plan: None,
            outcome: None,
            notes: vec![
                "the merge refuses a schema it cannot map by name — a wrong-shaped merge is worse than none",
            ],
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Err(anyhow::anyhow!(SCHEMA_REFUSED));
    }

    // ── the plan (one snapshot) ─────────────────────────────────────────────
    let (report, plan_data) = plan_merge(source, &args.dest, &src, &tgt_conn, delta)?;
    match commit_batch {
        None => {
            // The report IS the dry run: nothing has landed.
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        Some(stated) => {
            if stated
                != report
                    .plan
                    .as_ref()
                    .map(|p| p.batch_id.as_str())
                    .unwrap_or("")
            {
                bail!(
                    "the stated batch id {stated:?} is not the plan's own id {:?} — \
                     the source or the target moved since the report; re-run the \
                     report and commit THAT id (never a stale one)",
                    report
                        .plan
                        .as_ref()
                        .map(|p| p.batch_id.as_str())
                        .unwrap_or("?")
                );
            }
            let plan = report.plan.expect("plan exists when the id matched");
            let outcome = commit_merge(
                source,
                &args.dest,
                &src,
                &mut tgt_conn,
                &mapping,
                &plan_data,
                &plan,
            )?;
            let done = MergeReport {
                mode: "merge",
                verdict: "committed",
                source: report.source,
                target: report.target,
                schema_delta: report.schema_delta,
                plan: Some(plan),
                outcome: Some(outcome),
                notes: report.notes,
            };
            println!("{}", serde_json::to_string_pretty(&done)?);
            Ok(())
        }
    }
}

/// True when two paths name the same file, so `--dest` cannot be a
/// symlink/hardlink alias of `--source`.
pub fn same_file(a: &Path, b: &Path) -> bool {
    #[allow(clippy::needless_return)] // the two cfg arms return differently
    {
        // Unix: dev+inode (symlinks and hardlinks both caught).
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            return match (fs::metadata(a), fs::metadata(b)) {
                (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
                _ => false,
            };
        }
        // Elsewhere: canonical paths.
        #[cfg(not(unix))]
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(ca), Ok(cb)) => ca == cb,
            _ => false,
        }
    }
}

pub fn store_facts(path: &Path, src: &SourceHandle, tables: &[&str]) -> Result<StoreFacts> {
    let mut counts = BTreeMap::new();
    for t in tables {
        counts.insert((*t).to_string(), src.count(t)?);
    }
    counts.insert("tombstones".into(), src.count("tombstones")?);
    counts.insert("audit_events".into(), src.count("audit_events")?);
    Ok(StoreFacts {
        path: path.display().to_string(),
        schema_version: src.schema_version(),
        size_bytes: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        counts,
    })
}

pub fn target_facts(path: &Path, conn: &Connection) -> Result<StoreFacts> {
    let mut counts = BTreeMap::new();
    for t in MERGE_TABLES {
        let n: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap_or(0);
        counts.insert((*t).to_string(), n);
    }
    Ok(StoreFacts {
        path: path.display().to_string(),
        schema_version: crate::storage_layout::schema_version(conn),
        size_bytes: fs::metadata(path).map(|m| m.len()).unwrap_or(0),
        counts,
    })
}

/// The schema-delta check: re-derived from BOTH live connections. The mapping
/// is the identity — every source column must exist in the target with the
/// same declared type. A source column the target lacks, or a type that
/// moved, is a REFUSAL that names table and column. A target-only column is
/// mechanical when it carries a default (the INSERT omits it; the schema
/// fills it) and a refusal when it is NOT NULL with none.
pub fn schema_delta(
    src: &SourceHandle,
    tgt: &Connection,
) -> Result<(SchemaDelta, BTreeMap<String, Vec<String>>)> {
    let mut mapping = BTreeMap::new();
    for table in MERGE_TABLES {
        let s_cols = src.columns(table)?;
        let mut t_map: HashMap<String, String> = HashMap::new();
        {
            let sql = format!("PRAGMA table_info({table})");
            let mut stmt = tgt.prepare(&sql)?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?;
            for row in rows {
                let (name, ty, notnull, _dflt) = row?;
                t_map.insert(name, ty);
                let _ = notnull;
            }
        }
        // Target-only columns: mechanical with a default, refusal without.
        {
            let sql = format!("PRAGMA table_info({table})");
            let mut stmt = tgt.prepare(&sql)?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?;
            for row in rows {
                let (name, notnull, dflt) = row?;
                let in_source = s_cols.iter().any(|(n, _)| *n == name);
                if !in_source && notnull == 1 && dflt.is_none() && name != "id" {
                    return Ok((
                        SchemaDelta {
                            verdict: "refused",
                            mapping: mapping.clone(),
                            refusal: Some(NamedRefusal {
                                table: (*table).to_string(),
                                column: name,
                                reason: "target-only column, NOT NULL with no default — \
                                         the merge refuses to invent a value"
                                    .into(),
                            }),
                        },
                        mapping,
                    ));
                }
            }
        }
        let mut cols = Vec::new();
        for (name, s_ty) in &s_cols {
            match t_map.get(name) {
                None => {
                    return Ok((
                        SchemaDelta {
                            verdict: "refused",
                            mapping: mapping.clone(),
                            refusal: Some(NamedRefusal {
                                table: (*table).to_string(),
                                column: name.clone(),
                                reason: "column exists in the source but not the target — \
                                         a semantic delta the merge refuses to guess"
                                    .into(),
                            }),
                        },
                        mapping,
                    ));
                }
                Some(t_ty) if t_ty != s_ty => {
                    return Ok((
                        SchemaDelta {
                            verdict: "refused",
                            mapping: mapping.clone(),
                            refusal: Some(NamedRefusal {
                                table: (*table).to_string(),
                                column: name.clone(),
                                reason: format!(
                                    "column type moved: source {s_ty:?} vs target {t_ty:?}"
                                ),
                            }),
                        },
                        mapping,
                    ));
                }
                Some(_) => cols.push(name.clone()),
            }
        }
        mapping.insert((*table).to_string(), cols);
    }
    Ok((
        SchemaDelta {
            verdict: "identity",
            mapping: mapping.clone(),
            refusal: None,
        },
        mapping,
    ))
}

/// The plan: tombstone binds, dedupe, the land set, the batch id. Reads the
/// source through ONE snapshot; reads the target directly (the caller owns
/// the target's consistency).
pub fn plan_merge(
    source_path: &Path,
    dest_path: &Path,
    src: &SourceHandle,
    tgt: &Connection,
    delta: SchemaDelta,
) -> Result<(MergeReport, PlanData)> {
    // Target-side sets: live (content_hash, domain) + target tombstone hashes.
    let mut live: HashSet<(String, String)> = HashSet::new();
    {
        let mut stmt = tgt.prepare("SELECT content_hash, domain FROM knowledge")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        })?;
        for row in rows {
            let (h, d) = row?;
            if let (Some(h), Some(d)) = (h, d) {
                live.insert((h, d));
            }
        }
    }
    let tgt_tombstones: HashSet<String> = tombstone_hashes(tgt)?;

    // Source-side snapshot: counts, tombstone facts, then the row walk.
    let (plan, data, source_counts) = src.snapshot(|c| {
        let src_tombs_all: i64 =
            c.query_row("SELECT COUNT(*) FROM tombstones", [], |r| r.get(0))?;
        let without_digest: i64 = c.query_row(
            "SELECT COUNT(*) FROM tombstones WHERE content_hash IS NULL",
            [],
            |r| r.get(0),
        )?;
        let scheme_mismatch: i64 = c.query_row(
            "SELECT COUNT(*) FROM tombstones WHERE content_hash IS NOT NULL \
             AND LENGTH(content_hash) != 16",
            [],
            |r| r.get(0),
        )?;
        let reused_live: i64 = c.query_row(
            "SELECT COUNT(*) FROM tombstones WHERE content_hash IS NULL \
             AND knowledge_id IN (SELECT id FROM knowledge)",
            [],
            |r| r.get(0),
        )?;
        let src_tomb_hashes = tombstone_hashes(c)?;

        let total: i64 = c.query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))?;
        let rel_total: i64 = c.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?;

        // The row walk: light rows only (no content) — id, hash, domain,
        // flagged, provenance refs.
        let mut without_hash = 0usize;
        let mut dedupe_live = 0usize;
        let mut dedupe_target_tomb = 0usize;
        let mut src_tombstoned = 0usize;
        let mut land_ids: Vec<i64> = Vec::new();
        let mut land_hashes: Vec<String> = Vec::new();
        let mut quarantined = 0usize;
        let mut source_ids: HashSet<i64> = HashSet::new();
        let mut revision_ids: HashSet<i64> = HashSet::new();
        {
            let mut stmt = c.prepare(
                "SELECT id, content_hash, domain, flagged, source_id, revision_id
                 FROM knowledge ORDER BY id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                ))
            })?;
            for row in rows {
                let (id, hash, domain, flagged, sid, rid) = row?;
                let (Some(hash), Some(domain)) = (hash, domain) else {
                    without_hash += 1;
                    continue;
                };
                if src_tomb_hashes.contains(&hash) || tgt_tombstones.contains(&hash) {
                    if src_tomb_hashes.contains(&hash) {
                        src_tombstoned += 1;
                    }
                    if tgt_tombstones.contains(&hash) {
                        dedupe_target_tomb += 1;
                    }
                    continue;
                }
                if live.contains(&(hash.clone(), domain.clone())) {
                    dedupe_live += 1;
                    continue;
                }
                if flagged != 0 {
                    quarantined += 1;
                }
                if let Some(s) = sid {
                    source_ids.insert(s);
                }
                if let Some(rv) = rid {
                    revision_ids.insert(rv);
                }
                land_ids.push(id);
                land_hashes.push(hash);
            }
        }

        // Relationships ride their chunk: an edge lands only when its
        // knowledge row lands — no fabricated linkage onto deduped twins.
        let mut edge_chunk_ids = 0usize;
        for chunk in land_ids.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT COUNT(*) FROM relationships WHERE knowledge_id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            edge_chunk_ids += stmt.query_row(params.as_slice(), |r| r.get::<_, i64>(0))? as usize;
        }
        let skipped_rels = (rel_total as usize).saturating_sub(edge_chunk_ids);

        // Entities: the endpoints of the edges that will land.
        let mut entity_refs: HashSet<i64> = HashSet::new();
        for chunk in land_ids.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT from_entity_id, to_entity_id FROM relationships
                 WHERE knowledge_id IN ({placeholders})
                   AND from_entity_id IS NOT NULL AND to_entity_id IS NOT NULL"
            );
            let mut stmt = c.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                chunk.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (a, b) = row?;
                entity_refs.insert(a);
                entity_refs.insert(b);
            }
        }

        let plan = MergePlan {
            batch_id: String::new(), // computed outside (needs the hash set)
            knowledge: KnowledgePlan {
                source_total: total as usize,
                to_land: land_ids.len(),
                without_content_hash: without_hash,
                dedupe_live,
                dedupe_target_tombstone: dedupe_target_tomb,
                source_tombstoned: src_tombstoned,
                quarantined_to_land: quarantined,
                tombstones_without_digest: without_digest,
                tombstone_digest_scheme_mismatch: scheme_mismatch,
                tombstone_ids_reused_live: reused_live,
            },
            entities: EntityPlan {
                referenced_by_landed_edges: entity_refs.len(),
            },
            relationships: RelPlan {
                source_total: rel_total as usize,
                to_land: edge_chunk_ids,
                skipped_chunk_not_landed: skipped_rels,
            },
            sources: SourcePlan {
                referenced_by_landed_rows: source_ids.len(),
            },
            source_revisions: RevisionPlan {
                referenced_by_landed_rows: revision_ids.len(),
            },
        };
        let mut source_counts = BTreeMap::new();
        for t in MERGE_TABLES {
            let n: i64 = c
                .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap_or(0);
            source_counts.insert((*t).to_string(), n);
        }
        source_counts.insert("tombstones".into(), src_tombs_all);
        let audit_n: i64 = c
            .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
            .unwrap_or(0);
        source_counts.insert("audit_events".into(), audit_n);
        Ok((
            plan,
            PlanData {
                land_ids,
                land_hashes,
                source_ids_needed: source_ids.into_iter().collect(),
                revision_ids_needed: revision_ids.into_iter().collect(),
            },
            source_counts,
        ))
    })?;

    // The batch id: a hash of the plan itself — the same state produces the
    // same id, so the commit's recomputation binds the landed bytes to the
    // described act.
    let batch_id = compute_batch_id(source_path, dest_path, &plan, &data.land_hashes)?;

    let report = MergeReport {
        mode: "merge",
        verdict: "planned",
        source: StoreFacts {
            path: source_path.display().to_string(),
            schema_version: src.schema_version(),
            size_bytes: fs::metadata(source_path).map(|m| m.len()).unwrap_or(0),
            counts: source_counts,
        },
        target: target_facts(dest_path, tgt)?,
        schema_delta: delta,
        plan: Some(plan),
        outcome: None,
        notes: vec![
            "embeddings and FTS vocab are NOT imported — the target re-embeds and re-indexes with its own model",
            "the source's audit chain stays with the source; the import writes ONE audit row in the target",
            "quarantined (flagged) rows land as quarantined — the review queue still governs them",
            "revert any time: brain-migrate-rehearse merge --dest <target> --revert-batch <batch_id>",
            "landed rows carry origin 'imported:<batch_id>' — the batch stamp is the provenance; the source's own origin values are not preserved",
            "the commit writes a batch receipt (.import-batch-<batch_id>.json beside the target) — the revert reads it; keep the two together",
        ],
    };
    Ok((report_with_batch(report, batch_id), data))
}

fn report_with_batch(mut r: MergeReport, batch_id: String) -> MergeReport {
    if let Some(p) = r.plan.as_mut() {
        p.batch_id = batch_id;
    }
    r
}

/// sha256 over the plan's identity: source path, target path, the sorted
/// land hashes, and every count the report states. `wi-` + 16 hex.
pub fn compute_batch_id(
    source_path: &Path,
    dest_path: &Path,
    plan: &MergePlan,
    land_hashes: &[String],
) -> Result<String> {
    let mut hashes = land_hashes.to_vec();
    hashes.sort();
    let mut h = Sha256::new();
    h.update(b"brain-import-batch-v1");
    h.update(source_path.to_string_lossy().as_bytes());
    h.update(b"\x00");
    h.update(dest_path.to_string_lossy().as_bytes());
    h.update(b"\x00");
    h.update(hashes.join("\n").as_bytes());
    h.update(
        format!(
            "|k{}/{}|e{}|r{}/{}|s{}|v{}",
            plan.knowledge.to_land,
            plan.knowledge.source_total,
            plan.entities.referenced_by_landed_edges,
            plan.relationships.to_land,
            plan.relationships.skipped_chunk_not_landed,
            plan.sources.referenced_by_landed_rows,
            plan.source_revisions.referenced_by_landed_rows
        )
        .as_bytes(),
    );
    Ok(format!("wi-{}", hex::encode(&h.finalize()[..8])))
}

/// The tombstone bind set: every tombstone content_hash in the store. All
/// schemes go in — a sha256-era digest can never equal a 16-hex content hash,
/// so including it is harmless and the counts stay in the report.
pub fn tombstone_hashes(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT content_hash FROM tombstones WHERE content_hash IS NOT NULL")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

// ── the commit ──────────────────────────────────────────────────────────────

pub fn commit_merge(
    source: &Path,
    dest: &Path,
    src: &SourceHandle,
    tgt_conn: &mut Connection,
    mapping: &BTreeMap<String, Vec<String>>,
    data: &PlanData,
    plan: &MergePlan,
) -> Result<LandOutcome> {
    let src_sha_before = sha256_and_size(source)?.0;

    let mut outcome = LandOutcome {
        knowledge_landed: 0,
        knowledge_unique_conflicts: 0,
        entities_landed: 0,
        entities_reused: 0,
        relationships_landed: 0,
        relationships_existing: 0,
        sources_landed: 0,
        sources_existing: 0,
        revisions_landed: 0,
        revisions_existing: 0,
        audit_row: None,
        source_sha256_unchanged: true,
    };

    let batch_id = plan.batch_id.clone();
    let origin_stamp = format!("imported:{batch_id}");

    // ONE transaction: provenance, knowledge, edges, the audit row — the
    // batch lands whole or not at all. BEGIN IMMEDIATE (the write
    // discipline): a running kernel may hold the target's write lock, and a
    // DEFERRED transaction that reads first then writes would deadlock on
    // the upgrade instead of failing fast.
    let tx = tgt_conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("begin target transaction")?;

    // Sources and revisions first: knowledge.source_id/revision_id carry FKs,
    // so the provenance rows must exist (mapped) before the knowledge rows.
    let source_map = upsert_sources(src, &tx, &data.source_ids_needed, &mut outcome)?;
    let revision_map = upsert_revisions(
        src,
        &tx,
        &data.revision_ids_needed,
        &source_map,
        &mut outcome,
    )?;

    // ── knowledge ───────────────────────────────────────────────────────────
    let k_cols: &Vec<String> = mapping
        .get("knowledge")
        .context("schema delta checked; knowledge mapping exists")?;
    let mut insert_cols: Vec<String> = k_cols
        .iter()
        .filter(|c| c.as_str() != "id")
        .cloned()
        .collect();
    // The PII flag rides by name from any source that carries it (every
    // store since the column was added does). A source predating the pii
    // column lacks it — and the schema default would turn read-time
    // redaction OFF for every landed row (the exact hazard the pii pin
    // names). So the merge computes the flag through the kernel's own gate
    // for such rows, explicitly.
    let pii_computed = !insert_cols.iter().any(|c| c == "pii");
    if pii_computed {
        insert_cols.push("pii".to_string());
    }
    let col_sql = insert_cols.join(", ");
    let params_sql = (1..=insert_cols.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!("INSERT INTO knowledge ({col_sql}) VALUES ({params_sql})");
    let gov_idx = insert_cols
        .iter()
        .position(|c| c == "governance_hash")
        .context("knowledge carries governance_hash (register row 129)")?;
    let origin_idx = insert_cols
        .iter()
        .position(|c| c == "origin")
        .context("knowledge carries origin (the batch stamp)")?;

    // source knowledge id → target id, captured at insert time.
    let mut kid_map: HashMap<i64, i64> = HashMap::new();
    let mut fetched_hashes: Vec<String> = Vec::with_capacity(data.land_hashes.len());

    for chunk in data.land_ids.chunks(500) {
        // The full rows, every mapped column by name, from the read-only source.
        let rows = fetch_source_knowledge_rows(src, chunk)?;
        for row in rows {
            let sid: i64 = match row.get("id") {
                Some(Value::Integer(i)) => *i,
                _ => bail!("landed row without an id — the plan's ids are the source's own"),
            };
            match row.get("content_hash") {
                Some(Value::Text(h)) => fetched_hashes.push(h.clone()),
                _ => bail!("landed row {sid} lost its content hash between plan and fetch"),
            }
            let mut binds: Vec<Value> = Vec::with_capacity(insert_cols.len());
            for col in &insert_cols {
                if col == "governance_hash" {
                    binds.push(Value::Null); // bound below, recomputed
                    continue;
                }
                if pii_computed && col == "pii" {
                    // Source predating the pii column: screen through the kernel's gate.
                    let title = value_text(row.get("title").cloned()).unwrap_or_default();
                    let content = value_text(row.get("content").cloned()).unwrap_or_default();
                    binds.push(Value::Integer(i64::from(crate::gate::pii_flag(
                        &title, &content,
                    ))));
                    continue;
                }
                let mut v = row.get(col).cloned().unwrap_or(Value::Null);
                // Provenance FKs: the source's ids name the SOURCE's rows;
                // the landed row must name the TARGET's (mapped above).
                if col == "source_id" {
                    v = match v {
                        Value::Integer(sid) => match source_map.get(&sid) {
                            Some(t) => Value::Integer(*t),
                            None => bail!(
                                "source row {sid}: provenance source {sid} did not map —                                      the plan and the commit read different snapshots"
                            ),
                        },
                        _ => Value::Null,
                    };
                }
                if col == "revision_id" {
                    v = match v {
                        Value::Integer(rid) => match revision_map.get(&rid) {
                            Some(t) => Value::Integer(*t),
                            None => bail!(
                                "source row: revision {rid} did not map —                                      the plan and the commit read different snapshots"
                            ),
                        },
                        _ => Value::Null,
                    };
                }
                binds.push(v);
            }
            let owner = value_text(row.get("owner").cloned());
            let scope =
                value_text(row.get("access_scope").cloned()).unwrap_or_else(|| "private".into());
            let domain = value_text(row.get("domain").cloned()).unwrap_or_else(|| "global".into());
            binds[gov_idx] = Value::Text(crate::service::ingest::governance_hash(
                owner.as_deref(),
                &scope,
                &domain,
            ));
            binds[origin_idx] = Value::Text(origin_stamp.clone());
            match tx.execute(&insert_sql, rusqlite::params_from_iter(binds.iter())) {
                Ok(_) => {
                    kid_map.insert(sid, tx.last_insert_rowid());
                    outcome.knowledge_landed += 1;
                }
                Err(rusqlite::Error::SqliteFailure(e, Some(msg)))
                    if e.extended_code == 2067 /* SQLITE_CONSTRAINT_UNIQUE */ =>
                {
                    // The dedupe backstop (content_hash+domain, ump_id,
                    // published slug): a named skip, never a crash — and
                    // NEVER a blanket ConstraintViolation catch (an FK
                    // failure here is a bug, not a skip).
                    outcome.knowledge_unique_conflicts += 1;
                    eprintln!("merge: unique conflict skipped: {msg}");
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(e))
                        .context(format!("insert knowledge from source row {sid}"))
                }
            }
        }
        eprintln!(
            "merge: knowledge {}/{}",
            outcome.knowledge_landed + outcome.knowledge_unique_conflicts,
            data.land_ids.len()
        );
    }

    // The fetched rows must be the planned ones: the source is LIVE, and a
    // row changed between the plan's snapshot and this fetch must refuse,
    // not land different bytes under the same batch id.
    let mut fetched_sorted = fetched_hashes.clone();
    fetched_sorted.sort();
    let mut planned_sorted = data.land_hashes.clone();
    planned_sorted.sort();
    if fetched_sorted != planned_sorted {
        bail!(
            "the source moved between the report and this commit ({} fetched vs              {} planned hashes match) — the batch id no longer describes what              would land; re-run the report",
            fetched_sorted.len(),
            planned_sorted.len()
        );
    }

    // ── entities + relationships ────────────────────────────────────────────
    let created_entities = land_edges(src, &tx, &data.land_ids, &kid_map, &mut outcome)?;

    // ── the ONE audit row (the kernel's own chain machinery) ────────────────
    let detail = serde_json::json!({
        "batch": batch_id,
        "source": source.display().to_string(),
        "landed": {
            "knowledge": outcome.knowledge_landed,
            "entities": outcome.entities_landed,
            "relationships": outcome.relationships_landed,
            "sources": outcome.sources_landed,
            "source_revisions": outcome.revisions_landed,
        },
        "skipped": {
            "dedupe_live": plan.knowledge.dedupe_live,
            "dedupe_target_tombstone": plan.knowledge.dedupe_target_tombstone,
            "source_tombstoned": plan.knowledge.source_tombstoned,
            "unique_conflicts": outcome.knowledge_unique_conflicts,
        },
    });
    outcome.audit_row = audit::record(
        &tx,
        AuditKind::Import,
        "import",
        &batch_id,
        AuditStatus::Ok,
        &detail.to_string(),
    );

    tx.commit().context("commit target transaction")?;
    let _ = revision_map; // mappings consumed above; kept for symmetry of the call

    // The batch receipt: the revert reads THIS (the audit chain stores only
    // hashes, so the receipt is the act's readable record — the tool
    // family's sidecar discipline).
    let receipt = serde_json::json!({
        "batch_id": batch_id,
        "source": source.display().to_string(),
        "landed": {
            "knowledge": outcome.knowledge_landed,
            "entities": outcome.entities_landed,
            "relationships": outcome.relationships_landed,
            "sources": outcome.sources_landed,
            "source_revisions": outcome.revisions_landed,
        },
        "entities_created": created_entities,
        "committed_at": now_iso(),
    });
    write_atomic(
        &import_batch_receipt_path(dest, &batch_id),
        serde_json::to_string_pretty(&receipt)?.as_bytes(),
    )?;

    let src_sha_after = sha256_and_size(source)?.0;
    outcome.source_sha256_unchanged = src_sha_before == src_sha_after;
    if !outcome.source_sha256_unchanged {
        bail!(
            "THE SOURCE FILE CHANGED DURING THE MERGE ({} vs {}) — the batch \
             landed from a snapshot, but investigate the writer before trusting \
             the report's skip counts",
            src_sha_before,
            src_sha_after
        );
    }
    Ok(outcome)
}

fn value_text(v: Option<Value>) -> Option<String> {
    match v {
        Some(Value::Text(s)) => Some(s),
        Some(Value::Integer(i)) => Some(i.to_string()),
        _ => None,
    }
}

/// Full source knowledge rows (every mapped column, by name), chunked.
pub fn fetch_source_knowledge_rows(
    src: &SourceHandle,
    ids: &[i64],
) -> Result<Vec<HashMap<String, Value>>> {
    src.snapshot(|c| {
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!("SELECT * FROM knowledge WHERE id IN ({placeholders}) ORDER BY id");
        let mut stmt = c.prepare(&sql)?;
        let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |r| {
            let mut m = HashMap::new();
            for (i, name) in names.iter().enumerate() {
                let v: Value = r.get(i)?;
                m.insert(name.clone(), v);
            }
            Ok(m)
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    })
}

/// Provenance upserts — state-preserving (a deleted source stays deleted;
/// the import never reactivates what the operator deleted) and
/// non-superseding (the import never moves the target's current revision).
/// Both run INSIDE the batch transaction: the batch lands whole or not at all.
fn upsert_sources(
    src: &SourceHandle,
    tx: &rusqlite::Transaction,
    source_ids: &[i64],
    outcome: &mut LandOutcome,
) -> Result<HashMap<i64, i64>> {
    let rows: Vec<SourceSourceRow> = src.snapshot(|c| {
        let mut out = Vec::new();
        for chunk in chunked(source_ids, 500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT id, uri, kind, title, state, created_at, updated_at
                     FROM sources WHERE id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql)?;
            let rs = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })?;
            for r in rs {
                out.push(r?);
            }
        }
        Ok(out)
    })?;

    let mut map = HashMap::new();
    for (sid, uri, kind, title, state, created_at, _updated) in rows {
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM sources WHERE uri = ?1", [&uri], |r| {
                r.get(0)
            })
            .ok();
        if let Some(id) = existing {
            map.insert(sid, id);
            outcome.sources_existing += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO sources (uri, kind, title, state, created_at)
             VALUES (?1, ?2, ?3, COALESCE(?4, 'active'), COALESCE(?5, CURRENT_TIMESTAMP))",
            rusqlite::params![uri, kind, title, state, created_at],
        )
        .with_context(|| format!("insert source {uri}"))?;
        map.insert(sid, tx.last_insert_rowid());
        outcome.sources_landed += 1;
    }
    Ok(map)
}

fn upsert_revisions(
    src: &SourceHandle,
    tx: &rusqlite::Transaction,
    revision_ids: &[i64],
    source_map: &HashMap<i64, i64>,
    outcome: &mut LandOutcome,
) -> Result<HashMap<i64, i64>> {
    let rows: Vec<SourceRevisionRow> = src.snapshot(|c| {
        let mut out = Vec::new();
        for chunk in chunked(revision_ids, 500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT id, source_id, revision, content_hash, chunk_count, byte_size,
                            state, fetched_at
                     FROM source_revisions WHERE id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql)?;
            let rs = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<i64>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            })?;
            for r in rs {
                out.push(r?);
            }
        }
        Ok(out)
    })?;

    let mut map = HashMap::new();
    for (rid, sid, revision, content_hash, chunk_count, byte_size, state, fetched_at) in rows {
        let Some(tsid) = source_map.get(&sid) else {
            continue; // provenance for a source that did not land — cannot happen
            // (sources are planned from the same landed set) but never a guess.
        };
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM source_revisions WHERE source_id = ?1 AND revision = ?2",
                rusqlite::params![tsid, revision],
                |r| r.get(0),
            )
            .ok();
        if let Some(id) = existing {
            map.insert(rid, id);
            outcome.revisions_existing += 1;
            continue;
        }
        tx.execute(
            "INSERT INTO source_revisions
               (source_id, revision, content_hash, chunk_count, byte_size, state, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, 'active'), ?7)",
            rusqlite::params![
                tsid,
                revision,
                content_hash,
                chunk_count,
                byte_size,
                state,
                fetched_at
            ],
        )
        .with_context(|| format!("insert revision {revision}"))?;
        map.insert(rid, tx.last_insert_rowid());
        outcome.revisions_landed += 1;
    }
    Ok(map)
}

/// Entities (dedupe by name — the kernel's own NOCASE identity) and edges
/// (INSERT OR IGNORE — the kernel's own open-edge discipline). An edge lands
/// only when its chunk landed; endpoints come only from those edges.
fn land_edges(
    src: &SourceHandle,
    tx: &rusqlite::Transaction,
    land_ids: &[i64],
    kid_map: &HashMap<i64, i64>,
    outcome: &mut LandOutcome,
) -> Result<Vec<i64>> {
    // ONE source snapshot: the landed chunks' edges + the endpoints' entity
    // rows (id → name/type/created_at), so no per-edge query exists.
    let (edges, entities): (Vec<SourceEdgeRow>, Vec<SourceEntityRow>) = src.snapshot(|c| {
        let mut edges = Vec::new();
        let mut endpoint_ids: HashSet<i64> = HashSet::new();
        for chunk in chunked(land_ids, 500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT id, from_entity_id, to_entity_id, relation_type, created_at,
                        valid_at, invalid_at, superseded_at, knowledge_id
                 FROM relationships WHERE knowledge_id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql)?;
            let rs = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                ))
            })?;
            for r in rs {
                let (rid, from, to, rel, created, valid, invalid, superseded, kid) = r?;
                endpoint_ids.insert(from);
                endpoint_ids.insert(to);
                edges.push((rid, from, to, rel, created, valid, invalid, superseded, kid));
            }
        }
        let mut entities = Vec::new();
        let endpoint_list: Vec<i64> = endpoint_ids.into_iter().collect();
        for chunk in chunked(&endpoint_list, 500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT id, name, entity_type, created_at FROM entities
                 WHERE id IN ({placeholders})"
            );
            let mut stmt = c.prepare(&sql)?;
            let rs = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?;
            for r in rs {
                entities.push(r?);
            }
        }
        Ok((edges, entities))
    })?;

    // Upsert the endpoints by name (kernel discipline), then map the source
    // entity id → target id for the edges below. The CREATED ids are
    // returned: the batch receipt records them, and the revert deletes
    // exactly those (never a reused entity).
    let mut eid_to_target: HashMap<i64, i64> = HashMap::new();
    let mut created_entities: Vec<i64> = Vec::new();
    for (eid, name, entity_type, created_at) in &entities {
        let existing: Option<i64> = tx
            .query_row("SELECT id FROM entities WHERE name = ?1", [name], |r| {
                r.get(0)
            })
            .ok();
        let id = match existing {
            Some(id) => {
                outcome.entities_reused += 1;
                id
            }
            None => {
                tx.execute(
                    "INSERT OR IGNORE INTO entities (name, entity_type, created_at)
                     VALUES (?1, ?2, ?3)",
                    rusqlite::params![name, entity_type, created_at],
                )?;
                let id = tx.query_row("SELECT id FROM entities WHERE name = ?1", [name], |r| {
                    r.get(0)
                })?;
                outcome.entities_landed += 1;
                created_entities.push(id);
                id
            }
        };
        eid_to_target.insert(*eid, id);
    }

    for (_rid, from, to, rel, created, valid, invalid, superseded, kid) in edges {
        let (Some(tkid), Some(fid), Some(tid)) = (
            kid.and_then(|k| kid_map.get(&k).copied()),
            eid_to_target.get(&from).copied(),
            eid_to_target.get(&to).copied(),
        ) else {
            continue; // the chunk did not land (defensive) or an unmapped endpoint
        };
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO relationships
               (from_entity_id, to_entity_id, relation_type, knowledge_id,
                created_at, valid_at, invalid_at, superseded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![fid, tid, rel, tkid, created, valid, invalid, superseded],
        )?;
        if inserted == 1 {
            outcome.relationships_landed += 1;
        } else {
            outcome.relationships_existing += 1;
        }
    }
    Ok(created_entities)
}

pub fn chunked(v: &[i64], n: usize) -> Vec<&[i64]> {
    v.chunks(n).collect()
}

// ── the revert ──────────────────────────────────────────────────────────────

/// `--revert-batch <id>`: tombstone EXACTLY the rows that batch landed (the
/// forget-path way — digest preserved, named reason), remove exactly their
/// edges, and clean up entities the batch created (zero remaining edges).
/// One audit row, the kernel's own chain machinery writing it.
pub fn phase_revert_batch(dest: &Path, batch_id: &str) -> Result<()> {
    confirm_source_exists(dest).with_context(|| format!("target not found: {}", dest.display()))?;
    let mut conn =
        Connection::open(dest).with_context(|| format!("open target {}", dest.display()))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("enable foreign_keys on target")?;
    let origin_stamp = format!("imported:{batch_id}");

    // BEGIN IMMEDIATE — the same write discipline as the commit (the revert
    // is a write act against a store a running kernel may hold).
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("begin revert transaction")?;
    let n: i64 = tx.query_row(
        "SELECT COUNT(*) FROM knowledge WHERE origin = ?1",
        [&origin_stamp],
        |r| r.get(0),
    )?;
    if n == 0 {
        bail!(
            "revert-batch {batch_id}: no rows carry the stamp '{origin_stamp}' — \
             nothing to revert (wrong id, or already reverted)"
        );
    }

    // The batch receipt names the entities the batch CREATED (the audit
    // chain stores only hashes, so the receipt is the act's readable
    // record). Without it the revert refuses: it must not guess which
    // entities were created versus reused.
    let receipt_path = import_batch_receipt_path(dest, batch_id);
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&receipt_path).with_context(|| {
            format!(
                "no batch receipt found beside the target ({}): the revert refuses \
                 to guess which entities the batch created",
                receipt_path.display()
            )
        })?)
        .context("the batch receipt does not parse")?;
    let created_entities: Vec<i64> = receipt
        .get("entities_created")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();

    let rels_deleted = tx.execute(
        "DELETE FROM relationships
         WHERE knowledge_id IN (SELECT id FROM knowledge WHERE origin = ?1)",
        [&origin_stamp],
    )?;

    // The tombstone rows, the forget-path column set + the named reason.
    let reason = format!("import-revert:{batch_id}");
    tx.execute(
        "INSERT INTO tombstones (knowledge_id, document_id, content_hash, reason)
         SELECT id, document_id, content_hash, ?2 FROM knowledge WHERE origin = ?1",
        rusqlite::params![origin_stamp, reason],
    )?;

    let k_deleted = tx.execute("DELETE FROM knowledge WHERE origin = ?1", [&origin_stamp])?;

    // Exactly the batch-created entities — and only those the kernel has not
    // adopted meanwhile (a post-import edge pointing at one keeps it alive).
    let mut entities_removed = 0usize;
    for chunk in chunked(&created_entities, 500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "DELETE FROM entities WHERE id IN ({placeholders})
             AND NOT EXISTS (SELECT 1 FROM relationships r
                             WHERE r.from_entity_id = entities.id
                                OR r.to_entity_id = entities.id)"
        );
        entities_removed += tx.execute(&sql, rusqlite::params_from_iter(chunk.iter()))? as usize;
    }

    let detail = serde_json::json!({
        "batch": batch_id,
        "reverted": {
            "knowledge": k_deleted,
            "relationships": rels_deleted,
            "entities": entities_removed,
        },
    });
    let audit_row = audit::record(
        &tx,
        AuditKind::Import,
        "import",
        batch_id,
        AuditStatus::Ok,
        &detail.to_string(),
    );
    tx.commit().context("commit revert transaction")?;

    let report = serde_json::json!({
        "mode": "merge-revert",
        "verdict": "reverted",
        "batch_id": batch_id,
        "reverted": {
            "knowledge": k_deleted,
            "relationships": rels_deleted,
            "entities_removed": entities_removed,
        },
        "audit_row": audit_row,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

pub fn confirm_source_exists(source: &Path) -> Result<()> {
    if !source.exists() {
        bail!("source DB not found: {}", source.display());
    }
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn import_batch_receipt_path(dest: &Path, batch_id: &str) -> PathBuf {
    let mut p = dest.as_os_str().to_os_string();
    p.push(format!(".import-batch-{batch_id}.json"));
    PathBuf::from(p)
}

pub fn sha256_and_size(path: &Path) -> Result<(String, u64)> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let size = bytes.len() as u64;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok((hex::encode(h.finalize()), size))
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".tmp-{}", std::process::id()));
    let tmp_path = PathBuf::from(tmp);
    fs::write(&tmp_path, bytes).with_context(|| format!("write {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path)
        .with_context(|| format!("rename {} → {}", tmp_path.display(), path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    // ── merge pins (the workspace import) ────────────────────────────────
    //
    // Each pin drives the REAL phase functions against fixture stores. The
    // red-drills for these pins are executed by planting the defect in the
    // source file, watching the pin fail, and restoring from the round's own
    // copy (never git) — recorded in the round's records.

    /// A fixture source store with current-schema tables: rows to land, a
    /// tombstone-bound row, a duplicate (by content_hash+domain), rows with
    /// provenance, and a small entity graph.
    struct MergeFixture {
        _dir: tempfile::TempDir,
        source: PathBuf,
        target: PathBuf,
    }

    fn build_merge_fixture() -> MergeFixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.db");
        let target = dir.path().join("target.db");
        crate::register_sqlite_vec::register_sqlite_vec();
        // The source: current schema, then rows.
        {
            let mut c = Connection::open(&source).expect("open fixture source");
            {
                use crate::migration::run_migration;
                run_migration(&mut c, 256)
            }
            .expect("migrate fixture source");
            c.execute_batch(
                r#"
                INSERT INTO sources (id, uri, kind, title, state) VALUES
                    (10, 'fixture://doc-1', 'vault', 'Doc One', 'active'),
                    (11, 'fixture://doc-2', 'vault', NULL, 'deleted');
                INSERT INTO source_revisions (id, source_id, revision, content_hash, chunk_count, byte_size, state)
                    VALUES (20, 10, 'rev-1', 'src-hash-1', 2, 256, 'active');
                INSERT INTO entities (id, name, entity_type) VALUES
                    (30, 'Alpha', 'person'), (31, 'Beta', 'org'), (32, 'Gamma', 'topic');
                INSERT INTO knowledge (id, title, content, content_hash, domain, source_id, revision_id, origin, access_scope, owner)
                    VALUES
                    (1, 'alpha memory', 'content-alpha', 'hash-alpha', 'global', 10, 20, 'agent', 'private', NULL),
                    (2, 'dup memory', 'content-dup', 'hash-dup', 'global', 10, NULL, 'agent', 'private', NULL),
                    (3, 'dead memory', 'content-dead', 'hash-dead', 'global', 11, NULL, 'agent', 'private', NULL),
                    (4, 'nohash memory', 'content-nohash', NULL, 'global', NULL, NULL, 'agent', 'private', NULL),
                    (5, 'quarantined', 'reach me at quarantine@example.com please', 'hash-quar', 'global', NULL, NULL, 'agent', 'private', NULL);
                UPDATE knowledge SET flagged = 1 WHERE id = 5;
                INSERT INTO relationships (id, from_entity_id, to_entity_id, relation_type, knowledge_id)
                    VALUES (40, 30, 31, 'mentions', 1), (41, 31, 32, 'references', 3);
                INSERT INTO tombstones (knowledge_id, document_id, content_hash)
                    VALUES (99, 'fixture://doc-9', 'hash-dead');
                "#,
            )
            .expect("seed fixture source");
        }
        // The target: current schema, one live row that duplicates hash-dup,
        // one tombstone that binds hash-gone.
        {
            let mut c = Connection::open(&target).expect("open fixture target");
            {
                use crate::migration::run_migration;
                run_migration(&mut c, 256)
            }
            .expect("migrate fixture target");
            c.execute_batch(
                r#"
                INSERT INTO knowledge (title, content, content_hash, domain, origin)
                    VALUES ('existing', 'content-dup', 'hash-dup', 'global', 'human');
                INSERT INTO tombstones (knowledge_id, document_id, content_hash)
                    VALUES (1, 'fixture://gone', 'hash-gone');
                INSERT INTO sources (id, uri, kind, state) VALUES
                    (50, 'fixture://doc-1', 'vault', 'active');
                INSERT INTO entities (id, name, entity_type) VALUES
                    (60, 'Beta', 'org');
                "#,
            )
            .expect("seed fixture target");
        }
        MergeFixture {
            _dir: dir,
            source,
            target,
        }
    }

    fn sha_of(path: &Path) -> String {
        sha256_and_size(path).expect("sha").0
    }

    fn merge_args(f: &MergeFixture) -> ImportInvocation {
        ImportInvocation {
            source: Some(f.source.clone()),
            dest: f.target.clone(),
            commit_batch: None,
            revert_batch: None,
        }
    }

    #[test]
    fn test_merge_never_writes_the_source() {
        let f = build_merge_fixture();
        let before = sha_of(&f.source);
        // dry run
        phase_merge(&merge_args(&f), None).expect("dry run");
        assert_eq!(
            sha_of(&f.source),
            before,
            "dry run must not touch the source"
        );
        // commit run
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present"))
                .expect("open source");
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        assert_eq!(delta.verdict, "identity");
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert!(
            outcome.source_sha256_unchanged,
            "the source file must be byte-identical after the commit"
        );
        assert_eq!(
            sha_of(&f.source),
            before,
            "commit must not touch the source"
        );
    }

    /// The red-drill arm, standing: the accident class the pin exists for,
    /// reproduced test-locally (a read-write open writing the source) so the
    /// pin's detection mechanism is PROVEN to fire — a sha comparison that
    /// could never fail is decoration.
    #[test]
    fn test_merge_source_pin_detects_a_changed_source() {
        let f = build_merge_fixture();
        let before = sha_of(&f.source);
        // The mutant, test-local: the thing the law forbids.
        {
            let c = Connection::open(&f.source).unwrap();
            c.execute("UPDATE knowledge SET title = 'mutant' WHERE id = 1", [])
                .unwrap();
        }
        assert_ne!(
            sha_of(&f.source),
            before,
            "the pin's comparison must see a write"
        );
    }

    #[test]
    fn test_merge_tombstoned_memory_does_not_resurrect() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present")).unwrap();
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        // hash-dead is tombstoned in the SOURCE; hash-gone in the TARGET;
        // hash-dup is a live dedupe hit; the NULL-hash row is skipped, named.
        assert_eq!(
            plan.knowledge.source_tombstoned, 1,
            "source tombstone binds hash-dead"
        );
        assert_eq!(plan.knowledge.dedupe_target_tombstone, 0);
        assert_eq!(
            plan.knowledge.dedupe_live, 1,
            "hash-dup is a live dedupe hit"
        );
        assert_eq!(
            plan.knowledge.without_content_hash, 1,
            "the NULL-hash row is skipped by name"
        );
        assert_eq!(plan.knowledge.to_land, 2, "alpha + quarantined land");
        assert_eq!(
            plan.knowledge.quarantined_to_land, 1,
            "quarantined lands AS quarantined"
        );
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert_eq!(outcome.knowledge_landed, 2);
        assert_eq!(outcome.knowledge_unique_conflicts, 0);
        // The tombstoned content must NOT be in the target.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE content_hash IN ('hash-dead', 'hash-gone')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "deleted memory does not resurrect");
        // The deduped content lands once — the target's own row.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE content_hash = 'hash-dup'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "a duplicate lands once or not at all");
    }

    #[test]
    fn test_merge_dry_run_lands_nothing() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        phase_merge(&args, None).expect("dry run");
        let tgt = Connection::open(&args.dest).unwrap();
        let n: i64 = tgt
            .query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "the report lands nothing");
        let audits: i64 = tgt
            .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(audits, 0, "no audit row without the commit");
    }

    #[test]
    fn test_merge_commit_requires_the_report_batch_id() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        // A wrong id refuses BEFORE anything lands.
        let err = phase_merge(&args, Some("wi-deadbeefdeadbeef")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("not the plan's own id"),
            "the refusal must name the id binding: {msg}"
        );
        let tgt = Connection::open(&args.dest).unwrap();
        let n: i64 = tgt
            .query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "a refused commit lands nothing");
    }

    #[test]
    fn test_merge_commit_lands_and_writes_one_audit_row() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present")).unwrap();
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        assert!(
            plan.batch_id.starts_with("wi-"),
            "batch id shape: {}",
            plan.batch_id
        );
        let batch = plan.batch_id.clone();
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert_eq!(outcome.knowledge_landed, 2);
        assert!(
            outcome.audit_row.is_some(),
            "the commit writes ONE audit row"
        );
        // The audit row: kind import, target_hash = the batch id's digest,
        // ON the chain (the chain stores hashes, not plaintext).
        let (kind, target_hash): (String, String) = tgt
            .query_row(
                "SELECT kind, target_hash FROM audit_events WHERE id = ?1",
                [outcome.audit_row.unwrap()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "import");
        assert_eq!(
            target_hash,
            audit::hash(&batch),
            "the audit row names the batch"
        );
        assert!(
            audit::verify_chain(&tgt),
            "the chain must verify after the import row"
        );
        // Landed rows carry the batch stamp + a recomputed governance hash.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE origin = ?1",
                [format!("imported:{batch}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "every landed row carries the batch stamp");
        let nohash: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE origin = ?1 AND (governance_hash IS NULL OR governance_hash = '')",
                [format!("imported:{batch}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(nohash, 0, "every landed row names a governance hash");
        // Provenance: doc-1 existed in the target → reused, never touched.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM sources WHERE uri = 'fixture://doc-1' AND state = 'active' AND id = 50",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "an existing source is reused by uri, not duplicated");
        // doc-2's only chunk was tombstoned memory — provenance rides its
        // chunks: it must not appear in the target at all (a deleted source
        // never resurrects, not even as a row).
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM sources WHERE uri = 'fixture://doc-2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n, 0,
            "a deleted source whose chunk did not land stays absent"
        );
    }

    #[test]
    fn test_merge_lands_edges_only_for_landed_chunks() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present")).unwrap();
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        // Edge 40 rides chunk 1 (lands); edge 41 rides chunk 3 (tombstoned).
        assert_eq!(
            plan.relationships.to_land, 1,
            "only the landed chunk's edge"
        );
        assert_eq!(plan.relationships.skipped_chunk_not_landed, 1);
        assert_eq!(plan.entities.referenced_by_landed_edges, 2, "Alpha + Beta");
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert_eq!(outcome.relationships_landed, 1);
        // Gamma (endpoint of the skipped edge) must NOT land.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE name = 'Gamma'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "endpoints of unlanded edges do not land");
        // Beta existed in the target → reused, not duplicated.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE name = 'Beta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(outcome.entities_reused, 1);
        assert_eq!(outcome.entities_landed, 1, "only Alpha is new");
    }

    #[test]
    fn test_merge_revert_tombstones_exactly_its_own_rows() {
        let f = build_merge_fixture();
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present")).unwrap();
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        let batch = plan.batch_id.clone();
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert_eq!(outcome.knowledge_landed, 2);
        let pre_existing: i64 = tgt
            .query_row(
                "SELECT id FROM knowledge WHERE content_hash = 'hash-dup'",
                [],
                |r| r.get(0),
            )
            .unwrap();

        phase_revert_batch(&args.dest, &batch).expect("revert");
        // The batch's rows: gone + tombstoned with the named reason.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE origin = ?1",
                [format!("imported:{batch}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "the batch's rows are gone");
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM tombstones WHERE reason = ?1",
                [format!("import-revert:{batch}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "exactly the batch's rows are tombstoned");
        // The pre-existing row and the deduped twin survive untouched.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM knowledge WHERE id = ?1",
                [pre_existing],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "the target's own rows are untouched");
        // The batch's edges are gone; the entity the batch created (Alpha, no
        // remaining edges) is gone; Beta (target's own) stays.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE name = 'Alpha'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "the batch-created entity is cleaned up");
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE name = 'Beta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "the target's own entity survives");
        // The revert wrote its own audit row (two import rows now: land +
        // revert — the chain stores hashes, so the count is the evidence);
        // the chain still verifies.
        let n: i64 = tgt
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE kind = 'import'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "the revert writes its own audit row");
        assert!(
            audit::verify_chain(&tgt),
            "the chain verifies after the revert"
        );
    }

    #[test]
    fn test_merge_revert_refuses_an_unknown_batch() {
        let f = build_merge_fixture();
        let err = phase_revert_batch(&f.target, "wi-nonexistent0000").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("nothing to revert"),
            "the refusal must name the empty batch: {msg}"
        );
    }

    #[test]
    fn test_merge_refuses_a_mismatched_schema_by_name() {
        let f = build_merge_fixture();
        // A source column the target lacks: the delta must REFUSE naming it.
        {
            let c = Connection::open(&f.source).unwrap();
            c.execute("ALTER TABLE entities ADD COLUMN legacy_weight REAL", [])
                .unwrap();
        }
        let src_conn = SourceHandle::open_read_only(&f.source).unwrap();
        let src = SourceHandle::new(&src_conn);
        let tgt = Connection::open(&f.target).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, _mapping) = schema_delta(&src, &tgt).unwrap();
        assert_eq!(delta.verdict, "refused");
        let refusal = delta.refusal.expect("named refusal");
        assert_eq!(refusal.table, "entities");
        assert_eq!(refusal.column, "legacy_weight");
    }

    #[test]
    fn test_merge_refuses_missing_or_same_paths() {
        let f = build_merge_fixture();
        // A target that does not exist → refuse.
        let mut args = merge_args(&f);
        args.dest = f._dir.path().join("missing.db");
        let err = phase_merge(&args, None).unwrap_err();
        assert!(format!("{err:#}").contains("target not found"));
        // The source and the target being the same file → refuse.
        let mut args = merge_args(&f);
        args.dest = f.source.clone();
        let err = phase_merge(&args, None).unwrap_err();
        assert!(format!("{err:#}").contains("refuses a target that IS the source"));
    }

    #[test]
    fn test_merge_source_scan_no_write_verbs_on_the_source_side() {
        // The SOURCE SCAN pin: the merge section's source-side code names no
        // write verb. Read this file's own merge section: between the merge
        // marker and the helpers, every `.execute(`/`.execute_batch(`/
        // `.commit(` receiver must be the TARGET side (tx / tgt_conn), and
        // SourceHandle's impl must contain none at all.
        let src = include_str!("migrate_merge.rs");
        let section = src;
        let impl_start = section
            .find("pub struct SourceHandle")
            .expect("SourceHandle");
        let impl_end = section
            .find("// ── merge report shapes")
            .expect("report shapes");
        let impl_text = &section[impl_start..impl_end];
        for verb in [
            ".execute(",
            ".execute_batch(",
            ".commit(",
            "INSERT INTO",
            "UPDATE ",
            "DELETE FROM",
        ] {
            assert!(
                !impl_text.contains(verb),
                "SourceHandle's impl names the write verb {verb:?} — the read-only-by-construction pin"
            );
        }
        // The whole merge section: INSERT/UPDATE/DELETE SQL is allowed ONLY
        // on the target side; assert every write-SQL line sits after the
        // commit functions begin (i.e. the plan/source-side functions contain
        // none). The plan/snapshot region is from the section start to the
        // commit marker.
        let commit_start = section.find("// ── the commit ──").expect("commit marker");
        let source_side = &section[section
            .find("pub struct ImportInvocation")
            .expect("invocation")..commit_start];
        for verb in ["INSERT INTO", "UPDATE ", "DELETE FROM", ".commit("] {
            assert!(
                !source_side.contains(verb),
                "the merge's source-side/plan code names the write verb {verb:?}"
            );
        }
    }
    #[test]
    fn test_parse_import_argv_requires_both_paths_by_name() {
        let err = parse_import_argv(&[]).unwrap_err();
        assert!(
            format!("{err:#}").contains("--source is required"),
            "{err:#}"
        );
        let err = parse_import_argv(&["--source".into(), "/tmp/a.db".into()]).unwrap_err();
        assert!(format!("{err:#}").contains("--dest is required"), "{err:#}");
        let inv = parse_import_argv(&[
            "--import".into(),
            "--source".into(),
            "/tmp/a.db".into(),
            "--dest".into(),
            "/tmp/b.db".into(),
            "--dry-run".into(),
        ])
        .expect("parses");
        assert_eq!(inv.source.as_deref(), Some(Path::new("/tmp/a.db")));
        assert_eq!(inv.dest, PathBuf::from("/tmp/b.db"));
        assert!(inv.commit_batch.is_none() && inv.revert_batch.is_none());
        // revert + commit-batch refuses (two batch decisions in one act).
        // A source alongside a revert is accepted and ignored (the revert
        // reads only the target) — the run_import dispatch makes that true.
        let err = parse_import_argv(&[
            "--import".into(),
            "--source".into(),
            "/tmp/a.db".into(),
            "--dest".into(),
            "/tmp/b.db".into(),
            "--revert-batch".into(),
            "wi-x".into(),
            "--commit-batch".into(),
            "wi-y".into(),
        ])
        .unwrap_err();
        assert!(format!("{err:#}").contains("runs alone"), "{err:#}");
        let inv = parse_import_argv(&[
            "--dest".into(),
            "/tmp/b.db".into(),
            "--revert-batch".into(),
            "wi-x".into(),
        ])
        .expect("revert alone parses");
        assert_eq!(inv.revert_batch.as_deref(), Some("wi-x"));
        // unknown flags refuse
        let err = parse_import_argv(&["--force".into()]).unwrap_err();
        assert!(
            format!("{err:#}").contains("unknown import flag"),
            "{err:#}"
        );
    }

    /// The pii construction pin: a source WITHOUT the pii column (the
    /// earliest-schema shape) lands rows whose flag the KERNEL'S OWN GATE
    /// computed — never the schema default (which would turn read-time
    /// redaction off for every landed row). The EXEMPT entry in the pii
    /// guard names this construction; this pin makes the claim true.
    #[test]
    fn test_merge_computes_pii_when_the_source_lacks_the_column() {
        let f = build_merge_fixture();
        // The earliest-schema shape: no pii column at all.
        {
            let c = Connection::open(&f.source).unwrap();
            c.execute_batch("ALTER TABLE knowledge DROP COLUMN pii;")
                .unwrap();
        }
        let args = merge_args(&f);
        let src_conn =
            SourceHandle::open_read_only(args.source.as_deref().expect("source present")).unwrap();
        let src = SourceHandle::new(&src_conn);
        let mut tgt = Connection::open(&args.dest).unwrap();
        tgt.pragma_update(None, "foreign_keys", "ON").unwrap();
        let (delta, mapping) = schema_delta(&src, &tgt).unwrap();
        // The delta stays IDENTITY: pii is a target-only column WITH a
        // default, so the mapping omits it and the commit appends it.
        assert_eq!(delta.verdict, "identity");
        let (report, data) = plan_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &tgt,
            delta,
        )
        .unwrap();
        let plan = report.plan.expect("plan");
        let outcome = commit_merge(
            args.source.as_deref().expect("source present"),
            &args.dest,
            &src,
            &mut tgt,
            &mapping,
            &data,
            &plan,
        )
        .unwrap();
        assert!(outcome.knowledge_landed > 0);
        // A row whose content carries an address (pii_flag fires) and a
        // row whose content does not: the flags must differ, computed.
        let pii_rows: Vec<(i64, i64)> = {
            let mut stmt = tgt
                .prepare("SELECT id, pii FROM knowledge WHERE origin LIKE 'imported:%' ORDER BY id")
                .unwrap();
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .flatten()
                .collect()
        };
        // Every landed row carries an EXPLICIT flag (the column was bound,
        // not defaulted): both 0 and 1 appear across the fixture's rows.
        let ones = pii_rows.iter().filter(|(_, pii)| *pii == 1).count();
        let zeros = pii_rows.iter().filter(|(_, pii)| *pii == 0).count();
        assert!(
            ones > 0 && zeros > 0,
            "computed flags must vary with content: {pii_rows:?}"
        );
        // Spot-check the construction: the fixture's alpha content is clean.
        let alpha_pii: i64 = tgt
            .query_row(
                "SELECT pii FROM knowledge WHERE content_hash = 'hash-alpha' AND origin LIKE 'imported:%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            alpha_pii, 0,
            "clean content computes pii=0 through the gate"
        );
    }
}
