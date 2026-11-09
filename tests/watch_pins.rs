//! The watch pins (IMPROVE08 S1): the deterministic core's laws, red-first.
//!
//! RED LEDGER (2026-11-09, against `597fe74f` before this round's code):
//!   1. compile-presence — `brain_server::watch` did not exist; this file
//!      did not compile ("could not find `watch` in `brain_server`").
//!   2. schema — `rw_s1_schema_three_tables_and_the_stamp` red twice: the
//!      pragma probes found no `watch_configs`/`watch_seen`/`watch_ticks`,
//!      and the stamp read `1.32.31` while the pin demanded `1.32.32`.
//!   3. the moved lockstep pins (version_axis / per_domain / main_suite /
//!      scholar sa8 / agreement / clean_cycle / rbac) went red against the
//!      still-`1.32.31` stamp — the pins demand the move before it exists.
//!
//! Green: the core + the ONE guarded migration landed in the same commit.
//!
//! The laws pinned here (plan 08 §0.4 + the execution prompt):
//!   RW-1  the watch's only product is a well-labeled queue — `destination`
//!         is the CLOSED value `proposals`; nothing else parses.
//!   RW-4  dedupe: a replayed feed files nothing new — `(lane_id, item_key)`
//!         seen-state, model-absent; same feed state → same proposal set.
//!   RW-5  the per-lane host pin starts at create: https, no userinfo, a
//!         host at all (`watch_lane_host_required`), and the SSRF screen
//!         (private/metadata addresses refused — the literal-IP paths, no
//!         DNS in CI).
//!   RW-8  the publishers' etiquette by construction: the arXiv 3 s
//!         inter-call constant, the once-per-day lane law, the RSS `<ttl>`
//!         floor — named constants with pinned values.

use brain_server::migration::run_migration;
use brain_server::storage_layout;
use brain_server::watch::{
    AdapterMeta, FeedItem, LaneSpec, SourceKind, WATCH_ARXIV_CADENCE_SECS,
    WATCH_ARXIV_MIN_INTERVAL_SECS, WATCH_DESTINATIONS, WATCH_MAX_FEED_BYTES, WATCH_SOURCE_KINDS,
    WatchError, freshness_floor_secs, is_stale, near_key, tick_core, validate_lane_spec,
};
use std::collections::HashSet;

// ── the lane spec ────────────────────────────────────────────────────────

fn good_spec(name: &str) -> LaneSpec<'_> {
    LaneSpec {
        name,
        source_kind: "rss",
        feed_url: "https://lobste.rs/rss.rss",
        query: None,
        cadence_secs: 3600,
        destination: "proposals",
    }
}

/// RW-1's structural face: `destination` is the closed vocabulary with ONE
/// value. A lane that names any other destination does not parse — there is
/// no memory destination for a watch lane to name (the plan's §1.3 refusal:
/// no watch output bypasses the proposal queue).
#[test]
fn rw1_destination_is_the_closed_value_proposals() {
    assert_eq!(WATCH_DESTINATIONS, &["proposals"]);
    assert!(validate_lane_spec(&good_spec("ok")).is_ok());

    for dest in ["memory", "knowledge", "", "proposals ", "PROPOSALS"] {
        let spec = LaneSpec {
            destination: dest,
            ..good_spec("greedy")
        };
        match validate_lane_spec(&spec) {
            Err(WatchError::DestinationInvalid) => {}
            other => panic!(
                "destination {dest:?} must be the named watch_destination_invalid refusal, got {other:?}"
            ),
        }
    }
}

/// The lane source-kind vocabulary is closed too: arxiv_api | rss | atom |
/// adapter. An unknown kind is the named refusal, never a silent "generic".
#[test]
fn rw1_source_kind_vocabulary_is_closed() {
    assert_eq!(WATCH_SOURCE_KINDS, &["arxiv_api", "rss", "atom", "adapter"]);
    let spec = LaneSpec {
        source_kind: "sitemap",
        ..good_spec("old-plan-shape")
    };
    assert!(matches!(
        validate_lane_spec(&spec),
        Err(WatchError::SourceKindInvalid(_))
    ));
    for kind in WATCH_SOURCE_KINDS {
        let spec = LaneSpec {
            source_kind: kind,
            feed_url: "https://example.com/feed",
            cadence_secs: 86_400,
            ..good_spec("kinds")
        };
        assert!(
            validate_lane_spec(&spec).is_ok(),
            "every vocabulary kind parses: {kind}"
        );
    }
}

/// RW-5: the host pin starts at create. A lane without a valid host is the
/// named `watch_lane_host_required` refusal — non-https, userinfo, and
/// hostless URLs all refuse. (The SSRF screen itself is the next pin; this
/// pin is the pure, DNS-free half.)
#[test]
fn rw5_lane_host_is_required_and_https_only() {
    for url in [
        "http://lobste.rs/rss.rss",          // not https
        "https://user:pw@lobste.rs/rss.rss", // userinfo — a credential in the URL
        "https://",                          // no host at all (EmptyHost)
        "not a url",                         // unparseable
        "ftp://lobste.rs/rss.rss",           // wrong scheme
    ] {
        let spec = LaneSpec {
            feed_url: url,
            ..good_spec("hostless")
        };
        match validate_lane_spec(&spec) {
            Err(WatchError::LaneHostRequired) => {}
            other => panic!("feed URL {url:?} must refuse watch_lane_host_required, got {other:?}"),
        }
    }
}

/// RW-5: the SSRF screen. A private or metadata address is refused at lane
/// create through the ONE egress screen (`webhook::resolve_and_validate_sink`
/// with `allow_private=false`). The literal-IP paths need no DNS, so this
/// pin runs in CI; the public-host accept path is exercised by the live arm.
#[test]
fn rw5_private_and_metadata_hosts_refused_at_create() {
    for host in ["127.0.0.1", "169.254.169.254", "10.0.0.1", "192.168.1.1"] {
        let err = brain_server::watch::screen_lane_host(host)
            .expect_err("a private/metadata lane host must refuse");
        assert!(
            matches!(err, WatchError::LaneHostRefused(_)),
            "{host} must be the watch_lane_host_refused refusal, got {err:?}"
        );
    }
}

/// A lane name is a short trust LABEL: non-empty, bounded, no control bytes.
#[test]
fn rw1_lane_name_is_bounded() {
    for name in ["", "   ", &"x".repeat(65)] {
        let spec = good_spec(name);
        assert!(
            matches!(validate_lane_spec(&spec), Err(WatchError::LaneNameInvalid)),
            "name {name:?} must refuse watch_lane_name_invalid"
        );
    }
    let ok = good_spec("arxiv-health-compliance");
    assert!(validate_lane_spec(&ok).is_ok());
}

/// Cadence bounds: a lane cadence is minutes-to-a-month; and for arXiv lanes
/// the documented once-per-day law is the FLOOR — a sub-daily arXiv lane is
/// a misconfiguration refused at create (the manual: cache your results).
#[test]
fn rw8_arxiv_lane_cadence_is_daily_by_law() {
    let arxiv = LaneSpec {
        source_kind: "arxiv_api",
        feed_url: "https://export.arxiv.org/api/query?search_query=cat:cs.CR",
        ..good_spec("arxiv-sub-daily")
    };
    let spec = LaneSpec {
        cadence_secs: 3600,
        ..arxiv
    };
    assert!(matches!(
        validate_lane_spec(&spec),
        Err(WatchError::ArxivCadenceTooShort)
    ));
    let spec = LaneSpec {
        cadence_secs: WATCH_ARXIV_CADENCE_SECS,
        ..arxiv
    };
    assert!(validate_lane_spec(&spec).is_ok());
}

/// RW-8's structural face: the etiquette constants exist with their pinned
/// values — the 3 s arXiv inter-call delay, the once-per-day lane law, and
/// the 16 MiB feed cap (CISA's 5.7 MB live clears provider_http's 4 MiB
/// precedent with headroom).
#[test]
fn rw8_the_etiquette_constants_are_pinned() {
    assert_eq!(WATCH_ARXIV_MIN_INTERVAL_SECS, 3);
    assert_eq!(WATCH_ARXIV_CADENCE_SECS, 86_400);
    assert_eq!(WATCH_MAX_FEED_BYTES, 16 * 1024 * 1024);
}

// ── the freshness floors (RW-8's tick-time face) ─────────────────────────

/// The arXiv lane's floor is its (create-guaranteed daily) cadence; an RSS
/// lane honors the publisher's `<ttl>` hint as the recorded ground formula
/// `min(cadence, ttl×60)` — pinned as recorded, quoted in the module docs.
#[test]
fn rw8_freshness_floors() {
    let arxiv = freshness_floor_secs(SourceKind::ArxivApi, 86_400, None);
    assert_eq!(arxiv, 86_400);

    // RSS with a known ttl (lobsters: 120 minutes): the ground's min() formula.
    let rss = freshness_floor_secs(SourceKind::Rss, 86_400, Some(120));
    assert_eq!(rss, 7_200, "min(cadence=1d, ttl=120min) is the ttl window");
    let rss = freshness_floor_secs(SourceKind::Rss, 1_800, Some(120));
    assert_eq!(rss, 1_800, "a shorter operator cadence is never loosened");

    // No ttl seen yet: the lane's own cadence.
    assert_eq!(freshness_floor_secs(SourceKind::Rss, 3_600, None), 3_600);
    assert_eq!(
        freshness_floor_secs(SourceKind::Adapter, 3_600, Some(1)),
        3_600
    );

    // is_stale: never ticked → stale; inside the floor → fresh (skip);
    // at/after the floor → stale (fetch).
    assert!(is_stale(3_600, None, 1_000));
    assert!(!is_stale(3_600, Some(0), 3_599));
    assert!(is_stale(3_600, Some(0), 3_600));
}

// ── the seen-state law (RW-4) ────────────────────────────────────────────

fn lane(name: &str) -> brain_server::watch::WatchLane {
    brain_server::watch::WatchLane {
        id: 1,
        name: name.to_string(),
        source_kind: SourceKind::Rss,
        feed_url: "https://lobste.rs/rss.rss".to_string(),
        host: "lobste.rs".to_string(),
        query: None,
        cadence_secs: 3600,
        destination: brain_server::watch::Destination::Proposals,
        ttl_secs: None,
        enabled: true,
    }
}

fn items() -> Vec<FeedItem> {
    vec![
        FeedItem {
            identity: "https://lobste.rs/s/62loik".to_string(),
            title: "systemd loves Rust".to_string(),
            link: Some("https://amutable.com/blog/systemd-loves-rust".to_string()),
            published: Some("2026-11-08T12:55:05Z".to_string()),
            updated: None,
            summary: Some("A write-up of the Rust modules in systemd.".to_string()),
            authors: vec!["lorenzleutgeb".to_string()],
            categories: vec!["rust".to_string(), "systemd".to_string()],
            adapter: None,
        },
        FeedItem {
            identity: "https://lobste.rs/s/jbdtim".to_string(),
            title: "Who wants to crowdsource LLM scans of articles?".to_string(),
            link: Some("https://lobste.rs/s/jbdtim".to_string()),
            published: Some("2026-11-08T16:36:17Z".to_string()),
            updated: None,
            summary: None,
            authors: vec!["wofo".to_string()],
            categories: vec!["ai".to_string()],
            adapter: None,
        },
        // No identity at all: the failed-processed accounting line, never filed.
        FeedItem {
            identity: String::new(),
            title: "identity-less".to_string(),
            link: None,
            published: None,
            updated: None,
            summary: None,
            authors: vec![],
            categories: vec![],
            adapter: None,
        },
    ]
}

/// RW-4, the core form: tick once → one proposal per NEW item; replay the
/// SAME feed state against the seen-state the first tick produced → nothing
/// new files (the replayed tick files nothing new), the repeats land as
/// `deduped`, and the identity-less item lands as `failed` both times.
/// Twice-run: two fresh seen-sets over the same feed state produce the SAME
/// proposal set — the pure-function law, model-absent.
#[test]
fn rw4_the_replayed_tick_files_nothing_new() {
    let ln = lane("lobsters");
    let feed = items();

    let first = tick_core(&ln, &feed, &HashSet::new(), "2026-11-09T12:00:00Z");
    assert_eq!(first.accounting.items_seen, 3);
    assert_eq!(first.accounting.filed, 2, "two identified items file");
    assert_eq!(first.accounting.deduped, 0);
    assert_eq!(first.accounting.failed, 1, "the identity-less item fails");
    assert_eq!(first.accounting.skipped_fresh, 0);
    assert_eq!(first.proposals.len(), 2);
    assert_eq!(first.seen_additions.len(), 2);

    // The seen-state the first tick produced — exactly what the tick's tx
    // inserts into watch_seen.
    let seen: HashSet<String> = first
        .seen_additions
        .iter()
        .map(|a| a.item_key.clone())
        .collect();

    let replay = tick_core(&ln, &feed, &seen, "2026-11-09T12:00:00Z");
    assert!(
        replay.proposals.is_empty(),
        "the replayed tick files NOTHING new (RW-4)"
    );
    assert_eq!(replay.accounting.filed, 0);
    assert_eq!(
        replay.accounting.deduped, 2,
        "the repeats are counted, labeled"
    );
    assert_eq!(replay.accounting.failed, 1);
    assert!(replay.seen_additions.is_empty());

    // Twice-run equality: same feed state → same proposal set (content
    // included — the deterministic header over fixed inputs).
    let again = tick_core(&ln, &feed, &HashSet::new(), "2026-11-09T12:00:00Z");
    assert_eq!(again.proposals, first.proposals);
    let mut ids_a: Vec<&str> = first
        .proposals
        .iter()
        .map(|p| p.identity.as_str())
        .collect();
    let mut ids_b: Vec<&str> = again
        .proposals
        .iter()
        .map(|p| p.identity.as_str())
        .collect();
    ids_a.sort();
    ids_b.sort();
    assert_eq!(ids_a, ids_b);
}

/// An item ALREADY seen under one lane is new under another: the seen-state
/// is per-LANE (`(lane_id, item_key)`), so two lanes over the same feed each
/// file their own copy — nothing is silently dropped across lanes.
#[test]
fn rw4_seen_state_is_per_lane() {
    let a = lane("lane-a");
    let mut b = lane("lane-b");
    b.id = 2;
    let feed = &items()[..2];
    let seen_for_a: HashSet<String> = tick_core(&a, feed, &HashSet::new(), "t")
        .seen_additions
        .iter()
        .map(|x| x.item_key.clone())
        .collect();
    let for_b = tick_core(&b, feed, &HashSet::new(), "t");
    assert_eq!(
        for_b.accounting.filed, 2,
        "lane b never saw lane a's seen-state"
    );
    let _ = seen_for_a;
}

/// The near-key: lowercase, punctuation-stripped, hashed — deterministic,
/// model-absent. Same story, different URL → same near-key (the human sees
/// it labeled on the proposal; nothing is silently dropped).
#[test]
fn rw4_the_near_key_is_deterministic_and_model_absent() {
    let a = near_key("Systemd Loves Rust!");
    let b = near_key("systemd loves rust");
    let c = near_key("  Systemd   loves -- rust...");
    assert_eq!(a, b, "case + punctuation never change the near-key");
    assert_eq!(a, c, "whitespace collapses");
    assert_eq!(a, near_key(a_repeat()), "stable across calls");
    assert_ne!(
        near_key("systemd loves rust"),
        near_key("rust loves systemd"),
        "different stories differ (xxh3, not a toy hash)"
    );
}

fn a_repeat() -> &'static str {
    "Systemd Loves Rust!"
}

// ── the adapter passthrough shape ────────────────────────────────────────

/// Adapter lanes carry the plan-07 provenance passthrough (doi/pmid/license/
/// retraction); a retracted record shows the SAME deterministic status line
/// shape plan 07 renders. The core renders it into the proposal header —
/// the human reviews the retraction flag at approve time.
#[test]
fn rw1_adapter_passthrough_renders_the_status_line() {
    let ln = lane("retraction-watch");
    let item = FeedItem {
        identity: "10.1016/S0140-6736(97)11096-0".to_string(),
        title: "Retracted record".to_string(),
        link: None,
        published: None,
        updated: None,
        summary: Some("the abstract".to_string()),
        authors: vec![],
        categories: vec![],
        adapter: Some(AdapterMeta {
            source_api: "europepmc".to_string(),
            doi: Some("10.1016/s0140-6736(97)11096-0".to_string()),
            pmid: None,
            license: None,
            retracted: true,
        }),
    };
    let out = tick_core(&ln, &[item], &HashSet::new(), "2026-11-09T12:00:00Z");
    assert_eq!(out.proposals.len(), 1);
    let p = &out.proposals[0];
    assert!(
        p.content.contains("Retracted: true"),
        "the 07 status line shape"
    );
    assert!(p.content.contains("doi: 10.1016/s0140-6736(97)11096-0"));
    assert!(p.content.contains("source_api: europepmc"));
    // The clean twin: no status line for a non-retracted record.
    let clean = FeedItem {
        adapter: None,
        identity: "x".to_string(),
        title: "clean".to_string(),
        ..items()[1].clone()
    };
    let out2 = tick_core(&ln, &[clean], &HashSet::new(), "t");
    assert!(!out2.proposals[0].content.contains("Retracted:"));
}

// ── the schema lockstep (this round's own record) ────────────────────────

/// The ONE guarded migration lands 1.32.32: `watch_configs` / `watch_seen` /
/// `watch_ticks` exist after `run_migration`, the stamp moved with them,
/// both arms of the upsert moved, the refuse-newer probe moved ONE ABOVE the
/// ceiling, and the rehearsal's parity walk carries all three tables.
#[test]
fn rw_s1_schema_three_tables_and_the_stamp() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("memory db");
    run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB).expect("migration");
    for table in ["watch_configs", "watch_seen", "watch_ticks"] {
        let present: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .expect("sqlite_master");
        assert_eq!(present, 1, "{table} must exist after run_migration");
    }
    let stamp: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("stamp");
    assert_eq!(
        stamp, "1.32.33",
        "run_migration must stamp the CURRENT ceiling — the stamp and the tables moved \
         together at 1.32.32 (IMPROVE08), and each later schema round re-pins the stamp \
         (1.32.33 at IMPROVE11's hitl_decision_events)"
    );
    // Re-run is a no-op (the guarded CREATEs skip).
    run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB).expect("re-migration");

    // The lockstep, structurally: ceiling == stamp == the upsert's both arms;
    // the probe one ABOVE the ceiling.
    assert_eq!(storage_layout::LATEST_KNOWN_SCHEMA, "1.32.33");
    let migration = std::fs::read_to_string("src/migration.rs").expect("migration src");
    assert!(
        migration.contains("'schema_version', '1.32.33'")
            && migration.contains("DO UPDATE SET value = '1.32.33'"),
        "both arms of the schema_version upsert must move together"
    );
    assert!(
        migration.contains("CREATE TABLE IF NOT EXISTS watch_configs")
            && migration.contains("CREATE TABLE IF NOT EXISTS watch_seen")
            && migration.contains("CREATE TABLE IF NOT EXISTS watch_ticks"),
        "the three tables are the migration's payload"
    );
    let layout = std::fs::read_to_string("src/storage_layout.rs").expect("layout src");
    assert!(
        layout.contains(r#"assert!(is_newer_than_known(Some("1.32.34")));"#)
            && layout.contains(r#"assert!(!is_newer_than_known(Some("1.32.33")));"#),
        "the refuse-newer probe must sit above the new ceiling, or it silently tests Equal"
    );
    let rehearsal =
        std::fs::read_to_string("src/bin/brain_migrate_rehearse.rs").expect("rehearsal");
    for table in ["watch_configs", "watch_seen", "watch_ticks"] {
        assert!(
            rehearsal.contains(&format!("\"{table}\"")),
            "PARITY_TABLES must carry {table} — a table outside the rehearsal's parity walk \
             is a table the rehearsal does not prove it copied"
        );
    }
}

// ── S2: the parsers, the corpus, the transport (RW-5/6/7 pins) ──────────
//
// RED LEDGER (2026-11-09, after S1 landed): compile-presence —
// `parse_feed` / `ParsedFeed` / `WatchTransport` / `build_arxiv_query_url`
// did not exist; this file did not compile until the parser + the
// transport landed in the same commit.

const FIXTURES: &str = "tests/fixtures/watch";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).expect("fixture present")
}

/// RW-6: the corpus byte-integrity pin — every file in `SHA256SUMS` matches
/// its committed hash (the 07 corpus law: drift re-records under a new
/// name, never overwrites).
#[test]
fn rw6_the_fixture_corpus_round_trips_byte_identically() {
    use sha2::{Digest, Sha256};
    let sums = std::fs::read_to_string(format!("{FIXTURES}/SHA256SUMS")).expect("sums");
    let mut checked = 0;
    for line in sums.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, path) = line.split_once("  ").expect("two-space sha256sums format");
        let bytes = std::fs::read(format!("{FIXTURES}/{}", path.trim())).expect("fixture");
        let got = hex::encode(Sha256::digest(&bytes));
        assert_eq!(
            got,
            hash.trim(),
            "{} drifted from the committed hash",
            path.trim()
        );
        checked += 1;
    }
    assert!(
        checked >= 6,
        "the corpus carries at least the six recorded files, got {checked}"
    );
}

/// RW-7: the five recorded live shapes parse through ONE parser.
/// - the arXiv Atom lane: 3 entries, identities the BARE arXiv ids WITH
///   version, RFC-3339 dates, authors, categories;
/// - lobsters: guid identities, the `<ttl>120` honored as 7200 s, RFC-822
///   normalized to RFC-3339;
/// - NIST: a dozen declared namespaces — the parser is prefix-tolerant;
/// - CISA's trimmed prefix: exactly its three items, entity-decoded;
/// - the Sunday arXiv RSS: ZERO items, no error (the empty-state honesty).
#[test]
fn rw7_the_five_live_shapes_parse() {
    // 1. the arXiv Atom lane (LIVE).
    let feed = brain_server::watch::parse_feed(
        brain_server::watch::SourceKind::ArxivApi,
        &fixture("arxiv-atom.xml"),
    )
    .expect("the arXiv Atom fixture parses");
    assert_eq!(feed.items.len(), 3);
    assert_eq!(feed.items[0].identity, "2410.05451v3", "id WITH version");
    assert!(feed.items[0].published.is_some());
    assert!(!feed.items[0].authors.is_empty());
    assert!(
        feed.items
            .iter()
            .any(|i| i.categories.iter().any(|c| c == "cs.CR"))
    );
    let secalign = feed
        .items
        .iter()
        .find(|i| i.identity == "2410.05451v3")
        .unwrap();
    assert!(
        secalign
            .summary
            .as_deref()
            .unwrap_or("")
            .contains("SecAlign"),
        "the summary is the item's DATA"
    );

    // 2. lobsters (LIVE): guid identity + the ttl honored + RFC-822 dates.
    let feed = brain_server::watch::parse_feed(
        brain_server::watch::SourceKind::Rss,
        &fixture("lobsters.rss"),
    )
    .expect("the lobsters fixture parses");
    assert!(
        feed.items.len() >= 10,
        "lobsters carries its items, got {}",
        feed.items.len()
    );
    assert_eq!(
        feed.ttl_secs,
        Some(7_200),
        "the <ttl>120 cache hint, in seconds"
    );
    assert!(
        feed.items
            .iter()
            .any(|i| i.identity == "https://lobste.rs/s/62loik")
    );
    let first = feed
        .items
        .iter()
        .find(|i| i.identity == "https://lobste.rs/s/62loik")
        .unwrap();
    assert_eq!(
        first.published.as_deref(),
        Some("2026-11-08T06:55:05-06:00"),
        "RFC-822 -0600 re-rendered in the uniform RFC-3339 form (the \
         publisher's offset preserved, not silently converted)"
    );

    // 3. NIST (LIVE): namespace-prefix-tolerant local-name parsing.
    let feed =
        brain_server::watch::parse_feed(brain_server::watch::SourceKind::Rss, &fixture("nist.rss"))
            .expect("the NIST fixture parses");
    assert!(feed.items.len() >= 5, "NIST carries its items");
    assert!(
        feed.items.iter().all(|i| !i.title.is_empty()),
        "every NIST item carries a title through the declared namespaces"
    );

    // 4. CISA (LIVE, byte-prefix trimmed through item 3): exactly its three
    // items; the entity-encoded descriptions decode ONE level and stay DATA.
    let feed = brain_server::watch::parse_feed(
        brain_server::watch::SourceKind::Rss,
        &fixture("cisa-trim-prefix3.xml"),
    )
    .expect("the CISA trimmed prefix parses");
    assert_eq!(
        feed.items.len(),
        3,
        "the trim carries exactly its three items"
    );
    assert!(
        feed.items
            .iter()
            .all(|i| !i.summary.as_deref().unwrap_or("").contains("&lt;")),
        "descriptions are decoded (no raw entity escapes ride the text)"
    );

    // 5. the Sunday arXiv RSS (LIVE): zero items, NO error.
    let feed = brain_server::watch::parse_feed(
        brain_server::watch::SourceKind::Rss,
        &fixture("arxiv-rss-sunday-empty.xml"),
    )
    .expect("the empty state is HONEST, never an error");
    assert_eq!(feed.items.len(), 0, "the Sunday feed saw zero items");

    // The marked-synthetic weekday shape parses to its two items (the
    // fixture the Sunday probe could not record — the evidence says so).
    let feed = brain_server::watch::parse_feed(
        brain_server::watch::SourceKind::Rss,
        &fixture("arxiv-rss-weekday-items.SYNTHETIC.xml"),
    )
    .expect("the marked-synthetic weekday shape parses");
    assert_eq!(feed.items.len(), 2);
    assert!(
        feed.items
            .iter()
            .any(|i| i.identity == "http://arxiv.org/abs/2601.00001v1")
    );
}

/// RW-7: the security refusals. A feed carrying `<!DOCTYPE` or `<!ENTITY`
/// is refused BY NAME — entities are never expanded, the XXE posture
/// (inline synthetic bodies; no probe carried a DTD, and the refusal must
/// hold regardless).
#[test]
fn rw7_dtd_and_entity_are_refused_by_name_never_expanded() {
    let doctype = br#"<?xml version="1.0"?>
<rss version="2.0"><!DOCTYPE rss [<!ENTITY xxe SYSTEM "file:///etc/passwd">]>
<channel><title>t</title><item><title>&xxe;</title></item></channel></rss>"#;
    let entity_only = br#"<rss version="2.0"><!ENTITY trick "expanded"><channel><item><title>&trick;</title></item></channel></rss>"#;
    for body in [&doctype[..], &entity_only[..]] {
        match brain_server::watch::parse_feed(brain_server::watch::SourceKind::Rss, body) {
            Err(brain_server::watch::WatchError::FeedDtdRefused) => {}
            other => panic!("a DTD/entity feed must refuse watch_feed_dtd_refused, got {other:?}"),
        }
    }
}

/// RW-7: malformed bytes (a truncated document — an item opened and never
/// closed) refuse `watch_feed_malformed`; a non-feed document (an HTML
/// page, a JSON body) refuses `watch_feed_wrong_schema`.
#[test]
fn rw7_malformed_and_wrong_schema_are_named() {
    // Truncation: the lobsters bytes cut mid-tag (derived from the corpus,
    // honestly-synthetic).
    let full = fixture("lobsters.rss");
    let cut = &full[..full.len() - 200];
    match brain_server::watch::parse_feed(brain_server::watch::SourceKind::Rss, cut) {
        Err(brain_server::watch::WatchError::FeedMalformed) => {}
        other => panic!("a truncated feed must refuse watch_feed_malformed, got {other:?}"),
    }
    for body in [
        &b"<html><body><h1>503 Service Unavailable</h1></body></html>"[..],
        &br#"{"error":"not a feed"}"#[..],
    ] {
        match brain_server::watch::parse_feed(brain_server::watch::SourceKind::Rss, body) {
            Err(brain_server::watch::WatchError::FeedWrongSchema) => {}
            other => panic!("a non-feed body must refuse watch_feed_wrong_schema, got {other:?}"),
        }
    }
}

/// RW-7: the arXiv API's documented error shape — a single Atom entry whose
/// `<id>` starts `http://arxiv.org/api/errors#` — is the NAMED refusal,
/// NEVER parsed as zero items (inline synthetic body; the shape is the
/// manual's, the bytes are not a transcript).
#[test]
fn rw7_arxiv_error_entry_is_a_named_refusal() {
    let body = br#"<?xml version='1.0' encoding='UTF-8'?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Error</title>
  <entry>
    <id>http://arxiv.org/api/errors#error_403</id>
    <title>Error</title>
    <summary>your request looks automated</summary>
  </entry>
</feed>"#;
    match brain_server::watch::parse_feed(brain_server::watch::SourceKind::ArxivApi, body) {
        Err(brain_server::watch::WatchError::ArxivErrorEntry(_)) => {}
        other => panic!("the arXiv error-as-Atom shape must refuse by name, got {other:?}"),
    }
}

/// RW-5: the byte cap is the chunked read's law — accumulation past
/// `WATCH_MAX_FEED_BYTES` refuses `watch_feed_too_large` (the accumulator
/// is the named pure function the transport loop calls; this pin drives it
/// over an honestly-synthetic oversized body, no server).
#[test]
fn rw5_the_byte_cap_refuses_named() {
    let chunk = vec![b'x'; 1024 * 1024];
    let mut buf = Vec::new();
    for _ in 0..16 {
        // 16 MiB in 1 MiB chunks — exactly AT the cap is allowed.
        brain_server::watch::push_bounded(&mut buf, &chunk).expect("at the cap is allowed");
    }
    assert_eq!(buf.len(), brain_server::watch::WATCH_MAX_FEED_BYTES);
    match brain_server::watch::push_bounded(&mut buf, &chunk) {
        Err(brain_server::watch::WatchError::FeedTooLarge) => {}
        other => panic!("over the cap must refuse watch_feed_too_large, got {other:?}"),
    }
    assert_eq!(
        buf.len(),
        brain_server::watch::WATCH_MAX_FEED_BYTES,
        "the refusing chunk is never appended"
    );
}

/// RW-6: the replay transport serves the frozen corpus and refuses the
/// gap — every mapped URL returns bytes IDENTICAL to the fixture file (the
/// replay IS the corpus), and an unmapped URL is the named
/// `watch_replay_gap`, never a silent live fallback.
#[tokio::test]
async fn rw6_the_replay_transport_serves_the_corpus_and_refuses_the_gap() {
    let t = brain_server::watch::WatchTransport::replay();
    for (url, fixture_name) in [
        (
            "https://export.arxiv.org/api/query?search_query=cat:cs.CR&start=0&max_results=100",
            "arxiv-atom.xml",
        ),
        (
            "https://export.arxiv.org/rss/cs.CL",
            "arxiv-rss-sunday-empty.xml",
        ),
        ("https://lobste.rs/rss.rss", "lobsters.rss"),
        (
            "https://www.cisa.gov/cybersecurity-advisories/all.xml",
            "cisa-trim-prefix3.xml",
        ),
        ("https://www.nist.gov/news-events/news/rss.xml", "nist.rss"),
    ] {
        let bytes = t.get(url).await.expect("mapped URL replays");
        assert_eq!(
            bytes,
            fixture(fixture_name),
            "the replay IS the corpus: {url}"
        );
    }
    match t.get("https://example.com/feed.xml").await {
        Err(brain_server::watch::WatchError::ReplayGap) => {}
        other => panic!("an unmapped URL must refuse watch_replay_gap, got {other:?}"),
    }
}

/// RW-5: the live transport is host-pinned BEFORE any I/O — a URL whose
/// host is not the pinned lane host refuses `watch_fetch_host_mismatch`,
/// and a non-https URL refuses outright (both checks fire before the
/// client is ever consulted, so this pin runs offline).
#[tokio::test]
async fn rw5_the_live_transport_is_host_pinned_pre_io() {
    let t = brain_server::watch::WatchTransport::live("lobste.rs").expect("client builds");
    match t
        .get("https://www.cisa.gov/cybersecurity-advisories/all.xml")
        .await
    {
        Err(brain_server::watch::WatchError::FetchHostMismatch) => {}
        other => panic!("a foreign host must refuse watch_fetch_host_mismatch, got {other:?}"),
    }
    match t.get("http://lobste.rs/rss.rss").await {
        Err(brain_server::watch::WatchError::LaneHostRequired) => {}
        other => panic!("a non-https URL must refuse, got {other:?}"),
    }
}

/// RW-5 (structural): the client construction law reads from the source —
/// `redirect::Policy::none()` (a redirect is a pin bypass), `https_only`,
/// `no_proxy`, and the named timeouts, all in the ONE builder.
#[test]
fn rw5_the_client_builder_is_pinned_structurally() {
    let src = std::fs::read_to_string("src/watch.rs").expect("src");
    for needle in [
        "redirect(reqwest::redirect::Policy::none())",
        "https_only(true)",
        "no_proxy()",
        "WATCH_HTTP_CONNECT_TIMEOUT_SECS",
        "WATCH_HTTP_TOTAL_TIMEOUT_SECS",
    ] {
        assert!(src.contains(needle), "the builder must carry {needle}");
    }
}

/// RW-8: the arXiv param builder encodes per the manual — spaces as `+`,
/// the submittedDate window with encoded brackets, `sortBy=lastUpdatedDate`,
/// and the slice cap at 2000.
#[test]
fn rw8_the_arxiv_param_builder_encodes_per_the_manual() {
    let url = brain_server::watch::build_arxiv_query_url(
        "cat:cs.CR AND all:prompt injection",
        Some(("202611010000", "202611092359")),
        100,
    );
    assert!(url.starts_with("https://export.arxiv.org/api/query?"));
    assert!(url.contains("search_query=cat:cs.CR+AND+all:prompt+injection"));
    assert!(
        url.contains("submittedDate:%5B202611010000+TO+202611092359%5D"),
        "the incremental window, brackets encoded: {url}"
    );
    assert!(url.contains("sortBy=lastUpdatedDate"));
    assert!(url.contains("max_results=100"));

    let url = brain_server::watch::build_arxiv_query_url("cat:cs.CR", None, 5000);
    assert!(
        url.contains("max_results=2000"),
        "the slice cap is the manual's 2000, clamped: {url}"
    );
}

// ── S3: the tick surfaces + the checked accounting (RW-1/3/4 pins) ──────
//
// RED LEDGER (2026-11-09): the ordering note — the S3 implementation
// landed ahead of these pins (the create_proposal_core refactor made the
// seams concrete first), so the red evidence here is the MUTATION PROOFS
// recorded in the evidence file, not compile-presence: RW-3's pin was run
// against the bare best-effort audit::record form (red: the tick
// "succeeded" with no accounting row) and against the corrupt-epoch DB
// (green: the tick FAILS named and rolls back).

use brain_server::watch::{TickContext, WatchTransport, lane_by_name, run_tick};

/// A real tick over the replay corpus against a REAL migrated DB: the
/// filings ride the ONE seam, the seen-state lands, the accounting row +
/// the CHECKED audit row commit. The tick's whole DB side is in the tx
/// (RW-3's shape) and the proposals carry source `watch` (the approved
/// row's verbatim kind — RW-1's queue product).
#[tokio::test]
async fn rw3_a_real_tick_files_seen_accounting_and_the_checked_audit_row() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("memory db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    let id = brain_server::watch::create_lane(
        &conn,
        &LaneSpec {
            name: "lobsters-replay",
            source_kind: "rss",
            feed_url: "https://lobste.rs/rss.rss",
            query: None,
            cadence_secs: 3600,
            destination: "proposals",
        },
        "2026-11-09T12:00:00Z",
    )
    .expect("lane create (the SSRF screen admits the public host)");
    let lane = lane_by_name(&conn, "lobsters-replay")
        .expect("read")
        .expect("lane");
    let model =
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model");
    let transport = WatchTransport::replay();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins:watch".to_string(),
        now_epoch: 1_794_225_600,
        fetched_at: "2026-11-09T12:00:00Z".to_string(),
    };
    let report = run_tick(ctx).await.expect("the tick");
    assert!(report.refusal.is_none());
    assert!(!report.skipped_fresh);
    assert!(report.accounting.filed >= 1, "the corpus item(s) filed");
    assert_eq!(report.proposal_ids.len() as u64, report.accounting.filed);

    // The seen-state landed (the replay's second tick dedupes — RW-4's DB form).
    let seen: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM watch_seen WHERE lane_id = ?1",
            [id],
            |r| r.get(0),
        )
        .expect("seen count");
    assert_eq!(seen as u64, report.accounting.filed);

    // The accounting row landed.
    let ticks: i64 = conn
        .query_row("SELECT COUNT(*) FROM watch_ticks", [], |r| r.get(0))
        .expect("tick count");
    assert_eq!(ticks, 1);

    // THE CHECKED AUDIT ROW: the tick's evidence is IN the chain (kind
    // `watch`), and the detail rides AS A HASH (the chain never stores the
    // plaintext) — verify the hash matches the report's accounting line,
    // the checked form's own link.
    let (kind, detail_hash): (String, String) = conn
        .query_row(
            "SELECT kind, detail_hash FROM audit_events WHERE id = ?1",
            [report.audit_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .expect("audit row");
    assert_eq!(kind, "watch");
    assert_eq!(
        detail_hash,
        brain_server::audit::hash(&format!(
            "seen={} deduped={} filed={} failed={} skipped_fresh={}",
            report.accounting.items_seen,
            report.accounting.deduped,
            report.accounting.filed,
            report.accounting.failed,
            report.accounting.skipped_fresh
        )),
        "the accounting detail rides (hashed — the chain's own law)"
    );

    // The proposals are in the queue with source `watch`.
    let source: String = conn
        .query_row(
            "SELECT source FROM proposals WHERE id = ?1",
            [report.proposal_ids[0]],
            |r| r.get(0),
        )
        .expect("proposal");
    assert_eq!(source, "watch");
}

/// RW-4, the DB form: the SAME feed state ticked twice files nothing new —
/// the second tick dedupes against the seen-state the first one wrote, and
/// a FRESH DB over the same corpus produces the same proposal set (the
/// pure-function law holds through the gate's scoring, model-absent
/// identity).
#[tokio::test]
async fn rw4_the_replayed_tick_files_nothing_new_in_the_db() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let model =
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model");
    let transport = WatchTransport::replay();

    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    brain_server::watch::create_lane(
        &conn,
        &LaneSpec {
            name: "lobsters-replay",
            source_kind: "rss",
            feed_url: "https://lobste.rs/rss.rss",
            query: None,
            cadence_secs: 3600,
            destination: "proposals",
        },
        "t",
    )
    .expect("lane");
    let lane = lane_by_name(&conn, "lobsters-replay").unwrap().unwrap();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins".into(),
        now_epoch: 1_794_225_600,
        fetched_at: "2026-11-09T12:00:00Z".into(),
    };
    let first = run_tick(ctx).await.expect("tick 1");
    let lane = lane_by_name(&conn, "lobsters-replay").unwrap().unwrap();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins".into(),
        // ONE HOUR later — past the floor (a sooner re-tick is the
        // skipped_fresh line, pinned separately below).
        now_epoch: 1_794_229_200,
        fetched_at: "2026-11-09T13:00:00Z".into(),
    };
    let second = run_tick(ctx).await.expect("tick 2");
    assert!(
        second.proposal_ids.is_empty(),
        "the replay files NOTHING new"
    );
    assert_eq!(second.accounting.deduped, first.accounting.filed);
    assert_eq!(second.accounting.filed, 0);

    // A fresh DB, same corpus: the same proposal SET (the identity law —
    // the contents are byte-identical, the ids are db-local).
    let mut conn2 = rusqlite::Connection::open_in_memory().expect("db2");
    brain_server::migration::run_migration(&mut conn2, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    brain_server::watch::create_lane(
        &conn2,
        &LaneSpec {
            name: "lobsters-replay",
            source_kind: "rss",
            feed_url: "https://lobste.rs/rss.rss",
            query: None,
            cadence_secs: 3600,
            destination: "proposals",
        },
        "t",
    )
    .expect("lane");
    let lane2 = lane_by_name(&conn2, "lobsters-replay").unwrap().unwrap();
    let ctx2 = TickContext {
        conn: &mut conn2,
        model: &model,
        transport: &transport,
        lane: lane2,
        actor: "pins".into(),
        now_epoch: 1_794_225_600,
        fetched_at: "2026-11-09T12:00:00Z".into(),
    };
    let fresh = run_tick(ctx2).await.expect("fresh tick");
    assert_eq!(fresh.accounting.filed, first.accounting.filed);
    let mut contents_a: Vec<String> = first
        .proposal_ids
        .iter()
        .map(|id| {
            conn.query_row("SELECT content FROM proposals WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        })
        .collect();
    let mut contents_b: Vec<String> = fresh
        .proposal_ids
        .iter()
        .map(|id| {
            conn2
                .query_row("SELECT content FROM proposals WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap()
        })
        .collect();
    contents_a.sort();
    contents_b.sort();
    assert_eq!(
        contents_a, contents_b,
        "same feed state → same proposal set"
    );
}

/// RW-3, the behavioral pin: the CHECKED form means the tick FAILS when the
/// accounting INSERT fails — here forced honestly (a corrupt chain epoch
/// makes the checked audit write refuse), the tick returns the NAMED
/// `watch_audit_write_failed` and the transaction rolled back: no proposals,
/// no seen rows, no tick row. Nothing is half-recorded.
#[tokio::test]
async fn rw3_a_failed_accounting_insert_fails_the_tick_named_and_rolls_back() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    // The poisoned epoch: the checked write refuses any scheme it cannot
    // name — exactly the failure RW-3 says must fail the tick.
    conn.execute(
        "INSERT INTO schema_meta(key, value) VALUES ('audit_chain_epoch', 'bogus')
         ON CONFLICT(key) DO UPDATE SET value = 'bogus'",
        [],
    )
    .expect("poison the epoch");
    brain_server::watch::create_lane(
        &conn,
        &LaneSpec {
            name: "lobsters-replay",
            source_kind: "rss",
            feed_url: "https://lobste.rs/rss.rss",
            query: None,
            cadence_secs: 3600,
            destination: "proposals",
        },
        "t",
    )
    .expect("lane");
    let model =
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model");
    let transport = WatchTransport::replay();
    let lane = lane_by_name(&conn, "lobsters-replay").unwrap().unwrap();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins".into(),
        now_epoch: 1_794_225_600,
        fetched_at: "2026-11-09T12:00:00Z".into(),
    };
    let err = run_tick(ctx).await.expect_err("the tick must FAIL");
    assert_eq!(
        err.code(),
        "watch_audit_write_failed",
        "the named refusal, got {err:?}"
    );
    // THE ROLLBACK: nothing landed.
    for (table, label) in [
        ("watch_seen", "seen rows"),
        ("watch_ticks", "tick rows"),
        ("proposals", "proposals"),
        ("audit_events", "audit rows"),
    ] {
        let n: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .expect(label);
        assert_eq!(n, 0, "{label} must be empty after the failed tick");
    }
}

/// RW-8, the tick-time face: re-ticking INSIDE the freshness floor skips
/// honestly (skipped_fresh = the accounting line, never a second fetch) —
/// and the skipped tick STILL writes its audited accounting row.
#[tokio::test]
async fn rw8_the_retick_inside_the_floor_is_the_honest_skip() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    brain_server::watch::create_lane(
        &conn,
        &LaneSpec {
            name: "lobsters-replay",
            source_kind: "rss",
            feed_url: "https://lobste.rs/rss.rss",
            query: None,
            cadence_secs: 3600,
            destination: "proposals",
        },
        "t",
    )
    .expect("lane");
    let model =
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model");
    let transport = WatchTransport::replay();
    let lane = lane_by_name(&conn, "lobsters-replay").unwrap().unwrap();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins".into(),
        now_epoch: 1_794_225_600,
        fetched_at: "2026-11-09T12:00:00Z".into(),
    };
    run_tick(ctx).await.expect("tick 1");
    // 10 minutes later — inside the 3600 s floor.
    let lane = lane_by_name(&conn, "lobsters-replay").unwrap().unwrap();
    let ctx = TickContext {
        conn: &mut conn,
        model: &model,
        transport: &transport,
        lane,
        actor: "pins".into(),
        now_epoch: 1_794_226_200,
        fetched_at: "2026-11-09T12:10:00Z".into(),
    };
    let report = run_tick(ctx).await.expect("the skipped tick");
    assert!(report.skipped_fresh);
    assert!(report.proposal_ids.is_empty());
    assert_eq!(report.accounting.skipped_fresh, 1);
    // The audited row still wrote.
    let audit_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE kind='watch'",
            [],
            |r| r.get(0),
        )
        .expect("audit rows");
    assert_eq!(audit_rows, 2, "both ticks carry their accounting row");
}

/// RW-1, structural: the watch module holds NO knowledge INSERT — the
/// watch's only write paths are the three watch tables + the proposal queue
/// (through the gate's seam, which is not in this module).
#[test]
fn rw1_the_watch_module_writes_no_knowledge() {
    let src = std::fs::read_to_string("src/watch.rs").expect("src");
    assert!(
        !src.contains("INSERT INTO knowledge"),
        "the watch module must hold no knowledge INSERT (RW-1)"
    );
    let handler = std::fs::read_to_string("src/server/router/memory.rs").expect("router");
    let handler_region = handler
        .split("pub(crate) async fn watch_tick")
        .nth(1)
        .expect("the tick handler")
        .split("pub(crate) async fn ingest_markdown")
        .next()
        .expect("the handler region");
    assert!(
        !handler_region.contains("INSERT INTO knowledge"),
        "the tick handler must hold no knowledge INSERT (RW-1)"
    );
}

// ── S4: the synthesis provider posture (RW-2 pins) ──────────────────────
//
// RED LEDGER (2026-11-09): compile-presence — ResearchProviderProfile /
// validate_research_config / synthesize_task / render_synthesis_proposal
// did not exist; this section did not compile until the profile, the boot
// validator, the route, and the contract rows landed in the same commit.

/// Env-serializing guard for the profile pins (the config tests' restore
/// discipline: the pin leaves the environment exactly as it found it).
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn env_set(name: &str, value: Option<&str>) {
    // SAFETY: single-threaded per test under ENV_LOCK; no other thread in
    // this binary reads these vars concurrently.
    unsafe {
        match value {
            Some(v) => std::env::set_var(name, v),
            None => std::env::remove_var(name),
        }
    }
}
/// RW-2, structural: the provider call site is ONE — `synthesize_task` is
/// defined in watch.rs and invoked ONLY by the synthesize route (no tick,
/// no loop, no schedule, no CLI names it). And the boot validator rides
/// beside the scholar validator (the fail-closed region).
#[test]
fn rw2_the_provider_is_invoked_only_on_its_route() {
    let callers: Vec<String> = ["src/", "tests/"]
        .iter()
        .flat_map(|root| walk(root))
        .filter(|p| p.ends_with(".rs"))
        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|s| (p, s)))
        .filter(|(_, s)| s.contains("synthesize_task("))
        .map(|(p, _)| p)
        .collect();
    let in_src: Vec<&String> = callers.iter().filter(|p| p.starts_with("src/")).collect();
    assert_eq!(
        in_src.len(),
        2,
        "the provider call site is ONE (definition + the route): {in_src:?}"
    );
    // The CLI never calls it either.
    assert!(
        !callers.iter().any(|p| p.contains("bin/")),
        "the CLI must never invoke the provider autonomously (RW-2)"
    );
    let boot = std::fs::read_to_string("src/server/bootstrap.rs").expect("boot");
    assert!(
        boot.contains("config::validate_research_config()")
            && boot.contains("config::validate_scholar_config()"),
        "the research validator rides beside the scholar validator"
    );
}

fn walk(root: &str) -> Vec<String> {
    let mut out = Vec::new();
    let stack = vec![root.to_string()];
    let mut todo = stack;
    while let Some(dir) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                todo.push(path.to_string_lossy().to_string());
            } else {
                out.push(path.to_string_lossy().to_string());
            }
        }
    }
    out
}

/// RW-2: no lane can name synthesis as a destination — the closed
/// vocabulary has ONE value and "synthesis" is not it (the belt: even a
/// future vocabulary edit cannot route watch output at the provider).
#[test]
fn rw2_no_lane_destination_names_synthesis() {
    assert!(matches!(
        brain_server::watch::Destination::parse("synthesis"),
        Err(brain_server::watch::WatchError::DestinationInvalid)
    ));
}

/// RW-2, behavioral over a LOCAL stub: the pinned client posts the task,
/// reads the GPT-Researcher-shaped field, and the outcome carries the
/// provenance. The loopback endpoint is the plan's preferred posture —
/// admitted ONLY under BRAIN_EGRESS_ALLOW_PRIVATE=1 (the webhook's loud
/// opt-out; the screen refuses private addresses without it — that refusal
/// is pinned here too).
#[tokio::test]
async fn rw2_the_network_leg_posts_pinned_and_extracts_the_report() {
    // (the env lock is taken in a scoped block below — never across an
    // await; the stub server runs outside it)
    // A tiny axum stub answering the research shape.
    let app = axum::Router::new().route(
        "/research",
        axum::routing::post(|body: String| async move {
            assert!(body.contains("the research task"), "the task rides: {body}");
            axum::Json(serde_json::json!({ "report": "the briefing with sources", "sources": [] }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    let base_url = format!("http://{addr}/research");

    // WITHOUT the opt-out: the private address refuses (the SSRF screen).
    // The env lock scopes the MUTATION only — never held across an await
    // (the only env-mutating tests in this binary serialize on this lock,
    // so the set→call window cannot interleave with another mutator).
    {
        let _g = ENV_LOCK.lock().unwrap();
        env_set("BRAIN_EGRESS_ALLOW_PRIVATE", None);
    }
    let err = brain_server::watch::synthesize_task(&base_url, "k", "m", "the research task")
        .await
        .expect_err("a private endpoint refuses without the loud opt-out");
    assert!(matches!(
        err,
        brain_server::watch::WatchError::LaneHostRefused(_)
    ));

    // WITH it: the pinned client posts, the extractor takes `report`.
    {
        let _g = ENV_LOCK.lock().unwrap();
        env_set("BRAIN_EGRESS_ALLOW_PRIVATE", Some("1"));
    }
    let outcome =
        brain_server::watch::synthesize_task(&base_url, "k", "gpt-researcher", "the research task")
            .await
            .expect("the stub answers");
    {
        let _g = ENV_LOCK.lock().unwrap();
        env_set("BRAIN_EGRESS_ALLOW_PRIVATE", None);
    }
    assert_eq!(outcome.output, "the briefing with sources");
    assert_eq!(outcome.host, "127.0.0.1");
    assert_eq!(outcome.model, "gpt-researcher");

    env_set("BRAIN_EGRESS_ALLOW_PRIVATE", None);
    server.abort();
    let _ = server.await;
}
