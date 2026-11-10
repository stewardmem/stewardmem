//! The research watch (IMPROVE08): deterministic lanes → HITL proposals.
//!
//! THE DETERMINISTIC ZERO-TOKEN LAW. The tick's DECISIONS are a pure
//! function: `(lane, feed items, seen-state) → (proposals, accounting)` —
//! model-absent. Same feed state → same proposal set, across model
//! versions, across runs (RW-4). The plan's embedder near-dup is
//! DEFERRED (a model dependency in the tick hot path would break that
//! acceptance); what ships is the normalized-title near-key, hashed —
//! labeled on the proposal so the human sees same-story items, nothing
//! silently dropped.
//!
//! THE WATCH FILES PROPOSALS, NEVER MEMORY (RW-1). A lane's `destination`
//! is the CLOSED vocabulary with ONE value, `proposals` — there is no
//! memory destination for a lane to name, and this module holds no
//! knowledge INSERT at all. The HITL gate (`handlers::gate`) is the only
//! door to memory.
//!
//! FEED CONTENT IS DATA, NEVER INSTRUCTIONS. Item text is quarantined
//! into the proposal's content body as evidence bytes; the deterministic
//! screen runs at filing (the proposal seam's own law), and the threat
//! models are StateFence (arXiv:2611.02605 — evidence-chain contamination)
//! and MemHarmBench (arXiv:2611.01700 — cross-session poisoning via web
//! content). Nothing parsed here is ever executed as an instruction.
//!
//! NO DATA LEAVES EXCEPT PER-LANE OPERATOR-NAMED HOSTS (RW-5). A lane row
//! names its feed URL; lane create validates it (https, no userinfo, host
//! screened through `webhook::resolve_and_validate_sink` — no private or
//! metadata addresses) and the tick's client is pinned to THAT host. The
//! probe hosts (export.arxiv.org, www.cisa.gov, lobste.rs, www.nist.gov)
//! are lane DATA, never an allowlist in this file.
//!
//! THE PUBLISHERS' ETIQUETTE IS BY CONSTRUCTION (RW-8). The arXiv API
//! User's Manual (read 2026-11-09): "incorporate a 3 second delay"
//! between calls, and "there is no need to call the API more than once in
//! a day for the same query — cache your results". So the arXiv lane's
//! cadence is DAILY by law (refused at create below the floor; a re-tick
//! inside the window is the honest `skipped_fresh` accounting line, never
//! a second fetch), the between-calls delay is a named constant, and an
//! RSS lane honors the publisher's `<ttl>` cache hint with the recorded
//! ground formula `min(cadence, ttl×60)` — all three land as
//! `skipped_fresh` accounting, never as a second fetch.
//!
//! THE SCHEMA IS THE ROUND'S OWN TABLES (1.32.32). `watch_configs` (lane
//! rows), `watch_seen` (the `(lane_id, item_key)` identity + the near-key,
//! append-only), `watch_ticks` (the honest per-tick accounting). This
//! module writes ONLY these three tables plus the proposal queue — never
//! `knowledge`.

use std::collections::HashSet;
use std::fmt;

use rusqlite::{Connection, params};
use xxhash_rust::xxh3::xxh3_64;

// ── the closed vocabularies ─────────────────────────────────────────────

/// The lane source-kind vocabulary (closed): the arXiv Atom API lane, the
/// generic RSS 2.0 lane, the generic Atom lane, and the plan-07 scholarly
/// adapter lane. An unknown value is the named refusal, never a silent
/// "generic feed".
pub const WATCH_SOURCE_KINDS: &[&str] = &["arxiv_api", "rss", "atom", "adapter"];

/// The destination vocabulary (closed, ONE value — RW-1): the watch's
/// only product is a well-labeled human queue. There is no memory
/// destination for a lane to name.
pub const WATCH_DESTINATIONS: &[&str] = &["proposals"];

/// The feed byte cap: the chunked read accumulates to this, then the named
/// `watch_feed_too_large` refusal. CISA's `all.xml` is 5.7 MB live — over
/// provider_http's 4 MiB `DEFAULT_MAX_RESPONSE_BYTES` precedent, so the
/// watch carries its own named cap with headroom (the ground correction).
pub const WATCH_MAX_FEED_BYTES: usize = 16 * 1024 * 1024;

/// The arXiv API User's Manual's inter-call delay: "incorporate a 3 second
/// delay" between consecutive calls to the API.
pub const WATCH_ARXIV_MIN_INTERVAL_SECS: u64 = 3;

/// The arXiv once-per-day lane law (the manual, verbatim intent: "there is
/// no need to call the API more than once in a day for the same query —
/// cache your results"). An arXiv lane's cadence is DAILY at minimum;
/// sub-daily is refused at create, and a re-tick inside the window is the
/// honest `skipped_fresh` accounting line, never a second fetch.
pub const WATCH_ARXIV_CADENCE_SECS: u64 = 86_400;

/// A lane name is a short trust LABEL (the source-label bound law): non-
/// empty, no control bytes, bounded.
const WATCH_LANE_NAME_MAX: usize = 64;

/// Cadence bounds: minutes to a month. The tick is externally driven
/// (cron/CLI — never a daemon), so the cadence is the no-refetch floor.
const WATCH_CADENCE_MIN_SECS: u64 = 60;
const WATCH_CADENCE_MAX_SECS: u64 = 2_678_400;

/// Proposal-content bounds the core renders: the feed is DATA and the
/// queue is for humans — a truncated abstract with the item's link is the
/// honest shape (the full text lives at the link).
const WATCH_TITLE_MAX_CHARS: usize = 300;
const WATCH_SUMMARY_MAX_CHARS: usize = 4_000;

// ── the lane row ────────────────────────────────────────────────────────

/// The lane source kind, parsed from the closed [`WATCH_SOURCE_KINDS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// The arXiv Atom API (`export.arxiv.org/api/query`) — the documented
    /// lane, daily by law.
    ArxivApi,
    /// A generic RSS 2.0 feed (`<item>`, guid/link identity, RFC-822 dates).
    Rss,
    /// A generic Atom feed (`<entry>`, id identity, RFC-3339 dates).
    Atom,
    /// The plan-07 scholarly adapter lane (Europe PMC / Crossref / PubMed
    /// records through the scholar transport; the plan-07 provenance
    /// passthrough rides the proposal).
    Adapter,
}

impl SourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::ArxivApi => "arxiv_api",
            SourceKind::Rss => "rss",
            SourceKind::Atom => "atom",
            SourceKind::Adapter => "adapter",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, WatchError> {
        // The closed vocabulary is EXACT: no trim, no case folding — a
        // near-miss value refuses by name rather than silently parsing.
        match raw {
            "arxiv_api" => Ok(SourceKind::ArxivApi),
            "rss" => Ok(SourceKind::Rss),
            "atom" => Ok(SourceKind::Atom),
            "adapter" => Ok(SourceKind::Adapter),
            other => Err(WatchError::SourceKindInvalid(other.to_string())),
        }
    }
}

/// The lane destination. The closed vocabulary with ONE value (RW-1): the
/// watch files proposals, and the HITL gate is the only door to memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Proposals,
}

impl Destination {
    pub fn as_str(self) -> &'static str {
        "proposals"
    }

    pub fn parse(raw: &str) -> Result<Self, WatchError> {
        // The closed vocabulary is EXACT: one value, no trim, no case
        // folding. Anything else refuses by name (RW-1).
        match raw {
            "proposals" => Ok(Destination::Proposals),
            _ => Err(WatchError::DestinationInvalid),
        }
    }
}

/// A validated lane spec (the create-time shape).
#[derive(Debug, Clone)]
pub struct LaneSpec<'a> {
    pub name: &'a str,
    pub source_kind: &'a str,
    pub feed_url: &'a str,
    pub query: Option<&'a str>,
    pub cadence_secs: u64,
    pub destination: &'a str,
}

/// The lane row as the tick surface loads it.
#[derive(Debug, Clone)]
pub struct WatchLane {
    pub id: i64,
    pub name: String,
    pub source_kind: SourceKind,
    pub feed_url: String,
    pub host: String,
    pub query: Option<String>,
    pub cadence_secs: u64,
    pub destination: Destination,
    /// The publisher's RSS `<ttl>` cache hint (MINUTES as published,
    /// stored in seconds), learned from the last successful parse. None
    /// until the first fetch — the floor is then the lane's own cadence.
    pub ttl_secs: Option<u64>,
    pub enabled: bool,
}

// ── the named refusals ──────────────────────────────────────────────────

/// The watch module's named refusals. Fixed strings; no configuration
/// values, credentials, or feed content ever ride an error message (the
/// scholar law — feed text is untrusted data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchError {
    /// The feed URL carried no usable https host (or was not https, or
    /// carried userinfo) — the lane-create refusal.
    LaneHostRequired,
    /// The host failed the SSRF screen (`watch_lane_host_refused`): a
    /// private, reserved, or cloud-metadata address. The egress screen's
    /// own reason string rides (it names the host + class; the host is the
    /// operator's own input).
    LaneHostRefused(String),
    /// The lane name was empty, over-bound, or control-bearing.
    LaneNameInvalid,
    /// The source kind was outside [`WATCH_SOURCE_KINDS`].
    SourceKindInvalid(String),
    /// The destination was outside [`WATCH_DESTINATIONS`] — RW-1's
    /// structural face: there is no memory destination to name.
    DestinationInvalid,
    /// The cadence was outside the minutes-to-a-month bounds.
    CadenceInvalid,
    /// An arXiv lane below the documented daily law — refused at create.
    ArxivCadenceTooShort,
    /// The feed exceeded [`WATCH_MAX_FEED_BYTES`] (the chunked read's cap).
    FeedTooLarge,
    /// The feed carried `<!DOCTYPE` or `<!ENTITY` — refused BY NAME,
    /// never expanded (the XXE posture).
    FeedDtdRefused,
    /// The feed bytes were not parseable XML-shaped text.
    FeedMalformed,
    /// Neither Atom `<entry>` nor RSS `<item>` in the parsed feed.
    FeedWrongSchema,
    /// The arXiv API returned its error-as-Atom shape (a single entry
    /// whose id starts `http://arxiv.org/api/errors#`). Never parsed as
    /// zero items.
    ArxivErrorEntry(String),
    /// The replay transport had no fixture for this URL (SA-4 — CI
    /// replays the frozen corpus only).
    ReplayGap,
    /// The live transport's pre-I/O pin refusal: the URL's host is not the
    /// lane's pinned host — a foreign fetch never reaches the client (RW-5).
    FetchHostMismatch,
    /// The live fetch failed (transport error, non-success status, or a
    /// read error inside the chunked loop). Fixed string — the wire's
    /// bytes never ride the error.
    TransportFailed,
    /// The lane is disabled — the tick refuses (an enabled=false lane is
    /// config, not a fetch target).
    LaneDisabled,
    /// The CHECKED audit write failed — the tick FAILS, named (RW-3): the
    /// whole transaction rolls back, nothing is half-recorded.
    AuditWriteFailed,
    /// The synthesis provider's response exceeded the named bound
    /// (`research_output_too_large`).
    SynthesisOutputTooLarge,
    /// A storage error inside the tick's transaction.
    Db(String),
}

impl WatchError {
    /// The stable wire code (the named-refusal law: the route maps this to
    /// the HTTP error body's `code`; the text is fixed, never dynamic).
    pub fn code(&self) -> &'static str {
        match self {
            WatchError::LaneHostRequired => "watch_lane_host_required",
            WatchError::LaneHostRefused(_) => "watch_lane_host_refused",
            WatchError::LaneNameInvalid => "watch_lane_name_invalid",
            WatchError::SourceKindInvalid(_) => "watch_source_kind_invalid",
            WatchError::DestinationInvalid => "watch_destination_invalid",
            WatchError::CadenceInvalid => "watch_cadence_invalid",
            WatchError::ArxivCadenceTooShort => "watch_arxiv_cadence_too_short",
            WatchError::FeedTooLarge => "watch_feed_too_large",
            WatchError::FeedDtdRefused => "watch_feed_dtd_refused",
            WatchError::FeedMalformed => "watch_feed_malformed",
            WatchError::FeedWrongSchema => "watch_feed_wrong_schema",
            WatchError::ArxivErrorEntry(_) => "watch_arxiv_error_entry",
            WatchError::ReplayGap => "watch_replay_gap",
            WatchError::FetchHostMismatch => "watch_fetch_host_mismatch",
            WatchError::TransportFailed => "watch_transport_failed",
            WatchError::LaneDisabled => "watch_lane_disabled",
            WatchError::AuditWriteFailed => "watch_audit_write_failed",
            WatchError::SynthesisOutputTooLarge => "research_output_too_large",
            WatchError::Db(_) => "watch_db_error",
        }
    }
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchError::LaneHostRequired => write!(
                f,
                "watch_lane_host_required: feed_url must be https with a host and no userinfo"
            ),
            WatchError::LaneHostRefused(reason) => {
                write!(f, "watch_lane_host_refused: {reason}")
            }
            WatchError::LaneNameInvalid => write!(
                f,
                "watch_lane_name_invalid: name must be 1..={WATCH_LANE_NAME_MAX} printable chars"
            ),
            WatchError::SourceKindInvalid(v) => write!(
                f,
                "watch_source_kind_invalid: '{}' is not one of {}",
                v,
                WATCH_SOURCE_KINDS.join("|")
            ),
            WatchError::DestinationInvalid => write!(
                f,
                "watch_destination_invalid: the only destination is \"proposals\" — the watch \
                 files proposals, never memory (RW-1)"
            ),
            WatchError::CadenceInvalid => write!(
                f,
                "watch_cadence_invalid: cadence must be {WATCH_CADENCE_MIN_SECS}..=\
                 {WATCH_CADENCE_MAX_SECS} seconds"
            ),
            WatchError::ArxivCadenceTooShort => write!(
                f,
                "watch_arxiv_cadence_too_short: the arXiv API manual's law is once per day \
                 per query — cadence must be at least {WATCH_ARXIV_CADENCE_SECS} seconds"
            ),
            WatchError::FeedTooLarge => write!(
                f,
                "watch_feed_too_large: the feed exceeded {WATCH_MAX_FEED_BYTES} bytes"
            ),
            WatchError::FeedDtdRefused => write!(
                f,
                "watch_feed_dtd_refused: the feed carries a DTD or entity declaration — \
                 refused by name, never expanded"
            ),
            WatchError::FeedMalformed => {
                write!(
                    f,
                    "watch_feed_malformed: the feed bytes are not XML-shaped text"
                )
            }
            WatchError::FeedWrongSchema => write!(
                f,
                "watch_feed_wrong_schema: neither Atom <entry> nor RSS <item> found"
            ),
            WatchError::ArxivErrorEntry(_id) => write!(
                f,
                "watch_arxiv_error_entry: the arXiv API returned its error-as-Atom shape \
                 (errors are never parsed as zero items)"
            ),
            WatchError::ReplayGap => write!(
                f,
                "watch_replay_gap: no recorded fixture for this URL — CI replays the frozen \
                 corpus only"
            ),
            WatchError::FetchHostMismatch => write!(
                f,
                "watch_fetch_host_mismatch: the feed URL's host is not the lane's pinned host"
            ),
            WatchError::TransportFailed => {
                write!(f, "watch_transport_failed: the fetch failed")
            }
            WatchError::LaneDisabled => {
                write!(
                    f,
                    "watch_lane_disabled: the lane is disabled — enable it first"
                )
            }
            WatchError::AuditWriteFailed => write!(
                f,
                "watch_audit_write_failed: the tick's CHECKED accounting insert failed — the \
                 tick fails, the transaction rolls back (RW-3)"
            ),
            WatchError::SynthesisOutputTooLarge => write!(
                f,
                "research_output_too_large: the provider's response exceeded the named bound"
            ),
            WatchError::Db(e) => write!(f, "watch_db_error: {e}"),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<rusqlite::Error> for WatchError {
    fn from(e: rusqlite::Error) -> Self {
        WatchError::Db(e.to_string())
    }
}

// ── lane validation + create ────────────────────────────────────────────

/// The PURE half of lane create: the closed vocabularies, the name bounds,
/// the cadence bounds (with the arXiv daily law), and the URL law (https,
/// a host, no userinfo). Returns `(name, kind, feed_url, host, destination)`.
/// DNS/SSRF screening is the separate [`screen_lane_host`] half so this
/// stays testable without a resolver.
pub fn validate_lane_spec(
    spec: &LaneSpec<'_>,
) -> Result<(String, SourceKind, String, String, Destination), WatchError> {
    let name = spec.name.trim();
    if name.is_empty()
        || name.chars().count() > WATCH_LANE_NAME_MAX
        || name.chars().any(char::is_control)
    {
        return Err(WatchError::LaneNameInvalid);
    }
    let kind = SourceKind::parse(spec.source_kind)?;
    let destination = Destination::parse(spec.destination)?;
    if !(WATCH_CADENCE_MIN_SECS..=WATCH_CADENCE_MAX_SECS).contains(&spec.cadence_secs) {
        return Err(WatchError::CadenceInvalid);
    }
    if kind == SourceKind::ArxivApi && spec.cadence_secs < WATCH_ARXIV_CADENCE_SECS {
        // The manual's cadence law, enforced where the operator set it.
        return Err(WatchError::ArxivCadenceTooShort);
    }
    // The URL law: https only, a host, no userinfo (a credential in the
    // URL is a leaked credential AND a pin bypass).
    let url =
        reqwest::Url::parse(spec.feed_url.trim()).map_err(|_| WatchError::LaneHostRequired)?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err(WatchError::LaneHostRequired);
    }
    let host = url
        .host_str()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .ok_or(WatchError::LaneHostRequired)?
        .to_ascii_lowercase();
    Ok((
        name.to_string(),
        kind,
        spec.feed_url.trim().to_string(),
        host,
        destination,
    ))
}

/// The SSRF half of lane create: the ONE egress screen
/// (`webhook::resolve_and_validate_sink` with `allow_private=false`). The
/// literal-IP paths need no DNS; a hostname resolves here (lane create is
/// an operator act, never the tick hot path — the resolution is also the
/// boot-frozen egress pin's first entry, so a later rebinding loses).
pub fn screen_lane_host(host: &str) -> Result<(), WatchError> {
    crate::webhook::resolve_and_validate_sink(host, 443, false)
        .map(|_| ())
        .map_err(|e| WatchError::LaneHostRefused(e.to_string()))
}

/// Lane create: validate (pure) + screen (SSRF) + the one guarded INSERT.
/// `created_at` is a PARAMETER (the scholar law — a fixed input keeps the
/// write reproducible in tests). Returns the lane id.
pub fn create_lane(
    conn: &Connection,
    spec: &LaneSpec<'_>,
    created_at: &str,
) -> Result<i64, WatchError> {
    let (name, kind, feed_url, host, destination) = validate_lane_spec(spec)?;
    screen_lane_host(&host)?;
    let query = spec.query.map(str::trim).filter(|q| !q.is_empty());
    conn.query_row(
        "INSERT INTO watch_configs(name, source_kind, feed_url, host, query, cadence_secs, \
                                destination, ttl_secs, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 1, ?8)
         RETURNING id",
        params![
            name,
            kind.as_str(),
            feed_url,
            host,
            query,
            spec.cadence_secs as i64,
            destination.as_str(),
            created_at,
        ],
        |r| r.get(0),
    )
    .map_err(WatchError::from)
}

/// Load a lane row by name (the tick surface's lookup).
pub fn lane_by_name(conn: &Connection, name: &str) -> Result<Option<WatchLane>, WatchError> {
    let raw = conn
        .query_row(
            "SELECT id, name, source_kind, feed_url, host, query, cadence_secs, destination, \
                    ttl_secs, enabled
             FROM watch_configs WHERE name = ?1",
            [name],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                    r.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()
        .map_err(WatchError::from)?;
    raw.map(
        |(id, name, kind_raw, feed_url, host, query, cadence, dest_raw, ttl, enabled)| {
            Ok(WatchLane {
                id,
                name,
                source_kind: SourceKind::parse(&kind_raw).map_err(|_| {
                    WatchError::Db(format!("corrupt lane row: source_kind {kind_raw:?}"))
                })?,
                feed_url,
                host,
                query,
                cadence_secs: cadence.max(0) as u64,
                destination: Destination::parse(&dest_raw).map_err(|_| {
                    WatchError::Db(format!("corrupt lane row: destination {dest_raw:?}"))
                })?,
                ttl_secs: ttl.map(|v| v.max(0) as u64),
                enabled: enabled != 0,
            })
        },
    )
    .transpose()
}

use rusqlite::OptionalExtension;

// ── feed items + identity ───────────────────────────────────────────────

/// The plan-07 provenance passthrough (an ADAPTER lane's record fields).
/// The same vocabulary `scholar_meta` carries — the proposal header renders
/// it so the human reviews the retraction flag at approve time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterMeta {
    pub source_api: String,
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub license: Option<String>,
    pub retracted: bool,
}

/// One feed item as the parsers emit it — the tick core's input unit.
/// ALL text fields are DATA (evidence bytes), never instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedItem {
    /// The feed's own identity: the arXiv id WITH version for API lanes,
    /// `<guid>` else the normalized `<link>` for RSS, the `<id>` for Atom,
    /// the canonical identifier for adapter lanes. Empty = no identity →
    /// the `failed` accounting line (never filed, never deduped).
    pub identity: String,
    pub title: String,
    pub link: Option<String>,
    /// Normalized RFC-3339 (parsers convert RFC-822 at the edge).
    pub published: Option<String>,
    pub updated: Option<String>,
    pub summary: Option<String>,
    pub authors: Vec<String>,
    pub categories: Vec<String>,
    pub adapter: Option<AdapterMeta>,
}

/// The named per-lane hash (the kernel's own xxh3-64, already locked —
/// zero new dependencies): fixed-length, deterministic, model-absent.
/// The RAW identity never rides the seen-table (the proposal header
/// carries it for the human; the hash recipe is in the fixture README).
fn hash_key(s: &str) -> String {
    format!("{:016x}", xxh3_64(s.as_bytes()))
}

/// The normalized-title near-key: lowercase, punctuation-stripped,
/// whitespace-collapsed, hashed. Same story, different URL → same
/// near-key, LABELED on the proposal (the human sees it; nothing is
/// silently dropped). Model-absent by construction.
pub fn near_key(title: &str) -> String {
    let mut norm = String::with_capacity(title.len());
    let mut last_ws = true;
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            norm.extend(ch.to_lowercase());
            last_ws = false;
        } else if !last_ws {
            norm.push(' ');
            last_ws = true;
        }
    }
    hash_key(norm.trim())
}

// ── the pure tick core ──────────────────────────────────────────────────

/// The honest per-tick accounting (plan §1.1): every tick reports what it
/// saw, what was already seen, what it filed, what failed, and what was
/// skipped for freshness. Rides every tick result — the loop's health is
/// measurable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickAccounting {
    pub items_seen: u64,
    pub deduped: u64,
    pub filed: u64,
    pub failed: u64,
    pub skipped_fresh: u64,
}

impl TickAccounting {
    /// The stable detail string the tick's CHECKED audit row carries
    /// (fixed vocabulary, no feed content).
    pub fn detail(&self) -> String {
        format!(
            "seen={} deduped={} filed={} failed={} skipped_fresh={}",
            self.items_seen, self.deduped, self.filed, self.failed, self.skipped_fresh
        )
    }
}

/// The seen-state addition the tick's transaction inserts into
/// `watch_seen`: the hashed `(lane_id, item_key)` identity + the near-key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeenAddition {
    pub item_key: String,
    pub near_key: String,
}

/// The proposal the tick files through the gate — the deterministic
/// provenance header + the item text (DATA). `identity` is the RAW feed
/// identity (the header's `Item:` line); `item_key` is its hash (the
/// seen-table's key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchProposal {
    pub title: String,
    pub content: String,
    pub identity: String,
    pub item_key: String,
    pub near_key: String,
}

/// The tick core's outcome: the proposals to file + the seen additions to
/// insert + the accounting. A PURE function of its inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickOutcome {
    pub proposals: Vec<WatchProposal>,
    pub seen_additions: Vec<SeenAddition>,
    pub accounting: TickAccounting,
}

/// THE PURE TICK CORE: `(lane, feed items, seen-state, fetched-at) →
/// (proposals, accounting)`. No clock read, no network, no model — the
/// same inputs give the same proposal set across model versions and runs
/// (RW-4's "same feed state → same proposal set"). New identities file as
/// proposals; seen identities dedupe (counted, never dropped silently);
/// identity-less items land as `failed`. Feed order is preserved (the
/// queue reads chronologically).
pub fn tick_core(
    lane: &WatchLane,
    items: &[FeedItem],
    seen: &HashSet<String>,
    fetched_at: &str,
) -> TickOutcome {
    let mut outcome = TickOutcome {
        proposals: Vec::new(),
        seen_additions: Vec::new(),
        accounting: TickAccounting::default(),
    };
    outcome.accounting.items_seen = items.len() as u64;

    for item in items {
        if item.identity.trim().is_empty() {
            // No identity → not dedupeable → the failed line. Never filed
            // (it would re-file on every tick), never silently dropped
            // (the accounting says so).
            outcome.accounting.failed += 1;
            continue;
        }
        let item_key = hash_key(item.identity.trim());
        if seen.contains(&item_key) {
            outcome.accounting.deduped += 1;
            continue;
        }
        let near = near_key(&item.title);
        outcome.proposals.push(WatchProposal {
            title: bound_chars(&item.title, WATCH_TITLE_MAX_CHARS),
            content: render_proposal(lane, item, fetched_at, &near),
            identity: item.identity.trim().to_string(),
            item_key: item_key.clone(),
            near_key: near.clone(),
        });
        outcome.seen_additions.push(SeenAddition {
            item_key,
            near_key: near,
        });
        outcome.accounting.filed += 1;
    }
    outcome
}

/// Bound to N chars on char boundaries, with the honest ellipsis.
fn bound_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}…")
}

/// The deterministic provenance header + the item text. Fixed line
/// vocabulary (the human reviews THIS at approve time); adapter lanes
/// render the plan-07 passthrough + the retraction status line shape.
/// Feed text lands BELOW the header as DATA — the header lines are
/// kernel-rendered facts, the body is the quarantined evidence.
fn render_proposal(lane: &WatchLane, item: &FeedItem, fetched_at: &str, near: &str) -> String {
    let mut h = String::new();
    h.push_str(&format!("[Watch] lane: {}\n", lane.name));
    h.push_str(&format!(
        "Feed: {} ({})\n",
        lane.host,
        lane.source_kind.as_str()
    ));
    h.push_str(&format!("Item: {}\n", item.identity));
    h.push_str(&format!("Link: {}\n", item.link.as_deref().unwrap_or("-")));
    h.push_str(&format!(
        "Published: {}\n",
        item.published.as_deref().unwrap_or("-")
    ));
    h.push_str(&format!(
        "Authors: {}\n",
        if item.authors.is_empty() {
            "-".to_string()
        } else {
            item.authors.join(", ")
        }
    ));
    h.push_str(&format!(
        "Categories: {}\n",
        if item.categories.is_empty() {
            "-".to_string()
        } else {
            item.categories.join(", ")
        }
    ));
    h.push_str(&format!("Fetched: {fetched_at}\n"));
    h.push_str(&format!("Near-Key: {near}\n"));
    if let Some(a) = &item.adapter {
        h.push_str(&format!("source_api: {}\n", a.source_api));
        if let Some(doi) = &a.doi {
            h.push_str(&format!("doi: {doi}\n"));
        }
        if let Some(pmid) = &a.pmid {
            h.push_str(&format!("pmid: {pmid}\n"));
        }
        if let Some(lic) = &a.license {
            h.push_str(&format!("license: {lic}\n"));
        }
        // The plan-07 status line shape: the human sees the retraction
        // flag in the proposal they review. Recorded-never-floored here —
        // the machine floor (`scholar_meta_floor`) applies to knowledge
        // rows after approval, through the gate's own write path.
        h.push_str(&format!("Retracted: {}\n", a.retracted));
    }
    h.push('\n');
    h.push_str(&bound_chars(&item.title, WATCH_TITLE_MAX_CHARS));
    h.push_str("\n\n");
    if let Some(s) = &item.summary {
        h.push_str(&bound_chars(s, WATCH_SUMMARY_MAX_CHARS));
        h.push('\n');
    }
    h
}

// ── the freshness floors (RW-8's tick-time face) ────────────────────────

/// The lane's effective no-refetch floor, in seconds, BEFORE the fetch:
///
/// - arXiv API lanes: the create-guaranteed daily cadence (the manual's
///   once-per-day law — a re-tick inside the window is `skipped_fresh`,
///   never a second fetch).
/// - RSS/Atom lanes: the recorded ground formula `min(cadence, ttl×60)`
///   when the publisher's `<ttl>` hint is known (learned from the last
///   successful parse), else the lane's own cadence.
/// - adapter lanes: the lane's own cadence.
pub fn freshness_floor_secs(kind: SourceKind, cadence_secs: u64, ttl_secs: Option<u64>) -> u64 {
    match kind {
        SourceKind::ArxivApi => cadence_secs.max(WATCH_ARXIV_CADENCE_SECS),
        SourceKind::Rss | SourceKind::Atom => ttl_secs
            .map(|ttl| cadence_secs.min(ttl.saturating_mul(60)))
            .unwrap_or(cadence_secs),
        SourceKind::Adapter => cadence_secs,
    }
}

/// Stale = the lane MAY fetch: never ticked, or the last fetch is at/after
/// the floor ago. `false` = the tick honors the floor as the honest
/// `skipped_fresh` accounting line.
pub fn is_stale(floor_secs: u64, last_fetch_epoch: Option<i64>, now_epoch: i64) -> bool {
    last_fetch_epoch
        .map(|last| now_epoch.saturating_sub(last) >= floor_secs as i64)
        .unwrap_or(true)
}

// ── S2: the feed parsers (the hand-rolled std-side scanner) ─────────────
//
// NO XML CRATE IS LOCKED (the deny law — zero new dependencies), so the
// parser is a minimal tag-scanner over the TWO RECORDED GRAMMARS (Atom 1.0
// entries + RSS 2.0 items), exactly as the ground correction scopes it:
// local-name matching (namespace-PREFIX-tolerant — the NIST shape declares
// a dozen), one-level entity decode (the five XML built-ins — decoded HTML
// stays DATA, never instructions), CDATA passthrough, and the SECURITY
// REFUSALS: `<!DOCTYPE` / `<!ENTITY` refused BY NAME before any parsing
// (entities are never expanded — the XXE posture). Dates parse through the
// ALREADY-LOCKED chrono (`parse_from_rfc2822` for RSS, `parse_from_rfc3339`
// for Atom) — no new date code.
//
// Honest scope: the scanner reads the recorded grammars, not arbitrary XML.
// It does not resolve namespaces, does not handle the extreme XML corners
// (literal `>` inside attribute values, CDATA carrying raw same-name tags)
// — a document playing those games is outside the recorded shapes and
// fails one of the named refusals.

/// One parsed feed: the items (in feed order) + the channel's `<ttl>` cache
/// hint in SECONDS when the publisher declared one (RSS `<ttl>` is minutes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFeed {
    pub items: Vec<FeedItem>,
    pub ttl_secs: Option<u64>,
}

/// Parse a feed's bytes into [`ParsedFeed`]. The kind shapes the identity
/// rule: an [`SourceKind::ArxivApi`] lane strips the arXiv abs prefix from
/// the Atom `<id>`, KEEPING the version suffix (`2410.05451v3`); a generic
/// Atom lane takes `<id>` verbatim; an RSS lane takes `<guid>` else the
/// trimmed `<link>`; an adapter lane does not ride this parser (its records
/// come from the scholar transport, already shaped).
pub fn parse_feed(kind: SourceKind, bytes: &[u8]) -> Result<ParsedFeed, WatchError> {
    let text = std::str::from_utf8(bytes).map_err(|_| WatchError::FeedMalformed)?;
    // The security refusal BEFORE any parsing: a DTD or entity declaration
    // is refused by name and never expanded.
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err(WatchError::FeedDtdRefused);
    }
    // Schema dispatch: the Atom container else the RSS container; neither
    // is `watch_feed_wrong_schema` (an HTML error page, a JSON body).
    if text.contains("<feed") {
        let mut feed = ParsedFeed {
            items: parse_atom_entries(text)?,
            ttl_secs: None,
        };
        // The arXiv API's documented error shape: an entry whose id starts
        // the errors# prefix is the NAMED refusal — never parsed as zero
        // items, never silently dropped.
        for item in &feed.items {
            if item.identity.starts_with("http://arxiv.org/api/errors#") {
                return Err(WatchError::ArxivErrorEntry(item.identity.clone()));
            }
        }
        // The identity rule for the arXiv API lane: strip the abs prefix,
        // keep the version.
        if kind == SourceKind::ArxivApi {
            for item in &mut feed.items {
                for prefix in ["http://arxiv.org/abs/", "https://arxiv.org/abs/"] {
                    if let Some(stripped) = item.identity.strip_prefix(prefix) {
                        item.identity = stripped.to_string();
                        break;
                    }
                }
            }
        }
        Ok(feed)
    } else if text.contains("<rss") || text.contains("<channel") {
        parse_rss_channel(text)
    } else {
        Err(WatchError::FeedWrongSchema)
    }
}

/// Parse an Atom document's `<entry>` elements.
fn parse_atom_entries(text: &str) -> Result<Vec<FeedItem>, WatchError> {
    let mut items = Vec::new();
    for entry in extract_elements(text, "entry") {
        let id = element_text(entry, "id").unwrap_or_default();
        let title = element_text(entry, "title").unwrap_or_default();
        let published = date_rfc3339(element_text(entry, "published"));
        let updated = date_rfc3339(element_text(entry, "updated"));
        let summary = element_text(entry, "summary");
        let authors: Vec<String> = extract_elements(entry, "author")
            .iter()
            .filter_map(|a| element_text(a, "name"))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let categories: Vec<String> = extract_element_attrs(entry, "category")
            .iter()
            .filter_map(|(attrs, _)| attrs.get("term").cloned())
            .collect();
        // The links: rel="alternate" href is the item link.
        let link = extract_element_attrs(entry, "link")
            .into_iter()
            .find_map(|(attrs, _)| {
                let rel = attrs.get("rel").map(String::as_str);
                if rel.is_none() || rel == Some("alternate") {
                    attrs.get("href").cloned()
                } else {
                    None
                }
            });
        items.push(FeedItem {
            identity: id.trim().to_string(),
            title: title.trim().to_string(),
            link,
            published,
            updated,
            summary,
            authors,
            categories,
            adapter: None,
        });
    }
    Ok(items)
}

/// Parse an RSS document's `<item>` elements + the channel's `<ttl>`.
fn parse_rss_channel(text: &str) -> Result<ParsedFeed, WatchError> {
    // The balance law: an OPENED item never closed is a truncated document
    // — the named malformed refusal, not an honest empty state. (Entity-
    // encoded text never contains raw tags, so the count reads structure.)
    let opens = count_tag_opens(text, "item");
    let closes = count_tag_closes(text, "item");
    if opens != closes {
        return Err(WatchError::FeedMalformed);
    }
    let ttl_secs = extract_elements(text, "ttl")
        .first()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .map(|m| m.saturating_mul(60));
    let mut items = Vec::new();
    for item in extract_elements(text, "item") {
        let guid = element_text(item, "guid");
        let link_text = element_text(item, "link");
        let identity = guid
            .or_else(|| link_text.clone())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let title = element_text(item, "title").unwrap_or_default();
        let published = date_rfc2822(element_text(item, "pubDate"));
        let summary = element_text(item, "description");
        let mut authors: Vec<String> = extract_elements(item, "author")
            .iter()
            .map(|a| element_text(a, "name").unwrap_or_else(|| a.to_string()))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // dc:creator — local-name matching covers the prefixed form.
        for c in extract_elements(item, "creator") {
            let c = c.trim().to_string();
            if !c.is_empty() {
                authors.push(c);
            }
        }
        let categories: Vec<String> = extract_elements(item, "category")
            .iter()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        items.push(FeedItem {
            identity,
            title: title.trim().to_string(),
            link: link_text.map(|s| s.trim().to_string()),
            published,
            updated: None,
            summary,
            authors,
            categories,
            adapter: None,
        });
    }
    Ok(ParsedFeed { items, ttl_secs })
}

fn count_tag_opens(text: &str, local: &str) -> usize {
    count_tag_name(text, local, false)
}

fn count_tag_closes(text: &str, local: &str) -> usize {
    count_tag_name(text, local, true)
}

/// Count tags whose LOCAL name matches (prefix-tolerant): `<item …>`,
/// `<rss:item …>` (open) or `</item>`, `</rss:item>` (close).
fn count_tag_name(text: &str, local: &str, close: bool) -> usize {
    let mut n = 0;
    let mut rest = text;
    while let Some(pos) = rest.find('<') {
        let after = &rest[pos + 1..];
        let matches = if close {
            after.starts_with('/')
                && after[1..]
                    .trim_start()
                    .strip_prefix(local)
                    .is_some_and(|tail| {
                        tail.starts_with('>')
                            || tail.starts_with(':')
                            || tail.starts_with([' ', '\t', '\r', '\n'])
                    })
        } else {
            !after.starts_with('/')
                && after.strip_prefix(local).is_some_and(|tail| {
                    tail.starts_with('>')
                        || tail.starts_with([' ', '\t', '\r', '\n', '/'])
                        || tail.starts_with(':')
                })
        };
        if matches {
            n += 1;
        }
        rest = &rest[pos + 1..];
    }
    n
}

/// Every element whose LOCAL name matches, as its full `<tag …>body</tag>`
/// span body. Self-closing elements yield "" (the attrs are reachable via
/// [`extract_element_attrs`]). Sibling order preserved.
fn extract_elements<'a>(text: &'a str, local: &str) -> Vec<&'a str> {
    let mut bodies = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let Some((name_token, tag_end, self_closing, is_close)) = read_tag_name(text, i) else {
            i += 1;
            continue;
        };
        if is_close {
            // A stray close tag (or the outer close of a parent element the
            // caller handed us) — never an open; move past it.
            i = tag_end;
            continue;
        }
        if local_name_of(name_token) == local {
            if self_closing {
                bodies.push("");
                i = tag_end;
                continue;
            }
            // Find the matching close tag by local name.
            let mut j = tag_end;
            let mut found = None;
            while j < bytes.len() {
                if bytes[j] == b'<'
                    && let Some((cname, cend, _, is_close)) = read_tag_name(text, j)
                    && is_close
                    && local_name_of(cname) == local
                {
                    // The body ends at the close tag's '<'.
                    found = Some(j);
                    j = cend;
                    break;
                }
                j += 1;
            }
            if let Some(end) = found {
                bodies.push(&text[tag_end..end]);
                i = j;
                continue;
            }
        }
        i = tag_end.max(i + 1);
    }
    bodies
}

/// (attrs, body) for every element whose local name matches — used for
/// attribute-bearing elements (`<link href rel>`, `<category term>`).
fn extract_element_attrs<'a>(
    text: &'a str,
    local: &str,
) -> Vec<(std::collections::HashMap<String, String>, &'a str)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let Some((name_token, tag_end, self_closing, is_close)) = read_tag_name(text, i) else {
            i += 1;
            continue;
        };
        if is_close {
            i = tag_end;
            continue;
        }
        if local_name_of(name_token) == local {
            let open_tag = &text[i + 1..tag_end.saturating_sub(1)];
            let attrs = parse_attrs(open_tag);
            if self_closing {
                out.push((attrs, ""));
                i = tag_end;
                continue;
            }
            // Same close-scan as extract_elements.
            let mut j = tag_end;
            let mut found = None;
            while j < bytes.len() {
                if bytes[j] == b'<'
                    && let Some((cname, cend, _, close)) = read_tag_name(text, j)
                    && close
                    && local_name_of(cname) == local
                {
                    found = Some(j);
                    j = cend;
                    break;
                }
                j += 1;
            }
            if let Some(end) = found {
                out.push((attrs, &text[tag_end..end]));
                i = j;
                continue;
            }
        }
        i = tag_end.max(i + 1);
    }
    out
}

/// Read the tag starting at `text[pos] == '<'`: its name token (up to the
/// first whitespace / `>` / `/`), the index ONE PAST the tag's `>`, whether
/// it self-closes, and whether it is a CLOSE tag. A bare `<` in text (not a
/// tag) yields None.
fn read_tag_name(text: &str, pos: usize) -> Option<(&str, usize, bool, bool)> {
    let after = &text[pos + 1..];
    let (is_close, after) = after
        .strip_prefix('/')
        .map_or((false, after), |rest| (true, rest));
    let name_end = after
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(after.len());
    if name_end == 0 {
        return None; // "< " or "<>" — not a tag name
    }
    let name = &after[..name_end];
    // The tag's own '>' — attribute values with a raw '>' are outside the
    // recorded grammars (the scanner's honest scope).
    let gt_rel = name_end + after[name_end..].find('>')? + 1;
    let gt = pos + 1 + gt_rel;
    let self_closing = text[..gt].ends_with("/>");
    Some((name, gt, self_closing, is_close))
}

/// The local name of a tag token: everything after the LAST ':' (the
/// prefix-tolerant match the NIST shape needs).
fn local_name_of(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// The open tag's attribute map (name="value" | name='value'), decoded.
fn parse_attrs(open_tag: &str) -> std::collections::HashMap<String, String> {
    let mut attrs = std::collections::HashMap::new();
    let bytes = open_tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Skip to an attribute-name start.
        if !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b':' | b'_' | b'.'))
        {
            i += 1;
        }
        let name = &open_tag[name_start..i];
        // Whitespace, '=', whitespace, quote.
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            continue; // not a key=value attr; resume scanning after the name
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || (bytes[j] != b'"' && bytes[j] != b'\'') {
            i = j;
            continue;
        }
        let q = bytes[j] as char;
        let value_start = j + 1;
        let Some(close_rel) = open_tag[value_start..].find(q) else {
            break;
        };
        let value_end = value_start + close_rel;
        attrs.insert(
            name.to_string(),
            decode_entities(open_tag[value_start..value_end].trim()),
        );
        i = value_end + 1;
    }
    attrs
}

/// An element's decoded TEXT: CDATA passes through raw (the wrapper
/// stripped, no entity decode inside), otherwise ONE level of the five
/// XML built-ins (decoded HTML stays DATA). Element-bounded.
fn element_text(scope: &str, local: &str) -> Option<String> {
    let raw = extract_elements(scope, local).into_iter().next()?;
    Some(decode_element_text(raw))
}

/// CDATA passthrough or one-level entity decode.
fn decode_element_text(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(inner) = raw
        .strip_prefix("<![CDATA[")
        .and_then(|r| r.strip_suffix("]]>"))
    {
        return inner.to_string();
    }
    decode_entities(raw)
}

/// ONE level of the five XML built-ins. `&amp;` decodes LAST so a
/// doubly-escaped `&amp;lt;` stays the honest single-level `&lt;` (the
/// decoded HTML is DATA, evidence bytes — never re-scanned, never run).
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// RFC-3339 normalization for Atom dates (already RFC-3339; parsed and
/// re-rendered so the stored form is uniform). Unparseable = None — a
/// missing date is honest absent data, not a refusal.
fn date_rfc3339(raw: Option<String>) -> Option<String> {
    let s = raw?;
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|d| d.to_rfc3339())
}

/// RFC-822 → RFC-3339 for RSS pubDates (lobsters: `-0600` offsets).
fn date_rfc2822(raw: Option<String>) -> Option<String> {
    let s = raw?;
    chrono::DateTime::parse_from_rfc2822(s.trim())
        .ok()
        .map(|d| d.to_rfc3339())
}

// ── S2: the transport (replay/live, the SA-4 discipline inherited) ──────

/// The named HTTP budgets (inside the shared 30 s wall — a per-lane tick is
/// one fetch, and the ground's merge-order fallback stays unnecessary).
pub const WATCH_HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
pub const WATCH_HTTP_TOTAL_TIMEOUT_SECS: u64 = 25;

/// The tick's fetch transport: LIVE (a client pinned to the LANE's host by
/// construction — `redirect::Policy::none()` because a redirect is a pin
/// bypass, `https_only`, `no_proxy`, the named budgets) or REPLAY (the
/// frozen corpus — the SA-4 discipline: CI never performs a live fetch).
/// There is no process global: the transport is a per-tick VALUE, so a
/// test cannot accidentally bless the live arm and the live arm cannot
/// leak into CI.
#[derive(Debug, Clone)]
pub enum WatchTransport {
    Live {
        client: reqwest::Client,
        host: String,
    },
    Replay,
}

impl WatchTransport {
    /// The live client for ONE lane's host. The host was SSRF-screened at
    /// lane create; the client pins every subsequent fetch to it.
    pub fn live(lane_host: &str) -> Result<Self, WatchError> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(
                WATCH_HTTP_CONNECT_TIMEOUT_SECS,
            ))
            .timeout(std::time::Duration::from_secs(
                WATCH_HTTP_TOTAL_TIMEOUT_SECS,
            ))
            .user_agent(concat!(
                "brain-server/",
                env!("CARGO_PKG_VERSION"),
                " (research-watch; publishers-etiquette-honoring)"
            ))
            .build()
            .map_err(|_| WatchError::TransportFailed)?;
        Ok(WatchTransport::Live {
            client,
            host: lane_host.to_ascii_lowercase(),
        })
    }

    /// The replay arm: URL → frozen fixture bytes. A URL the corpus does
    /// not know is the named `watch_replay_gap`, never a silent live
    /// fallback (RW-6).
    pub fn replay() -> Self {
        WatchTransport::Replay
    }

    /// ONE GET, host-checked BEFORE any I/O (the pin bypass refusals — a
    /// foreign host or a non-https scheme never reach the client), then
    /// the CHUNKED bounded read: `Response::chunk()` accumulates under
    /// [`WATCH_MAX_FEED_BYTES`], the refusing chunk is never appended, and
    /// the client's own connect/total timeouts bound the stream (the
    /// Context7-verified shape on the locked reqwest).
    pub async fn get(&self, url: &str) -> Result<Vec<u8>, WatchError> {
        match self {
            WatchTransport::Replay => {
                let fixture = watch_replay_fixture_file(url).ok_or(WatchError::ReplayGap)?;
                std::fs::read(format!(
                    "{}/tests/fixtures/watch/{fixture}",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .map_err(|_| WatchError::ReplayGap)
            }
            WatchTransport::Live { client, host } => {
                // The pre-I/O pin checks: the URL is https on THE lane's
                // host, or the fetch refuses by name.
                let parsed = reqwest::Url::parse(url).map_err(|_| WatchError::LaneHostRequired)?;
                if parsed.scheme() != "https" {
                    return Err(WatchError::LaneHostRequired);
                }
                let url_host = parsed
                    .host_str()
                    .map(|h| h.to_ascii_lowercase())
                    .ok_or(WatchError::LaneHostRequired)?;
                if url_host != *host {
                    return Err(WatchError::FetchHostMismatch);
                }
                let mut resp = client
                    .get(url)
                    .send()
                    .await
                    .map_err(|_| WatchError::TransportFailed)?;
                if !resp.status().is_success() {
                    return Err(WatchError::TransportFailed);
                }
                let mut buf: Vec<u8> = Vec::new();
                while let Some(chunk) = resp
                    .chunk()
                    .await
                    .map_err(|_| WatchError::TransportFailed)?
                {
                    push_bounded(&mut buf, &chunk)?;
                }
                Ok(buf)
            }
        }
    }
}

/// The bounded accumulator: append `chunk` unless the running total would
/// cross [`WATCH_MAX_FEED_BYTES`] — then the named `watch_feed_too_large`
/// refusal, the offending chunk never appended (the pin drives THIS over a
/// synthetic oversized body; the transport loop is the thin caller).
pub fn push_bounded(buf: &mut Vec<u8>, chunk: &[u8]) -> Result<(), WatchError> {
    if buf.len() + chunk.len() > WATCH_MAX_FEED_BYTES {
        return Err(WatchError::FeedTooLarge);
    }
    buf.extend_from_slice(chunk);
    Ok(())
}

/// The replay map (URL → fixture). THE CORPUS IS THE MAP: the same bytes
/// the live probes returned, per the README provenance. CISA replays the
/// MARKED byte-prefix trim (5.7 MB does not belong in git; CI never
/// downloads it); the arXiv RSS lane replays the LIVE-RECORDED Sunday
/// state (the weekday-with-items shape is the marked-synthetic fixture,
/// parsed directly in tests — the URL map never serves synthetic bytes).
fn watch_replay_fixture_file(url: &str) -> Option<&'static str> {
    // NAMED APART from scholar.rs's `replay_fixture_for` (the dup-guard's
    // rename precedent): both are URL→fixture maps, but the contracts are
    // the different corpora — scholar maps RECORDED API ENVELOPES by
    // identifier slot; this maps WHOLE FEED TRANSCRIPTS by lane URL, into
    // tests/fixtures/watch/. A shared helper would force the two maps into
    // one abstraction they do not share.
    if url.starts_with("https://export.arxiv.org/api/query") {
        return Some("arxiv-atom.xml");
    }
    if url.starts_with("https://export.arxiv.org/rss/cs.CL") {
        return Some("arxiv-rss-sunday-empty.xml");
    }
    if url.starts_with("https://lobste.rs/rss") {
        return Some("lobsters.rss");
    }
    if url.starts_with("https://www.cisa.gov/cybersecurity-advisories/all.xml") {
        return Some("cisa-trim-prefix3.xml");
    }
    if url.starts_with("https://www.nist.gov/news-events/news/rss.xml") {
        return Some("nist.rss");
    }
    None
}

// ── S2: the arXiv param builder (the manual's escaping, by construction) ─

/// Build the arXiv Atom API query URL for a lane: the search query with the
/// manual's escaping (spaces as `+`, everything outside the unreserved set
/// percent-encoded), the optional `submittedDate:[from TO to]` incremental
/// window (YYYYMMDDhhmm forms), `sortBy=lastUpdatedDate`, and the slice cap
/// at the manual's 2000 (clamped — the slice size is kernel policy, never an
/// operator input).
pub fn build_arxiv_query_url(
    query: &str,
    window: Option<(&str, &str)>,
    max_results: u32,
) -> String {
    let mut search = encode_arxiv_value(query.trim());
    if let Some((from, to)) = window {
        search.push_str("+AND+submittedDate:%5B");
        search.push_str(from.trim());
        search.push_str("+TO+");
        search.push_str(to.trim());
        search.push_str("%5D");
    }
    let capped = max_results.min(2000);
    format!(
        "https://export.arxiv.org/api/query?search_query={search}&start=0&max_results={capped}&sortBy=lastUpdatedDate"
    )
}

/// The manual's escaping: the unreserved set plus a few query-syntax
/// punctuation pass through; space is `+`; everything else percent-encoded
/// (the scholar encoder's law, retargeted to arXiv's documented table).
fn encode_arxiv_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'('
            | b')'
            | b'/'
            | b':'
            | b'+'
            | b'*' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// ── S3: the tick orchestration (the surfaces' shared core) ──────────────
//
// THE TICK'S SHAPE (the ground §0.4.10): the whole DB side (the seen
// inserts + the proposals + the accounting row + the CHECKED audit row)
// rides ONE `BEGIN IMMEDIATE` transaction; the FETCH is excluded (the
// network never holds the write lock). The freshness floors are honored
// BEFORE the fetch — a skipped tick never issues a fetch and never holds
// the lock. RW-3: the audit row is the CHECKED in-tx write — a tick whose
// accounting INSERT fails FAILS, named; bare `audit::record` is best-effort
// by design and is NOT the tick's path (the mutation proof in the pins
// file's ledger shows exactly that failure: 25 proposals filed, audit_id 0).

/// One tick's honest report (the route's JSON; the CLI's stdout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickReport {
    pub lane: String,
    /// The freshness floor held: the fetch was skipped, the accounting row
    /// still written (the arXiv once-per-day law and the RSS `<ttl>` hint
    /// both land HERE — never as a second fetch).
    pub skipped_fresh: bool,
    /// The named refusal code when the tick refused at the feed level
    /// (transport failure, parser refusal) — the accounting row still
    /// wrote, `ok = 0`.
    pub refusal: Option<&'static str>,
    pub accounting: TickAccounting,
    /// The proposal ids the gate assigned (in feed order; empty when
    /// nothing filed).
    pub proposal_ids: Vec<i64>,
    /// The audit row id (the CHECKED write's RETURNING id — the evidence
    /// thread the operator pulls).
    pub audit_id: i64,
}

/// Everything a tick needs, supplied by the surface (the route hands a pool
/// connection + the server's model; the CLI hands its own). `conn` receives
/// the whole transaction.
pub struct TickContext<'a> {
    pub conn: &'a mut rusqlite::Connection,
    pub model: &'a dyn crate::embed::Embedder,
    pub transport: &'a WatchTransport,
    pub lane: WatchLane,
    pub actor: String,
    pub now_epoch: i64,
    pub fetched_at: String,
}

/// Run ONE lane tick: the freshness gate, the fetch + parse (outside the
/// tx), then ONE `BEGIN IMMEDIATE` carrying the seen inserts, the gate
/// filings (the ONE seam — [`crate::handlers::gate::create_proposal_core`]),
/// the accounting row, and the CHECKED audit row. A feed-level refusal
/// still writes its audited accounting row (`ok = 0`) and returns the
/// named refusal; a tick whose accounting INSERT fails FAILS (RW-3).
pub async fn run_tick(ctx: TickContext<'_>) -> Result<TickReport, WatchError> {
    let mut lane = ctx.lane.clone();
    if lane.destination != Destination::Proposals {
        // Unreachable today (the closed vocabulary has one value) — the
        // belt for a future vocabulary extension that forgets RW-1.
        return Err(WatchError::DestinationInvalid);
    }
    if !lane.enabled {
        return Err(WatchError::LaneDisabled);
    }

    // ── the freshness gate (before any fetch exists) ────────────────────
    let last_fetch_epoch: Option<i64> = ctx
        .conn
        .query_row(
            "SELECT ticked_at FROM watch_ticks WHERE lane_id = ?1 AND skipped_fresh = 0 \
             ORDER BY id DESC LIMIT 1",
            [lane.id],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(WatchError::from)?
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.timestamp());
    let floor = freshness_floor_secs(lane.source_kind, lane.cadence_secs, lane.ttl_secs);
    let stale = is_stale(floor, last_fetch_epoch, ctx.now_epoch);

    let outcome = if stale {
        // ── the fetch + parse (outside the tx — the write lock never
        // holds the network) ────────────────────────────────────────────
        match ctx.transport.get(&lane.feed_url).await {
            Ok(bytes) => match parse_feed(lane.source_kind, &bytes) {
                Ok(parsed) => {
                    // The publisher's ttl hint is LEARNED here (the lane
                    // row updates in the tx below): the NEXT freshness
                    // decision uses it.
                    if let Some(ttl) = parsed.ttl_secs {
                        lane.ttl_secs = Some(ttl);
                    }
                    let seen = load_seen_keys(ctx.conn, lane.id)?;
                    tick_core(&lane, &parsed.items, &seen, &ctx.fetched_at)
                }
                Err(e) => {
                    return refuse_tick(ctx, &lane, e);
                }
            },
            Err(e) => {
                return refuse_tick(ctx, &lane, e);
            }
        }
    } else {
        // The honest skip: the accounting line, the audited row, no fetch.
        TickOutcome {
            proposals: Vec::new(),
            seen_additions: Vec::new(),
            accounting: TickAccounting {
                skipped_fresh: 1,
                ..TickAccounting::default()
            },
        }
    };

    // ── the ONE transaction ──────────────────────────────────────────────
    // (BEGIN IMMEDIATE — the write-discipline law; the tick is a
    // read-modify-write of the seen-state plus the filings.)
    let mut tx = crate::workflow::tx::WorkflowTx::begin(ctx.conn).map_err(WatchError::from)?;
    let mut accounting = outcome.accounting;
    let mut proposal_ids: Vec<i64> = Vec::with_capacity(outcome.proposals.len());

    for addition in &outcome.seen_additions {
        tx.tx()
            .execute(
                "INSERT OR IGNORE INTO watch_seen(lane_id, item_key, near_key, seen_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    lane.id,
                    addition.item_key,
                    addition.near_key,
                    ctx.fetched_at
                ],
            )
            .map_err(WatchError::from)?;
    }

    for proposal in &outcome.proposals {
        // THE ONE SEAM: the exact validation/screen/scoring/insert path
        // every proposal rides — no private insert exists to bypass with
        // (the improve07 lesson, pinned). A screen REJECT on one poisoned
        // item fails THAT item (accounting.failed) and never the lane.
        let req = crate::handlers::gate::ProposalRequest {
            content: proposal.content.clone(),
            kind: "fact".to_string(),
            source: Some("watch".to_string()),
            origin_context: None,
            authority: None,
            observed_at: None,
            domain: None,
            title: Some(proposal.title.clone()),
            source_prompt: None,
        };
        if let Ok((resp, _verdict)) = crate::handlers::gate::create_proposal_core(
            tx.tx(),
            ctx.model,
            req,
            None,
            // the PATH-ASSIGNED stamp: the lane's tick files
            // per-item proposals — the lane id is the grouping reference.
            crate::handlers::gate::ProposalStamp::new(
                crate::handlers::gate::ProposalOrigin::Watch,
                format!("watch-lane:{}", lane.id),
            ),
        ) {
            proposal_ids.push(resp.id);
        } else {
            accounting.failed += 1;
        }
    }

    // The learned ttl rides the lane row (the NEXT freshness decision uses
    // the publisher's hint).
    if stale && let Some(ttl) = lane.ttl_secs {
        tx.tx()
            .execute(
                "UPDATE watch_configs SET ttl_secs = ?1 WHERE id = ?2",
                rusqlite::params![ttl as i64, lane.id],
            )
            .map_err(WatchError::from)?;
    }

    // The accounting row — the loop's health is measurable (plan §1.1).
    let detail = accounting.detail();
    tx.tx()
        .execute(
            "INSERT INTO watch_ticks(lane_id, ticked_at, items_seen, deduped, filed, failed, \
                                    skipped_fresh, ok, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                lane.id,
                ctx.fetched_at,
                accounting.items_seen as i64,
                accounting.deduped as i64,
                accounting.filed as i64,
                accounting.failed as i64,
                accounting.skipped_fresh as i64,
                1,
                detail,
            ],
        )
        .map_err(WatchError::from)?;

    // RW-3: the CHECKED in-tx audit write — if THIS insert fails the tick
    // FAILS (the `?`), the whole transaction rolls back, and the caller
    // sees the named error. Bare `audit::record` is best-effort by design;
    // the tick does not use it.
    let audit_id = crate::audit::record_tenant_checked(
        tx.tx(),
        crate::audit::AuditKind::Watch,
        &ctx.actor,
        &lane.name,
        crate::audit::AuditStatus::Ok,
        &detail,
        crate::audit::DEFAULT_TENANT,
    )
    .map_err(|_| WatchError::AuditWriteFailed)?;

    tx.commit().map_err(WatchError::from)?;

    Ok(TickReport {
        lane: lane.name,
        skipped_fresh: !stale,
        refusal: None,
        accounting,
        proposal_ids,
        audit_id,
    })
}

/// A feed-level refusal (transport/parser): the tick STILL writes its
/// audited accounting row (`ok = 0`, the named code in the detail — the
/// loop's health includes its failures) and returns the named error.
fn refuse_tick(
    ctx: TickContext<'_>,
    lane: &WatchLane,
    e: WatchError,
) -> Result<TickReport, WatchError> {
    let detail = format!(
        "refusal={} seen=0 deduped=0 filed=0 failed=0 skipped_fresh=0",
        e.code()
    );
    let mut tx = crate::workflow::tx::WorkflowTx::begin(ctx.conn).map_err(WatchError::from)?;
    tx.tx()
        .execute(
            "INSERT INTO watch_ticks(lane_id, ticked_at, items_seen, deduped, filed, failed, \
                                    skipped_fresh, ok, detail)
             VALUES (?1, ?2, 0, 0, 0, 0, 0, 0, ?3)",
            rusqlite::params![lane.id, ctx.fetched_at, detail],
        )
        .map_err(WatchError::from)?;
    let audit_id = crate::audit::record_tenant_checked(
        tx.tx(),
        crate::audit::AuditKind::Watch,
        &ctx.actor,
        &lane.name,
        crate::audit::AuditStatus::Error,
        &detail,
        crate::audit::DEFAULT_TENANT,
    )
    .map_err(|_| WatchError::AuditWriteFailed)?;
    tx.commit().map_err(WatchError::from)?;
    let _ = audit_id;
    Err(e)
}

/// The lane's seen-state, as the pure core expects (the hashed identities).
fn load_seen_keys(
    conn: &rusqlite::Connection,
    lane_id: i64,
) -> Result<HashSet<String>, WatchError> {
    let mut stmt = conn
        .prepare("SELECT item_key FROM watch_seen WHERE lane_id = ?1")
        .map_err(WatchError::from)?;
    let rows = stmt
        .query_map([lane_id], |r| r.get::<_, String>(0))
        .map_err(WatchError::from)?;
    let mut set = HashSet::new();
    for row in rows {
        set.insert(row.map_err(WatchError::from)?);
    }
    Ok(set)
}

// ── S4: the external synthesis leg (RW-2) ───────────────────────────────
//
// THE EXPLICIT PER-TASK ACT: the kernel NEVER invokes a provider on its
// own — no tick, no loop, no schedule names this path; the ONLY caller is
// POST /research/synthesize (the structural pin scans the production tree
// for it). The provider is the operator's configured external agent (a
// local GPT-Researcher, STORM, a hosted API) under the GDL posture: the
// profile is all-or-nothing fatal at boot (`validate_research_config`),
// the key rides the owner-only secret file, the endpoint is SSRF-screened
// and the client pinned per the provider_http construction law
// (`redirect::Policy::none()` — a redirect is a screen bypass —
// `no_proxy`, the named timeouts, the DNS pin to the validated set), the
// output is bounded, and THE OUTPUT LANDS AS A PROPOSAL, never memory:
// the route files through the gate's wrapper with `source: "synthesis"`
// and the deterministic header naming provider + model + task.

/// The synthesis response bound (the provider_http law's own ceiling —
/// a research report never needs more; a wall that size is hostile).
pub const WATCH_MAX_SYNTHESIS_BYTES: usize =
    crate::agentloop::provider_http::DEFAULT_MAX_RESPONSE_BYTES;

/// One synthesis outcome: the provider's text + the provenance the header
/// names (the endpoint HOST — never the full URL with any credential — and
/// the model id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesisOutcome {
    pub output: String,
    pub host: String,
    pub model: String,
}

/// Post ONE research task to the operator's provider endpoint. `base_url`
/// is the FULL endpoint (already validated by the boot profile check);
/// `api_key` is the OWNER-ONLY secret read (never logged, never echoed,
/// never in an error string). The output is the provider's text: a JSON
/// body's `report`/`content`/`text`/`output` string field when present
/// (the GPT-Researcher shape), else the raw body as text.
pub async fn synthesize_task(
    base_url: &str,
    api_key: &str,
    model: &str,
    task: &str,
) -> Result<SynthesisOutcome, WatchError> {
    let url = reqwest::Url::parse(base_url.trim()).map_err(|_| WatchError::TransportFailed)?;
    let host = url
        .host_str()
        .map(|h| h.to_ascii_lowercase())
        .ok_or(WatchError::TransportFailed)?
        .to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    // The SSRF screen — the ONE egress screen, `allow_private` governed by
    // the webhook's own loud opt-out (the local-endpoint posture: an
    // operator whose agent lives on the loopback opts in explicitly).
    let allow_private = std::env::var("BRAIN_EGRESS_ALLOW_PRIVATE").as_deref() == Ok("1");
    let addrs = crate::webhook::resolve_and_validate_sink(&host, port, allow_private)
        .map_err(|e| WatchError::LaneHostRefused(e.to_string()))?;
    let client = reqwest::Client::builder()
        // A redirect is a screen bypass: the validated ONE endpoint or
        // nothing (the provider_http law).
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .https_only(url.scheme() == "https")
        .connect_timeout(std::time::Duration::from_secs(
            WATCH_HTTP_CONNECT_TIMEOUT_SECS,
        ))
        .timeout(std::time::Duration::from_secs(
            WATCH_HTTP_TOTAL_TIMEOUT_SECS * 12,
        ))
        // The DNS pin: every connection goes to the validated set (a
        // later rebinding loses — the boot-frozen pin's per-request form).
        .resolve_to_addrs(&host, &addrs)
        .build()
        .map_err(|_| WatchError::TransportFailed)?;
    let resp = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::json!({ "task": task }).to_string())
        .send()
        .await
        .map_err(|_| WatchError::TransportFailed)?;
    if !resp.status().is_success() {
        return Err(WatchError::TransportFailed);
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|_| WatchError::TransportFailed)?;
    if bytes.len() > WATCH_MAX_SYNTHESIS_BYTES {
        return Err(WatchError::SynthesisOutputTooLarge);
    }
    let text = String::from_utf8(bytes.to_vec()).map_err(|_| WatchError::TransportFailed)?;
    let output = extract_report_text(&text);
    Ok(SynthesisOutcome {
        output,
        host,
        model: model.to_string(),
    })
}

/// The provider's report text: a JSON object's `report` | `content` |
/// `text` | `output` string field when one is present (the
/// GPT-Researcher/STORM shapes), else the raw body. Untrusted text either
/// way — it lands below the deterministic header as DATA and rides the
/// injection screen at filing like every capture.
fn extract_report_text(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body.trim())
        && let Some(obj) = v.as_object()
    {
        for key in ["report", "content", "text", "output"] {
            if let Some(s) = obj.get(key).and_then(|x| x.as_str()) {
                return s.to_string();
            }
        }
    }
    body.to_string()
}

/// The deterministic proposal header for a synthesis output — names the
/// provider (host), the model, and the TASK (bounded; the full task rides
/// the audit row as a hash only). The output lands BELOW the header as
/// quarantined DATA for the human reviewer.
pub fn render_synthesis_proposal(
    outcome: &SynthesisOutcome,
    task: &str,
    fetched_at: &str,
) -> (String, String) {
    let title = bound_chars(task.trim(), WATCH_TITLE_MAX_CHARS);
    let mut h = String::new();
    h.push_str("[Synthesis] provider host: ");
    h.push_str(&outcome.host);
    h.push('\n');
    h.push_str("Model: ");
    h.push_str(&outcome.model);
    h.push('\n');
    h.push_str("Task: ");
    h.push_str(&bound_chars(task.trim(), 500));
    h.push('\n');
    h.push_str(&format!("Fetched: {fetched_at}\n"));
    h.push_str("Provenance: operator-invoked external agent (POST /research/synthesize) — the kernel never calls a provider on its own\n");
    h.push('\n');
    h.push_str(&bound_chars(&outcome.output, WATCH_SUMMARY_MAX_CHARS));
    h.push('\n');
    (h, title)
}

/// The host of a URL, lowercased — the provenance header's provider line
/// (never the full URL: a credential must never ride a proposal header).
/// NAMED APART from brain-connector-crm.rs's `host_of` (the dup-guard's
/// rename precedent): both extract a lowercase host from a URL, but the
/// CRM connector's variant carries its own connector-specific fallback
/// contract; a shared helper would couple the watch's provenance header
/// to a connector binary.
pub fn watch_provider_host_of(url: &str) -> String {
    reqwest::Url::parse(url.trim())
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_ascii_lowercase()))
        .unwrap_or_else(|| "unparseable".to_string())
}

#[cfg(test)]
mod synthesis_tests {
    // The RW-2 filing law, pinned in-crate (create_proposal_core is
    // pub(crate) — the external pins file cannot see it): the provider's
    // output lands as a PROPOSAL (source `synthesis`, status pending)
    // through the REAL core; the header names provider + model + task; the
    // output is DATA below it; and the knowledge side holds NOTHING.
    use super::*;

    #[test]
    fn rw2_the_output_lands_as_a_proposal_never_memory() {
        crate::register_sqlite_vec::register_sqlite_vec();
        let mut conn = rusqlite::Connection::open_in_memory().expect("db");
        crate::migration::run_migration(&mut conn, crate::config::DB_MMAP_SIZE_MIB)
            .expect("migration");
        let outcome = SynthesisOutcome {
            output: "A briefing with sources.\n\n1. source one".to_string(),
            host: "127.0.0.1".to_string(),
            model: "gpt-researcher".to_string(),
        };
        let (content, title) = render_synthesis_proposal(
            &outcome,
            "brief the kernel on the watch posture",
            "2026-11-09T13:00:00Z",
        );
        assert!(content.contains("[Synthesis] provider host: 127.0.0.1"));
        assert!(content.contains("Model: gpt-researcher"));
        assert!(content.contains("Task: brief the kernel"));
        let model = crate::embed::StaticEmbedder::new(crate::config::MODEL_ID).expect("model");
        let (resp, verdict) = crate::handlers::gate::create_proposal_core(
            &conn,
            &model,
            crate::handlers::gate::ProposalRequest {
                content,
                kind: "fact".to_string(),
                source: Some("synthesis".to_string()),
                origin_context: None,
                authority: None,
                observed_at: None,
                domain: None,
                title: Some(title),
                source_prompt: None,
            },
            None,
            crate::handlers::gate::ProposalStamp::new(
                crate::handlers::gate::ProposalOrigin::Watch,
                "watch-synthesis",
            ),
        )
        .expect("the synthesis files");
        assert_eq!(verdict, crate::screen::ScreenResult::Clean);
        let (source, status): (String, String) = conn
            .query_row(
                "SELECT source, status FROM proposals WHERE id = ?1",
                [resp.id],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        r.get(1)?,
                    ))
                },
            )
            .expect("the proposal row");
        assert_eq!(source, "synthesis");
        assert_eq!(status, "pending", "the queue is the product — never memory");
        let knowledge: i64 = conn
            .query_row("SELECT COUNT(*) FROM knowledge", [], |r| r.get(0))
            .expect("knowledge count");
        assert_eq!(knowledge, 0, "the synthesis output never lands as memory");
    }

    /// The report extractor: the GPT-Researcher shape's field, else the
    /// raw body — untrusted text either way.
    #[test]
    fn rw2_the_report_extractor_takes_the_field_or_the_body() {
        assert_eq!(
            extract_report_text(r#"{"report":"the briefing","sources":[]}"#),
            "the briefing"
        );
        assert_eq!(
            extract_report_text(r#"{"content":"alt shape"}"#),
            "alt shape"
        );
        assert_eq!(
            extract_report_text("plain markdown body"),
            "plain markdown body"
        );
        assert_eq!(
            extract_report_text(r#"{"unrelated":1}"#),
            r#""{"unrelated":1}""#.trim_matches('"')
        );
    }
}
