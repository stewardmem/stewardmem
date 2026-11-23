//! The policy-anchor registry (GOVPACK-1, K-31) — the kernel's governed
//! registry of the AI instruments a deployment is governed by.
//!
//! THE LAWS this core enforces (each pinned in `tests/policy_anchor_pins.rs`):
//!
//! * **Fail-closed.** A missing, expired, superseded, or unverified anchor is
//!   a REFUSAL naming the anchor id — never a silent pass. The pack gate
//!   lists EVERY blocker (never just the first) with a distinct reason, and
//!   an anchor whose text the registry does not hold says
//!   `advisory-only — anchor text not held`, so an operator cannot mistake
//!   an unverified deployment for a verified one.
//! * **Never-guess.** The registry's entire public vocabulary is
//!   `verified_text_held / text_fetch_owed / superseded / retired` and the
//!   pack verdicts `verified / advisory-only / refused`. A field the kernel
//!   cannot state is rendered `not verified`/`not reported` by the caller —
//!   never a filler, and never the word "compliant".
//! * **Append-only history.** A changed instrument is a NEW version row via
//!   [`supersede`] with the old row's `in_force_until` stamped; no UPDATE of
//!   history, no DELETE. [`seed`] never edits a row in place: the same
//!   `anchor_id` with identical content is a skip; with ANY content drift it
//!   is a refusal naming the anchor (supersede is the only lawful change).
//! * **Rust computes hashes, SQL never.** `sha256_hex` is computed here from
//!   the corpus bytes; the stored value is compared against a re-computed
//!   digest, never re-derived in SQL.
//! * **Audit-per-write, in-tx.** Every seed, supersede, and pack-gate act
//!   writes one [`AuditKind::PolicyAnchor`] row through
//!   [`record_tenant_checked`] INSIDE the caller's transaction
//!   (SAVEPOINT-nested) — a transition and its evidence commit or roll back
//!   together. The detail is a closed vocabulary of counts and ids; the
//!   chain stores hashes, so no instrument text and no corpus path ride a
//!   row.
//! * **Write discipline.** The seed, the supersede, and the pack gate are
//!   `BEGIN IMMEDIATE` transactions from row one (a running kernel may hold
//!   the lock; a DEFERRED read-then-write deadlocks on upgrade).
//!
//! Date law: stamps are ISO-8601 TEXT (`YYYY-MM-DD`, or month-precision
//! `YYYY-MM` when that is all the issuer stated) compared lexicographically.
//! A month-precision stamp compares before any day within that month — the
//! precision is visible in the stored value, never guessed into a day that
//! was never stated. An `as_of` parameter is validated against the same
//! shapes and refused otherwise.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::audit::{AuditKind, AuditStatus, record_tenant_checked};

/// The audit actor for CLI-driven registry acts (the rules door's
/// convention: `operator` for a local operator act; a future service-lane
/// caller names its own actor).
pub const ACTOR: &str = "operator";

// ── the registry's public vocabulary ─────────────────────────────────────

/// The lifecycle state of one anchor row. The CHECK in the schema holds the
/// same four strings; this type is the Rust-side half of that law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorLifecycle {
    /// The legally operative text is held in the corpus and hash-verified.
    VerifiedTextHeld,
    /// The instrument is verified in force via law-firm/official-source
    /// analysis, but its text is not held — packs requiring it stay in
    /// advisory mode until the text lands and the hash is recorded.
    TextFetchOwed,
    /// A later version replaced this row (history; kept, never deleted).
    Superseded,
    /// Withdrawn from the registry's active set by an operator act.
    Retired,
}

impl AnchorLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            AnchorLifecycle::VerifiedTextHeld => "verified_text_held",
            AnchorLifecycle::TextFetchOwed => "text_fetch_owed",
            AnchorLifecycle::Superseded => "superseded",
            AnchorLifecycle::Retired => "retired",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "verified_text_held" => Some(AnchorLifecycle::VerifiedTextHeld),
            "text_fetch_owed" => Some(AnchorLifecycle::TextFetchOwed),
            "superseded" => Some(AnchorLifecycle::Superseded),
            "retired" => Some(AnchorLifecycle::Retired),
            _ => None,
        }
    }

    fn is_live(self) -> bool {
        matches!(
            self,
            AnchorLifecycle::VerifiedTextHeld | AnchorLifecycle::TextFetchOwed
        )
    }
}

// ── the seed manifest ────────────────────────────────────────────────────

/// One anchor row as the manifest states it. Only the fields the registry
/// persists are deserialized strictly; the manifest's remaining fields
/// (`verified_via`, the corpus receipts, the header prose) are the corpus's
/// own provenance and are ignored here — never stored as if they were
/// verification state.
#[derive(Debug, Deserialize)]
struct ManifestAnchor {
    id: String,
    issuer: String,
    instrument_no: String,
    series: String,
    title: String,
    #[serde(default)]
    scope_tags: Vec<String>,
    #[serde(default)]
    effective: serde_json::Value,
    state: String,
    sha256_hex: Option<String>,
    corpus_path: Option<String>,
    #[serde(default)]
    official_url: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    manifest_version: serde_json::Value,
    anchors: Vec<ManifestAnchor>,
}

/// A per-anchor schema refusal: this anchor cannot land, the rest of the
/// manifest may. (A whole-manifest contradiction is a [`SeedRefusal`] —
/// the act fails closed.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorSchemaRefusal {
    pub anchor_id: Option<String>,
    pub reason: String,
}

/// Why a manifest cannot seed at all. A total refusal: nothing lands.
#[derive(Debug, Clone)]
pub enum SeedRefusal {
    /// The manifest is not valid JSON (or not an object).
    Unparsable(String),
    /// The manifest's `manifest_version` is not a number this core knows.
    ManifestVersion(String),
    /// The manifest carries no anchors — seeding nothing is not a seed.
    EmptyAnchors,
    /// The manifest names the same anchor id twice — it contradicts itself.
    DuplicateAnchorId(String),
    /// A held instrument's bytes no longer hash to the manifest's recorded
    /// digest: the manifest's verification claim is false as a whole, so the
    /// whole act refuses (seeding the other anchors would launder the drift).
    HashDrift {
        anchor_id: String,
        path: String,
        expected: String,
        actual: String,
    },
    /// A held instrument's file is absent from the corpus root — the same
    /// class as drift: the manifest claims a held text that is not there.
    CorpusMissing { anchor_id: String, path: String },
}

impl std::fmt::Display for SeedRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedRefusal::Unparsable(e) => write!(f, "manifest is not parsable JSON: {e}"),
            SeedRefusal::ManifestVersion(found) => write!(
                f,
                "manifest_version {found} is not a version this registry seeds (expected 1)"
            ),
            SeedRefusal::EmptyAnchors => {
                write!(
                    f,
                    "the manifest carries no anchors — seeding nothing is not a seed"
                )
            }
            SeedRefusal::DuplicateAnchorId(id) => write!(
                f,
                "anchor id '{id}' appears more than once in the manifest — a manifest that \
                 contradicts itself seeds nothing"
            ),
            SeedRefusal::HashDrift {
                anchor_id,
                path,
                expected,
                actual,
            } => write!(
                f,
                "anchor '{anchor_id}' text-hash drift: {path} hashes to {actual}, the manifest \
                 records {expected} — re-measure and re-record before seeding (never round the \
                 fixture to the drift)"
            ),
            SeedRefusal::CorpusMissing { anchor_id, path } => write!(
                f,
                "anchor '{anchor_id}' claims held text at {path} but the file is absent from \
                 the corpus root — a held text that is not there is drift by another name"
            ),
        }
    }
}

/// One planned anchor: schema-checked, date-resolved, and (for held text)
/// hash-verified against the corpus bytes.
#[derive(Debug, Clone)]
pub struct PlannedAnchor {
    pub anchor_id: String,
    pub issuer: String,
    pub instrument_no: String,
    pub series: String,
    pub title: String,
    pub scope_tags_json: String,
    pub sha256_hex: Option<String>,
    pub state: AnchorLifecycle,
    pub corpus_path: Option<String>,
    pub official_url: Option<String>,
    pub notes: Option<String>,
    pub in_force_from: String,
}

/// The plan [`verify_manifest`] returns on a seedable manifest: the rows
/// that will land, the per-anchor schema refusals that will not, and the
/// manifest bytes' own digest (the audit row's target).
#[derive(Debug, Clone)]
pub struct SeedPlan {
    pub manifest_sha256: String,
    pub rows: Vec<PlannedAnchor>,
    pub refusals: Vec<AnchorSchemaRefusal>,
}

/// What one seed act did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedReport {
    pub inserted: usize,
    pub skipped: usize,
    pub schema_refusals: Vec<AnchorSchemaRefusal>,
    pub audit_row_id: i64,
}

// ── date law ─────────────────────────────────────────────────────────────

/// Accept `YYYY-MM-DD` or `YYYY-MM` (month precision, as the issuer stated
/// it); refuse everything else — a quarter name, a phrase, a range, or an
/// empty string is never guessed into a date. Returns the validated stamp
/// unchanged (comparison is lexicographic ISO).
fn validate_date(anchor: &str, field: &str, value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let full = |b: &[u8]| b.iter().all(|c| c.is_ascii_digit());
    let ok = (bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && full(&bytes[0..4])
        && full(&bytes[5..7])
        && full(&bytes[8..10]))
        || (bytes.len() == 7 && bytes[4] == b'-' && full(&bytes[0..4]) && full(&bytes[5..7]));
    if ok {
        Ok(value.to_string())
    } else {
        Err(format!(
            "anchor '{anchor}': {field} '{value}' is not an ISO date this registry stores \
             (YYYY-MM-DD, or YYYY-MM at month precision) — a stamp it cannot state is a stamp \
             it refuses, never a guess"
        ))
    }
}

/// Resolve the anchor's `in_force_from` from its `effective` object: the
/// first key in a FIXED priority order whose value parses as a date. The
/// order is `effective` (the force date, preferred over the signature date),
/// then `signed`, `issued`, `approved`, `promulgated`, `framework_dated`.
/// Values that are not dates (an effectivity PHRASE, a resolution number)
/// are skipped — the resolution order is deterministic and never guesses a
/// day out of prose. The manifest's own `notes` column preserves whatever
/// prose the issuer wrote.
fn resolve_in_force_from(anchor_id: &str, effective: &serde_json::Value) -> Result<String, String> {
    const KEYS: [&str; 6] = [
        "effective",
        "signed",
        "issued",
        "approved",
        "promulgated",
        "framework_dated",
    ];
    let obj = effective
        .as_object()
        .ok_or_else(|| format!("anchor '{anchor_id}': the effective block is not an object"))?;
    for key in KEYS {
        if let Some(val) = obj.get(key)
            && let Some(s) = val.as_str()
        {
            let s = s.trim();
            if validate_date(anchor_id, &format!("effective.{key}"), s).is_ok() {
                return Ok(s.to_string());
            }
        }
    }
    Err(format!(
        "anchor '{anchor_id}': no parseable effective date in the effective block — the \
         registry stores when an instrument came into force or it does not store the \
         instrument; it never invents a date"
    ))
}

// ── hash law ─────────────────────────────────────────────────────────────

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The corpus-path fence: relative, no `..` component, no absolute form —
/// the corpus root is the only place a held text may live.
fn validate_corpus_path(anchor_id: &str, path: &str) -> Result<(), String> {
    let p = Path::new(path);
    if p.is_absolute() || path.split('/').any(|c| c == "..") || path.split('\\').any(|c| c == "..")
    {
        return Err(format!(
            "anchor '{anchor_id}': corpus_path '{path}' escapes the corpus root — a held text \
             lives under the corpus root or it is not held"
        ));
    }
    Ok(())
}

// ── verify_manifest ──────────────────────────────────────────────────────

/// Schema-check every anchor row and (for `verified_text_held`) verify the
/// held file's bytes against the recorded digest. Returns the seed plan, or
/// a TOTAL refusal for anything that poisons the whole act: unparsable
/// JSON, a wrong manifest version, no anchors, duplicate ids, hash drift,
/// or a missing held file. Per-anchor schema problems ride the plan as
/// refusals — the seed lands the rest and reports both (the CLI exits 1 so
/// a partial seed is never mistaken for a clean one).
pub fn verify_manifest(json: &str, corpus_root: &Path) -> Result<SeedPlan, SeedRefusal> {
    let manifest: Manifest =
        serde_json::from_str(json).map_err(|e| SeedRefusal::Unparsable(e.to_string()))?;
    let version = match &manifest.manifest_version {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(u64::MAX),
        other => {
            return Err(SeedRefusal::ManifestVersion(other.to_string()));
        }
    };
    if version != 1 {
        return Err(SeedRefusal::ManifestVersion(version.to_string()));
    }
    if manifest.anchors.is_empty() {
        return Err(SeedRefusal::EmptyAnchors);
    }

    let mut seen = std::collections::HashSet::new();
    for a in &manifest.anchors {
        if !seen.insert(a.id.as_str()) {
            return Err(SeedRefusal::DuplicateAnchorId(a.id.clone()));
        }
    }

    // The manifest bytes' own digest — the seed act's audit target.
    let manifest_sha256 = sha256_hex(json.as_bytes());

    let mut rows = Vec::new();
    let mut refusals = Vec::new();
    for a in &manifest.anchors {
        match plan_anchor(a) {
            Ok(planned) => {
                // Held text is verified HERE, at plan time: a drift or a
                // missing file refuses the whole act before anything lands.
                if planned.state == AnchorLifecycle::VerifiedTextHeld {
                    let path = corpus_root.join(
                        planned
                            .corpus_path
                            .as_deref()
                            .expect("held anchor carries a corpus path"),
                    );
                    let bytes = std::fs::read(&path).map_err(|_| SeedRefusal::CorpusMissing {
                        anchor_id: planned.anchor_id.clone(),
                        path: path.display().to_string(),
                    })?;
                    let actual = sha256_hex(&bytes);
                    let expected = planned
                        .sha256_hex
                        .as_deref()
                        .expect("held anchor carries a digest");
                    if !actual.eq_ignore_ascii_case(expected) {
                        return Err(SeedRefusal::HashDrift {
                            anchor_id: planned.anchor_id.clone(),
                            path: path.display().to_string(),
                            expected: expected.to_string(),
                            actual,
                        });
                    }
                }
                rows.push(planned);
            }
            Err(reason) => refusals.push(AnchorSchemaRefusal {
                anchor_id: Some(a.id.clone()),
                reason,
            }),
        }
    }

    Ok(SeedPlan {
        manifest_sha256,
        rows,
        refusals,
    })
}

/// Schema-check ONE manifest anchor into a planned row. Every field the
/// registry persists is validated here; the state/hash agreement is the
/// manifest's own law enforced at the boundary.
fn plan_anchor(a: &ManifestAnchor) -> Result<PlannedAnchor, String> {
    let id_charset_ok = !a.id.is_empty()
        && a.id.len() <= 100
        && a.id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_');
    if !id_charset_ok {
        return Err(format!(
            "anchor id '{}' is not in the registry's charset ([A-Za-z0-9._-], 1..=100) — the \
             id is the pack bindings' join key, not free text",
            a.id
        ));
    }
    for (field, value) in [
        ("issuer", &a.issuer),
        ("instrument_no", &a.instrument_no),
        ("series", &a.series),
        ("title", &a.title),
    ] {
        if value.trim().is_empty() {
            return Err(format!("anchor '{}': {field} is empty", a.id));
        }
    }

    let Some(state) = AnchorLifecycle::parse(&a.state) else {
        return Err(format!(
            "anchor '{}': state '{}' is not in the registry's vocabulary \
             (verified_text_held / text_fetch_owed / superseded / retired) — the vocabulary \
             is the registry's whole public voice",
            a.id, a.state
        ));
    };
    // A manifest may only OFFER live states; superseded/retired are registry
    // decisions (supersede / a future retire act), never seed input.
    if !state.is_live() {
        return Err(format!(
            "anchor '{}': a manifest may not seed state '{}' — superseded and retired are \
             registry decisions, never seed input",
            a.id,
            state.as_str()
        ));
    }

    let in_force_from = resolve_in_force_from(&a.id, &a.effective)?;

    match (&a.sha256_hex, &a.corpus_path, state) {
        (Some(hex), Some(path), AnchorLifecycle::VerifiedTextHeld) => {
            if !is_sha256_hex(hex) {
                return Err(format!(
                    "anchor '{}': sha256_hex is not 64 hex chars — the digest is the anchor's \
                     spine, a malformed one is refused, never stored",
                    a.id
                ));
            }
            validate_corpus_path(&a.id, path)?;
        }
        (None, None, AnchorLifecycle::TextFetchOwed) => {}
        (sha, path, _) => {
            return Err(format!(
                "anchor '{}': state and held-text fields disagree (sha256_hex {}, corpus_path \
                 {}) — verified_text_held carries both, text_fetch_owed carries neither; a \
                 manifest that claims a hash it does not hold is lying about verification",
                a.id,
                sha.as_deref().map_or("absent", |_| "present"),
                path.as_deref().map_or("absent", |_| "present")
            ));
        }
    }

    Ok(PlannedAnchor {
        anchor_id: a.id.clone(),
        issuer: a.issuer.clone(),
        instrument_no: a.instrument_no.clone(),
        series: a.series.clone(),
        title: a.title.clone(),
        scope_tags_json: serde_json::to_string(&a.scope_tags)
            .expect("a Vec<String> always serializes"),
        sha256_hex: a.sha256_hex.as_deref().map(|s| s.to_ascii_lowercase()),
        state,
        corpus_path: a.corpus_path.clone(),
        official_url: a.official_url.clone(),
        notes: a.notes.clone(),
        in_force_from,
    })
}

// ── the write-discipline shape ───────────────────────────────────────

/// ONE `BEGIN IMMEDIATE` transaction around a write act: commits on
/// success, rolls back on ANY error (including the audit row's failure —
/// a transition and its evidence commit or roll back together). Raw SQL
/// transaction control on `&Connection` — the same shape
/// `record_tenant_checked` uses to nest its own SAVEPOINT inside a caller's
/// tx, and the reason this core takes `&Connection` only (the handler-lane
/// `WorkflowTx` needs `&mut`). The IMMEDIATE start is the write-discipline
/// needle: a running kernel may hold the lock, and a DEFERRED
/// read-then-write deadlocks on upgrade.
fn immediate_tx<T>(
    conn: &Connection,
    what: &'static str,
    body: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| format!("{what} begin: {e}"))?;
    match body(conn) {
        Ok(v) => conn
            .execute_batch("COMMIT")
            .map_err(|e| format!("{what} commit: {e}"))
            .map(|_| v),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

// ── seed ─────────────────────────────────────────────────────────────────

/// Land the plan's rows in ONE `BEGIN IMMEDIATE` transaction, idempotent by
/// `anchor_id`: an existing live row with identical content is a skip; with
/// ANY content drift it is a refusal naming the anchor — a changed
/// instrument is a NEW version via [`supersede`], never an in-place edit or
/// a silent overwrite. An offer matching a SUPERSEDED row is a regression
/// and refuses by name too. One [`AuditKind::PolicyAnchor`] row records the
/// act in the same transaction; any refusal rolls everything back.
pub fn seed(conn: &Connection, plan: &SeedPlan) -> Result<SeedReport, String> {
    immediate_tx(conn, "seed", |tx| {
        let mut inserted = 0usize;
        let mut skipped = 0usize;
        for row in &plan.rows {
            let existing: Option<(
                i64,
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                String,
                Option<String>,
            )> = tx
                .query_row(
                    "SELECT id, issuer, instrument_no, series, title, scope_tags, state, \
                         sha256_hex, in_force_from, corpus_path
                     FROM policy_anchor
                     WHERE anchor_id = ?1 AND state IN ('verified_text_held','text_fetch_owed')",
                    params![row.anchor_id],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                            r.get(7)?,
                            r.get(8)?,
                            r.get(9)?,
                        ))
                    },
                )
                .optional()
                .map_err(|e| format!("seed read {}: {e}", row.anchor_id))?;

            if let Some((
                id,
                issuer,
                instrument_no,
                series,
                title,
                scope_tags,
                state,
                sha,
                from,
                path,
            )) = existing
            {
                let same = issuer == row.issuer
                    && instrument_no == row.instrument_no
                    && series == row.series
                    && title == row.title
                    && scope_tags == row.scope_tags_json
                    && state == row.state.as_str()
                    && sha.as_deref().map(str::to_string) == row.sha256_hex
                    && from == row.in_force_from
                    && path == row.corpus_path;
                if same {
                    skipped += 1;
                    continue;
                }
                return Err(format!(
                    "anchor '{}' already lives in the registry at different content (stored row \
                     {}, hash {}) — a changed instrument is a NEW version via supersede; the seed \
                     never edits history in place",
                    row.anchor_id,
                    id,
                    sha.as_deref().unwrap_or("not held")
                ));
            }

            // No live row: refuse a regression offer (the content matches a
            // superseded row) before inserting.
            let superseded_match: Option<i64> = tx
                .query_row(
                    "SELECT id FROM policy_anchor
                     WHERE anchor_id = ?1 AND state = 'superseded'
                       AND issuer = ?2 AND instrument_no = ?3 AND series = ?4 AND title = ?5
                       AND scope_tags = ?6 AND IFNULL(sha256_hex,'') = IFNULL(?7,'')
                       AND in_force_from = ?8",
                    params![
                        row.anchor_id,
                        row.issuer,
                        row.instrument_no,
                        row.series,
                        row.title,
                        row.scope_tags_json,
                        row.sha256_hex,
                        row.in_force_from,
                    ],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("seed regression read {}: {e}", row.anchor_id))?;
            if let Some(old_id) = superseded_match {
                return Err(format!(
                    "anchor '{}' matches superseded row {old_id} — the offer is a REGRESSION of an \
                     instrument the registry already moved past; refusing (the live registry is \
                     never quietly rolled back)",
                    row.anchor_id
                ));
            }

            tx.execute(
                "INSERT INTO policy_anchor(anchor_id, issuer, instrument_no, series, title, \
                     scope_tags, sha256_hex, state, corpus_path, official_url, notes, \
                     supersedes_id, in_force_from, in_force_until)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, NULL)",
                params![
                    row.anchor_id,
                    row.issuer,
                    row.instrument_no,
                    row.series,
                    row.title,
                    row.scope_tags_json,
                    row.sha256_hex,
                    row.state.as_str(),
                    row.corpus_path,
                    row.official_url,
                    row.notes,
                    row.in_force_from,
                ],
            )
            .map_err(|e| {
                format!(
                    "seed insert {}: {e} (has the migration run?)",
                    row.anchor_id
                )
            })?;
            inserted += 1;
        }

        // The act's evidence row, INSIDE the same transaction (the CHECKED
        // form sees the caller's tx and nests via SAVEPOINT — a transition
        // and its evidence commit or roll back together).
        let detail = format!(
            "seed inserted={inserted} skipped={skipped} refused={} manifest_sha256={}",
            plan.refusals.len(),
            &plan.manifest_sha256[..24.min(plan.manifest_sha256.len())],
        );
        let audit_row_id = record_tenant_checked(
            tx,
            AuditKind::PolicyAnchor,
            ACTOR,
            &format!("manifest_sha256:{}", plan.manifest_sha256),
            AuditStatus::Ok,
            &detail,
            crate::audit::DEFAULT_TENANT,
        )
        .map_err(|e| format!("seed audit row failed (the act refuses, nothing lands): {e}"))?;

        Ok(SeedReport {
            inserted,
            skipped,
            schema_refusals: plan.refusals.clone(),
            audit_row_id,
        })
    })
}

// ── supersede ────────────────────────────────────────────────────────────

/// The new version an operator offers via [`supersede`]. `state` must be a
/// live state — superseded/retired are registry decisions, not seed or
/// supersede input.
#[derive(Debug, Clone)]
pub struct NewVersion {
    pub anchor_id: String,
    pub issuer: String,
    pub instrument_no: String,
    pub series: String,
    pub title: String,
    pub scope_tags_json: String,
    pub sha256_hex: Option<String>,
    pub state: AnchorLifecycle,
    pub corpus_path: Option<String>,
    pub official_url: Option<String>,
    pub notes: Option<String>,
    pub in_force_from: String,
}

/// What one supersede act did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersedeReport {
    pub old_row_id: i64,
    pub new_row_id: i64,
    pub in_force_until: String,
    pub audit_row_id: i64,
}

/// Supersede one live anchor row with a new version, in ONE transaction:
/// the old row's `in_force_until` is stamped with the new version's
/// `in_force_from` and its state becomes `superseded`; the new row lands
/// with `supersedes_id` pointing at it; one audit row records the act —
/// all three or nothing (an injected failure after the new-row insert
/// leaves the old row unstamped, the audit count unchanged, and the new
/// row absent; that is pinned, not hoped).
///
/// Refuses by name: an unknown old id, an old row that is not live (its
/// lineage has already moved on), a new version whose state is not live,
/// and a new `in_force_from` earlier than the old row's (a supersede never
/// moves an instrument backwards in time).
pub fn supersede(
    conn: &Connection,
    old_id: i64,
    new: &NewVersion,
) -> Result<SupersedeReport, String> {
    if !new.state.is_live() {
        return Err(format!(
            "supersede: the new version's state must be a live state, got '{}' — superseded \
             and retired are registry decisions, never a new version's state",
            new.state.as_str()
        ));
    }

    immediate_tx(conn, "supersede", |tx| {
        let old: Option<(String, String, String)> = tx
            .query_row(
                "SELECT anchor_id, state, in_force_from FROM policy_anchor WHERE id = ?1",
                params![old_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(|e| format!("supersede read {old_id}: {e}"))?;
        let Some((anchor_id, old_state, old_from)) = old else {
            return Err(format!(
                "supersede: no policy_anchor row {old_id} — refusing an act against a row the \
                 registry does not hold"
            ));
        };
        let old_lifecycle = AnchorLifecycle::parse(&old_state).ok_or_else(|| {
            format!(
                "supersede: row {old_id} carries state '{old_state}' outside the registry's vocabulary"
            )
        })?;
        if !old_lifecycle.is_live() {
            return Err(format!(
                "supersede: row {old_id} (anchor '{anchor_id}') is already '{}' — supersede the \
                 lineage's LIVE row, never a history row",
                old_lifecycle.as_str()
            ));
        }
        if new.in_force_from < old_from {
            return Err(format!(
                "supersede: the new version's in_force_from ({}) precedes the old row's ({}) — a \
                 supersede never moves an instrument backwards in time",
                new.in_force_from, old_from
            ));
        }

        // Stamp the old row FIRST (inside the same tx), so the live-state unique
        // index sees one live row for this anchor at every statement boundary.
        tx.execute(
            "UPDATE policy_anchor SET state = 'superseded', in_force_until = ?2 WHERE id = ?1",
            params![old_id, new.in_force_from],
        )
        .map_err(|e| format!("supersede stamp {old_id}: {e}"))?;

        tx.execute(
            "INSERT INTO policy_anchor(anchor_id, issuer, instrument_no, series, title, \
                 scope_tags, sha256_hex, state, corpus_path, official_url, notes, \
                 supersedes_id, in_force_from, in_force_until)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL)",
            params![
                new.anchor_id,
                new.issuer,
                new.instrument_no,
                new.series,
                new.title,
                new.scope_tags_json,
                new.sha256_hex,
                new.state.as_str(),
                new.corpus_path,
                new.official_url,
                new.notes,
                old_id,
                new.in_force_from,
            ],
        )
        .map_err(|e| format!("supersede insert for '{anchor_id}': {e}"))?;
        let new_row_id = tx.last_insert_rowid();

        let detail = format!(
            "supersede anchor={anchor_id} old_row={old_id} new_row={new_row_id} \
             until={}",
            new.in_force_from
        );
        let audit_row_id = record_tenant_checked(
            tx,
            AuditKind::PolicyAnchor,
            ACTOR,
            &format!("anchor:{anchor_id}"),
            AuditStatus::Ok,
            &detail,
            crate::audit::DEFAULT_TENANT,
        )
        .map_err(|e| format!("supersede audit row failed (the act refuses, nothing lands): {e}"))?;

        Ok(SupersedeReport {
            old_row_id: old_id,
            new_row_id,
            in_force_until: new.in_force_from.clone(),
            audit_row_id,
        })
    })
}

// ── state_as_of ──────────────────────────────────────────────────────────

/// One anchor's state as of a moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorState {
    pub anchor_id: String,
    pub title: String,
    pub state: AnchorLifecycle,
    pub sha256_hex: Option<String>,
    pub in_force_from: String,
    pub in_force_until: Option<String>,
}

/// The registry's deterministic quarter answer: a row is IN FORCE at `T`
/// iff `in_force_from <= T` and (`in_force_until` IS NULL OR
/// `in_force_until > T`) and its state is neither superseded nor retired.
/// Pure, deterministic, unit-drivable — this is the function K-30's
/// as-of-the-quarter export will build on.
pub fn state_as_of(conn: &Connection, as_of: &str) -> Result<Vec<AnchorState>, String> {
    validate_date("as_of", "--as-of", as_of).map_err(|e| format!("state_as_of: {e}"))?;
    let mut stmt = conn
        .prepare(
            "SELECT anchor_id, title, state, sha256_hex, in_force_from, in_force_until
             FROM policy_anchor
             WHERE state NOT IN ('superseded','retired')
               AND in_force_from <= ?1
               AND (in_force_until IS NULL OR in_force_until > ?1)
             ORDER BY anchor_id",
        )
        .map_err(|e| format!("state_as_of prepare: {e}"))?;
    let rows = stmt
        .query_map(params![as_of], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| format!("state_as_of query: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("state_as_of rows: {e}"))?;
    Ok(rows
        .into_iter()
        .map(
            |(anchor_id, title, state, sha256_hex, in_force_from, in_force_until)| AnchorState {
                state: AnchorLifecycle::parse(&state)
                    .expect("the schema CHECK holds the vocabulary"),
                anchor_id,
                title,
                sha256_hex,
                in_force_from,
                in_force_until,
            },
        )
        .collect())
}

// ── pack gate ────────────────────────────────────────────────────────────

/// Why one required anchor blocks its pack. Each reason renders a distinct
/// sentence; the fetch-owed sentence is the advisory the operator must not
/// mistake for a verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateBlockerReason {
    NotInRegistry,
    TextNotHeld,
    Superseded,
    Retired,
    NotInForce { in_force_from: String },
}

impl GateBlockerReason {
    pub fn text(&self) -> String {
        match self {
            GateBlockerReason::NotInRegistry => "not in the registry".to_string(),
            GateBlockerReason::TextNotHeld => "advisory-only — anchor text not held".to_string(),
            GateBlockerReason::Superseded => {
                "superseded — the registry holds a later version of this instrument".to_string()
            }
            GateBlockerReason::Retired => "retired from the registry".to_string(),
            GateBlockerReason::NotInForce { in_force_from } => {
                format!("not in force as of the queried date (in force from {in_force_from})")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateBlocker {
    pub anchor_id: String,
    pub reason: GateBlockerReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateAnchorFact {
    pub anchor_id: String,
    pub title: String,
    pub sha256_hex: Option<String>,
    pub in_force_from: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateVerdict {
    pub pack_id: String,
    pub passed: bool,
    pub verified: Vec<GateAnchorFact>,
    pub blockers: Vec<GateBlocker>,
}

/// The pack gate's outcome. `NoBindingsConfigured` is the fail-closed arm
/// for an unset knob (or a pack the knob does not name): NO bindings is
/// never "everything allowed" — an unconfigured gate refuses, naming the
/// knob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackGateOutcome {
    NoBindingsConfigured,
    Decided(GateVerdict),
}

/// The one-screen decision: may `pack_id` activate?
///
/// With `required` = None (or an empty slice) the outcome is
/// [`PackGateOutcome::NoBindingsConfigured`] — the knob's absence is itself
/// a refusal, fail-closed. With bindings, EVERY required anchor is resolved
/// against the registry and EVERY blocker is named (never just the first):
/// missing, fetch-owed, superseded, retired, or not yet in force. The
/// verdict is `passed` only when every required anchor is a live,
/// in-force, verified_text_held row.
///
/// A decided verdict is a WRITE act: the binding rows
/// (`policy_pack_binding`) and one [`AuditKind::PolicyAnchor`] row land in
/// ONE `BEGIN IMMEDIATE` transaction, so the durable record of what a gate
/// decision was made against exists beside its evidence. The binding rows
/// are idempotent (the pack's primary key); a repeat gate over the same
/// bindings records the new act and adds no duplicate rows.
pub fn pack_gate(
    conn: &Connection,
    pack_id: &str,
    required: Option<&[String]>,
) -> Result<PackGateOutcome, String> {
    let Some(required) = required.filter(|r| !r.is_empty()) else {
        return Ok(PackGateOutcome::NoBindingsConfigured);
    };

    immediate_tx(conn, "pack gate", |tx| {
        let mut verified = Vec::new();
        let mut blockers = Vec::new();
        for anchor_id in required {
            let row: Option<(String, String, Option<String>, String)> = tx
                .query_row(
                    "SELECT state, title, sha256_hex, in_force_from
                     FROM policy_anchor WHERE anchor_id = ?1
                       AND state IN ('verified_text_held','text_fetch_owed')",
                    params![anchor_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()
                .map_err(|e| format!("pack gate read {anchor_id}: {e}"))?;

            let Some((state, title, sha, from)) = row else {
                // No live row: distinguish a history-only lineage (superseded)
                // from an anchor the registry has never heard of. The NEWEST
                // history row's state names the reason (deterministic).
                let history: Option<String> = tx
                    .query_row(
                        "SELECT state FROM policy_anchor WHERE anchor_id = ?1
                           AND state IN ('superseded','retired')
                         ORDER BY id DESC LIMIT 1",
                        params![anchor_id],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| format!("pack gate history read {anchor_id}: {e}"))?;
                let reason = match history.as_deref() {
                    Some("superseded") => GateBlockerReason::Superseded,
                    Some("retired") => GateBlockerReason::Retired,
                    _ => GateBlockerReason::NotInRegistry,
                };
                blockers.push(GateBlocker {
                    anchor_id: anchor_id.clone(),
                    reason,
                });
                continue;
            };

            let lifecycle = AnchorLifecycle::parse(&state).ok_or_else(|| {
                format!(
                    "pack gate: anchor '{anchor_id}' carries state '{state}' outside the registry's vocabulary"
                )
            })?;
            match lifecycle {
                AnchorLifecycle::VerifiedTextHeld => {
                    verified.push(GateAnchorFact {
                        anchor_id: anchor_id.clone(),
                        title,
                        sha256_hex: sha,
                        in_force_from: from,
                    });
                }
                AnchorLifecycle::TextFetchOwed => {
                    blockers.push(GateBlocker {
                        anchor_id: anchor_id.clone(),
                        reason: GateBlockerReason::TextNotHeld,
                    });
                }
                AnchorLifecycle::Superseded | AnchorLifecycle::Retired => unreachable!(
                    "the live-row query filters to the live states"
                ),
            }
        }

        let passed = blockers.is_empty();
        let verdict = GateVerdict {
            pack_id: pack_id.to_string(),
            passed,
            verified,
            blockers,
        };

        // The durable record of what this decision was made against, and the
        // act's evidence row — one transaction, the SAME discipline as the
        // seed.
        for fact in &verdict.verified {
            let row_id: i64 = tx
                .query_row(
                    "SELECT id FROM policy_anchor WHERE anchor_id = ?1
                       AND state IN ('verified_text_held','text_fetch_owed')",
                    params![fact.anchor_id],
                    |r| r.get(0),
                )
                .map_err(|e| format!("pack gate binding read {}: {e}", fact.anchor_id))?;
            tx.execute(
                "INSERT OR IGNORE INTO policy_pack_binding(pack_id, anchor_row_id) VALUES (?1, ?2)",
                params![pack_id, row_id],
            )
            .map_err(|e| format!("pack gate binding write: {e}"))?;
        }
        for blocker in &verdict.blockers {
            // A blocker still records the BINDING ATTEMPT when the row exists
            // in any state; an anchor absent from the registry binds nothing
            // (there is no row id to bind — the refusal is the record, in the
            // audit).
            let row_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM policy_anchor WHERE anchor_id = ?1
                     ORDER BY id DESC LIMIT 1",
                    params![blocker.anchor_id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| {
                    format!(
                        "pack gate blocker binding read {}: {e}",
                        blocker.anchor_id
                    )
                })?;
            if let Some(row_id) = row_id {
                tx.execute(
                    "INSERT OR IGNORE INTO policy_pack_binding(pack_id, anchor_row_id) VALUES (?1, ?2)",
                    params![pack_id, row_id],
                )
                .map_err(|e| format!("pack gate blocker binding write: {e}"))?;
            }
        }

        let detail = format!(
            "pack-gate pack={pack_id} verdict={} required={} blockers={} bound={}",
            if passed { "verified" } else { "refused" },
            required.len(),
            verdict.blockers.len(),
            verdict.verified.len(),
        );
        record_tenant_checked(
            tx,
            AuditKind::PolicyAnchor,
            ACTOR,
            &format!("pack:{pack_id}"),
            if passed {
                AuditStatus::Ok
            } else {
                AuditStatus::Denied
            },
            &detail,
            crate::audit::DEFAULT_TENANT,
        )
        .map_err(|e| format!("pack gate audit row failed (the act refuses, nothing lands): {e}"))?;

        Ok(verdict)
    })
    .map(PackGateOutcome::Decided)
}

// ── verify_held_text (the corpus-drift detector) ─────────────────────────

/// One drifted or missing held text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldTextFault {
    pub anchor_id: String,
    pub path: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifyReport {
    pub checked: usize,
    pub ok: usize,
    pub faults: Vec<HeldTextFault>,
}

impl VerifyReport {
    pub fn is_clean(&self) -> bool {
        self.faults.is_empty()
    }
}

/// Re-hash every live `verified_text_held` row's corpus file against its
/// stored digest — the CLI verify's corpus-drift detector. Read-only: a
/// registry that says `verified_text_held` must still be TRUE of the bytes
/// on disk, and only a re-measurement proves it.
pub fn verify_held_text(conn: &Connection, corpus_root: &Path) -> Result<VerifyReport, String> {
    let mut stmt = conn
        .prepare(
            "SELECT anchor_id, sha256_hex, corpus_path FROM policy_anchor
             WHERE state = 'verified_text_held'
             ORDER BY anchor_id",
        )
        .map_err(|e| format!("verify prepare: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| format!("verify query: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("verify rows: {e}"))?;

    let mut report = VerifyReport::default();
    for (anchor_id, sha, path) in rows {
        report.checked += 1;
        let (Some(sha), Some(rel)) = (sha, path) else {
            report.faults.push(HeldTextFault {
                anchor_id,
                path: "not held".to_string(),
                expected: None,
                actual: None,
            });
            continue;
        };
        let file = corpus_root.join(&rel);
        match std::fs::read(&file) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual.eq_ignore_ascii_case(&sha) {
                    report.ok += 1;
                } else {
                    report.faults.push(HeldTextFault {
                        anchor_id,
                        path: file.display().to_string(),
                        expected: Some(sha),
                        actual: Some(actual),
                    });
                }
            }
            Err(_) => report.faults.push(HeldTextFault {
                anchor_id,
                path: file.display().to_string(),
                expected: Some(sha),
                actual: None,
            }),
        }
    }
    Ok(report)
}

// ── the read the CLI's table prints ──────────────────────────────────────

/// Every live anchor row, ordered by id — the CLI verify/seed tables' read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorRow {
    pub id: i64,
    pub anchor_id: String,
    pub issuer: String,
    pub instrument_no: String,
    pub series: String,
    pub title: String,
    pub sha256_hex: Option<String>,
    pub state: AnchorLifecycle,
    pub in_force_from: String,
    pub in_force_until: Option<String>,
}

pub fn list_live(conn: &Connection) -> Result<Vec<AnchorRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, anchor_id, issuer, instrument_no, series, title, sha256_hex, state, \
                 in_force_from, in_force_until
             FROM policy_anchor
             WHERE state IN ('verified_text_held','text_fetch_owed')
             ORDER BY id",
        )
        .map_err(|e| format!("list prepare: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(|e| format!("list query: {e}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| format!("list rows: {e}"))?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                anchor_id,
                issuer,
                instrument_no,
                series,
                title,
                sha256_hex,
                state,
                from,
                until,
            )| {
                AnchorRow {
                    id,
                    anchor_id,
                    issuer,
                    instrument_no,
                    series,
                    title,
                    sha256_hex,
                    state: AnchorLifecycle::parse(&state)
                        .expect("the schema CHECK holds the vocabulary"),
                    in_force_from: from,
                    in_force_until: until,
                }
            },
        )
        .collect())
}
