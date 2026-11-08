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
        SourceKind::Rss | SourceKind::Atom => match ttl_secs {
            Some(ttl) => cadence_secs.min(ttl.saturating_mul(60)),
            None => cadence_secs,
        },
        SourceKind::Adapter => cadence_secs,
    }
}

/// Stale = the lane MAY fetch: never ticked, or the last fetch is at/after
/// the floor ago. `false` = the tick honors the floor as the honest
/// `skipped_fresh` accounting line.
pub fn is_stale(floor_secs: u64, last_fetch_epoch: Option<i64>, now_epoch: i64) -> bool {
    match last_fetch_epoch {
        None => true,
        Some(last) => now_epoch.saturating_sub(last) >= floor_secs as i64,
    }
}
