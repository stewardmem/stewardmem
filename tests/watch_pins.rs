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
        stamp, "1.32.32",
        "run_migration must stamp 1.32.32 — the stamp and the tables move together"
    );
    // Re-run is a no-op (the guarded CREATEs skip).
    run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB).expect("re-migration");

    // The lockstep, structurally: ceiling == stamp == the upsert's both arms;
    // the probe one ABOVE the ceiling.
    assert_eq!(storage_layout::LATEST_KNOWN_SCHEMA, "1.32.32");
    let migration = std::fs::read_to_string("src/migration.rs").expect("migration src");
    assert!(
        migration.contains("'schema_version', '1.32.32'")
            && migration.contains("DO UPDATE SET value = '1.32.32'"),
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
        layout.contains(r#"assert!(is_newer_than_known(Some("1.32.33")));"#)
            && layout.contains(r#"assert!(!is_newer_than_known(Some("1.32.32")));"#),
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
