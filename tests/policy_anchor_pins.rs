//! The policy-anchor registry's pins (GOVPACK-1, K-31) — red-first, each
//! with its drill RUN and REVERTED, the observed failure text recorded in
//! `docs/GOVPACK1_EVIDENCE_2026-11-23.md`.
//!
//! The laws under pin:
//! * supersede stamps history — an unstamped old row reads as in force
//!   forever (pin 1, with the anti-vacuity arm: the fixture asserts the DB
//!   actually holds BOTH version rows before asserting the verdicts, so a
//!   gate that passed over an empty registry fails loudly, never smiles);
//! * the manifest's hash law is the boundary — a drifted held text refuses
//!   the whole seed naming path + expected + actual (pin 2);
//! * the seed never edits history in place — identical content skips, any
//!   drift refuses (pin 3);
//! * the pack gate names EVERY blocker with its distinct reason, and the
//!   fetch-owed reason is the verbatim advisory (pin 4);
//! * the supersede is one transaction or nothing (pin 5);
//! * the CLI is wiring only — no SQL, no sha2 (pin 6);
//! * an unset knob is the no-bindings refusal — fail-closed on the knob
//!   too (pin 7, driven through the REAL binary).
//!
//! Hermeticity: the lib-level pins drive in-memory DBs and tempfile
//! fixtures; the binary-level pins spawn the real `brain` against a scratch
//! DB with `BRAIN_DB_PATH` pinned to that scratch (the leash is the
//! ENVIRONMENT — a subprocess that resolves the default DB path would open
//! the operator's live store). The real manifest fixture's hash arm skips
//! honestly on a tree whose sibling corpus checkout is absent (the
//! OCR-probe precedent) and bites live where the corpus is present.

use std::path::{Path, PathBuf};
use std::process::Command;

use brain_server::service::policy_anchor as pa;
use rusqlite::Connection;

use brain_server::migration::run_migration;
use brain_server::register_sqlite_vec::register_sqlite_vec;

fn db() -> Connection {
    register_sqlite_vec();
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    run_migration(&mut conn, 0).expect("migration");
    conn
}

fn planned_held(anchor_id: &str, sha: &str, corpus_path: &str) -> pa::PlannedAnchor {
    pa::PlannedAnchor {
        anchor_id: anchor_id.to_string(),
        issuer: "Test Issuer".to_string(),
        instrument_no: "Test Instrument No. 1".to_string(),
        series: "2026".to_string(),
        title: format!("Test instrument {anchor_id}"),
        scope_tags_json: "[\"test\"]".to_string(),
        sha256_hex: Some(sha.to_string()),
        state: pa::AnchorLifecycle::VerifiedTextHeld,
        corpus_path: Some(corpus_path.to_string()),
        official_url: None,
        notes: None,
        in_force_from: "2026-01-01".to_string(),
    }
}

fn planned_owed(anchor_id: &str, in_force_from: &str) -> pa::PlannedAnchor {
    pa::PlannedAnchor {
        anchor_id: anchor_id.to_string(),
        issuer: "Test Issuer".to_string(),
        instrument_no: "Test Instrument No. 2".to_string(),
        series: "2026".to_string(),
        title: format!("Test instrument {anchor_id}"),
        scope_tags_json: "[\"test\"]".to_string(),
        sha256_hex: None,
        state: pa::AnchorLifecycle::TextFetchOwed,
        corpus_path: None,
        official_url: None,
        notes: None,
        in_force_from: in_force_from.to_string(),
    }
}

fn plan_of(rows: Vec<pa::PlannedAnchor>) -> pa::SeedPlan {
    pa::SeedPlan {
        manifest_sha256: "0".repeat(64),
        rows,
        refusals: vec![],
    }
}

/// A minimal manifest + one held corpus file, built in a temp dir. The
/// `TempDir` guard travels WITH the struct — a dropped guard deletes the
/// corpus out from under every later read (measured: the first draft
/// dropped it in the builder and both corpus-touching pins red on
/// NotFound). Returns (root, manifest_path, held_digest).
struct MiniCorpus {
    dir: PathBuf,
    manifest_path: PathBuf,
    held_digest: String,
    _guard: tempfile::TempDir,
}

fn mini_corpus() -> MiniCorpus {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().expect("tempdir");
    let held_bytes = b"the legally operative text of the test instrument\n";
    let held_digest = {
        let d = Sha256::digest(held_bytes);
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let corpus_file = dir.path().join("01-test/test-instrument.pdf");
    std::fs::create_dir_all(corpus_file.parent().expect("parent")).expect("mkdir");
    std::fs::write(&corpus_file, held_bytes).expect("write corpus");

    let manifest = format!(
        r#"{{
      "manifest_version": 1,
      "anchors": [
        {{
          "id": "PH-TEST-HELD", "issuer": "Test Issuer",
          "instrument_no": "Test Instrument No. 1", "series": "2026",
          "title": "The held test instrument",
          "scope_tags": ["test"],
          "effective": {{"signed": "2026-01-01"}},
          "state": "verified_text_held",
          "sha256_hex": "{held_digest}",
          "corpus_path": "01-test/test-instrument.pdf"
        }},
        {{
          "id": "PH-TEST-OWED", "issuer": "Test Issuer",
          "instrument_no": "Test Instrument No. 2", "series": "2026",
          "title": "The owed test instrument",
          "scope_tags": ["test"],
          "effective": {{"issued": "2026-02"}},
          "state": "text_fetch_owed",
          "sha256_hex": null,
          "corpus_path": null
        }}
      ]
    }}"#
    );
    let manifest_path = dir.path().join("manifest.json");
    std::fs::write(&manifest_path, manifest).expect("write manifest");
    MiniCorpus {
        dir: dir.path().to_path_buf(),
        manifest_path,
        held_digest,
        _guard: dir,
    }
}

// ── pin 1 (+ the anti-vacuity arm) ──────────────────────────────────────

/// A superseded anchor is never in force as of a quarter boundary: the
/// supersede stamps the old row's `in_force_until`, so the as-of answer is
/// v2 between the stamps and v1 before the boundary. THE ANTI-VACUITY ARM:
/// the fixture first asserts the DB actually holds BOTH version rows under
/// the one anchor_id — version history shares the anchor_id by law (the
/// partial unique index holds exactly one LIVE row), and a gate that
/// passed over an empty registry must fail loudly, not smile.
#[test]
fn a_superseded_anchor_is_never_in_force_as_of_a_quarter_boundary() {
    let conn = db();
    let v1 = planned_owed("PH-QTR", "2026-01-01");
    let r1 = pa::seed(&conn, &plan_of(vec![v1])).expect("seed v1");
    assert_eq!(r1.inserted, 1);
    let old_id = r1
        .audit_row_id
        .checked_sub(0)
        .map(|_| {
            conn.query_row(
                "SELECT id FROM policy_anchor WHERE anchor_id = 'PH-QTR'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .expect("v1 row")
        })
        .expect("v1 id");

    let new = pa::NewVersion {
        anchor_id: "PH-QTR".to_string(),
        issuer: "Test Issuer".to_string(),
        instrument_no: "Test Instrument No. 2".to_string(),
        series: "2026".to_string(),
        title: "The test instrument, version two".to_string(),
        scope_tags_json: "[\"test\"]".to_string(),
        sha256_hex: None,
        state: pa::AnchorLifecycle::TextFetchOwed,
        corpus_path: None,
        official_url: None,
        notes: None,
        in_force_from: "2026-04-01".to_string(),
    };
    pa::supersede(&conn, old_id, &new).expect("supersede");

    // THE ANTI-VACUITY ARM, before any verdict is asserted.
    let versions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM policy_anchor WHERE anchor_id = 'PH-QTR'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(
        versions, 2,
        "the fixture must hold BOTH version rows (history shares the anchor_id) before \
         the as-of verdicts mean anything"
    );

    // Between the stamps: v2 only.
    let at_boundary = pa::state_as_of(&conn, "2026-06-30").expect("as-of between");
    assert_eq!(
        at_boundary
            .iter()
            .map(|s| s.anchor_id.as_str())
            .collect::<Vec<_>>(),
        vec!["PH-QTR"],
    );
    let v2 = &at_boundary[0];
    assert_eq!(v2.title, "The test instrument, version two");
    assert_eq!(v2.in_force_from, "2026-04-01");

    // Before the boundary: v1 (the old row still in force).
    let before = pa::state_as_of(&conn, "2026-02-01").expect("as-of before");
    assert_eq!(
        before
            .iter()
            .map(|s| s.anchor_id.as_str())
            .collect::<Vec<_>>(),
        vec!["PH-QTR"],
    );
    assert_eq!(before[0].title, "Test instrument PH-QTR", "v1's own title");

    // At the boundary instant itself: the cutover is clean — the old row's
    // until > T is false exactly when the new row's from <= T becomes true.
    let at_cut = pa::state_as_of(&conn, "2026-04-01").expect("as-of at cut");
    assert_eq!(at_cut[0].title, "The test instrument, version two");
}

// ── pin 2 ───────────────────────────────────────────────────────────────

/// An anchor whose text hash drifts refuses to seed, BY NAME: the refusal
/// names the path, the expected digest, and the actual digest — never a
/// bare failure. THE DRILL (run and reverted, the red text in the evidence
/// file): skip the hash comparison in `verify_manifest` and the pin reds
/// with `got Ok(SeedPlan)` — the drift sails in.
#[test]
fn an_anchor_whose_text_hash_drifts_refuses_to_seed_by_name() {
    let corpus = mini_corpus();
    // Tamper the corpus copy.
    let held = corpus.dir.join("01-test/test-instrument.pdf");
    let mut bytes = std::fs::read(&held).expect("read held");
    bytes.extend(b"TAMPERED");
    std::fs::write(&held, &bytes).expect("tamper");

    let json = std::fs::read_to_string(&corpus.manifest_path).expect("read manifest");
    let err = pa::verify_manifest(&json, Some(&corpus.dir))
        .expect_err("a drifted held text must refuse the whole manifest");
    let text = err.to_string();
    assert!(
        text.contains("text-hash drift"),
        "the refusal names the drift class: {text}"
    );
    assert!(
        text.contains("test-instrument.pdf"),
        "the refusal names the PATH: {text}"
    );
    assert!(
        text.contains(&corpus.held_digest),
        "the refusal names the EXPECTED digest: {text}"
    );
    let actual = {
        use sha2::{Digest, Sha256};
        let d = Sha256::digest(&bytes);
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    assert!(
        text.contains(&actual),
        "the refusal names the ACTUAL digest: {text}"
    );
}

// ── pin 3 ───────────────────────────────────────────────────────────────

/// A seed is idempotent and never edits history in place: the second run
/// over an identical manifest skips every row and writes nothing but its
/// own act's audit row; the same anchor_id with ANY content drift refuses
/// by name (supersede is the only lawful change). THE DRILL (run and
/// reverted): make the second seed UPDATE the row in place and the pin
/// reds on the row-identity assertion.
#[test]
fn a_seed_is_idempotent_and_never_edits_history_in_place() {
    let conn = db();
    let rows = vec![
        planned_held(
            "PH-IDEM",
            "1111111111111111111111111111111111111111111111111111111111111111",
            "x.pdf",
        ),
        planned_owed("PH-IDEM-2", "2026-03-03"),
    ];
    let first = pa::seed(&conn, &plan_of(rows)).expect("first seed");
    assert_eq!(first.inserted, 2);

    let before: Vec<(i64, String, String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, anchor_id, created_at, title, IFNULL(in_force_until,'') \
                 FROM policy_anchor ORDER BY id",
            )
            .expect("prepare");
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .expect("query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("rows")
    };
    assert_eq!(before.len(), 2);
    let audit_after_first: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE kind = 'policy-anchor'",
            [],
            |r| r.get(0),
        )
        .expect("audit count");

    // Identical replay: skips, never an UPDATE, never a new evidence row
    // beyond the act's own.
    let second = pa::seed(
        &conn,
        &plan_of(vec![
            planned_held(
                "PH-IDEM",
                "1111111111111111111111111111111111111111111111111111111111111111",
                "x.pdf",
            ),
            planned_owed("PH-IDEM-2", "2026-03-03"),
        ]),
    )
    .expect("second seed");
    assert_eq!(second.inserted, 0, "identical content skips");
    assert_eq!(second.skipped, 2);
    let audit_after_second: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE kind = 'policy-anchor'",
            [],
            |r| r.get(0),
        )
        .expect("audit count");
    assert_eq!(
        audit_after_second,
        audit_after_first + 1,
        "ZERO new audit rows beyond the seed acts: one row per act, nothing per skipped row"
    );

    let after: Vec<(i64, String, String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, anchor_id, created_at, title, IFNULL(in_force_until,'') \
                 FROM policy_anchor ORDER BY id",
            )
            .expect("prepare");
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .expect("query")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("rows")
    };
    assert_eq!(
        before, after,
        "the replay rewrote NOTHING: ids, created_at, and every content field are the \
         row's history"
    );

    // Same anchor_id, different hash: refuse BY NAME.
    let drifted = pa::seed(
        &conn,
        &plan_of(vec![planned_held(
            "PH-IDEM",
            "2222222222222222222222222222222222222222222222222222222222222222",
            "x.pdf",
        )]),
    )
    .unwrap_err();
    assert!(
        drifted.contains("PH-IDEM"),
        "the refusal names the anchor: {drifted}"
    );
    assert!(
        drifted.contains("supersede"),
        "the refusal names the only lawful change: {drifted}"
    );
    // The refusal left the row untouched.
    let sha: String = conn
        .query_row(
            "SELECT sha256_hex FROM policy_anchor WHERE anchor_id = 'PH-IDEM'",
            [],
            |r| r.get(0),
        )
        .expect("sha");
    assert_eq!(
        sha, "1111111111111111111111111111111111111111111111111111111111111111",
        "the refused offer did not touch the stored row"
    );
}

// ── pin 4 ───────────────────────────────────────────────────────────────

/// The pack gate refuses and names EVERY blocker with its distinct reason:
/// a binding over one verified + one fetch-owed + one superseded + one
/// not-in-the-registry anchor lists all three blockers (never just the
/// first), the fetch-owed reason is the verbatim advisory sentence, and a
/// verified anchor passes. THE DRILL (run and reverted): return only the
/// first blocker and the pin reds with `left: 1 right: 3`.
#[test]
fn the_pack_gate_refuses_and_names_every_blocker() {
    let conn = db();
    let held_digest = "3".repeat(64);
    pa::seed(
        &conn,
        &plan_of(vec![
            planned_held("PH-GATE-OK", &held_digest, "ok.pdf"),
            planned_owed("PH-GATE-OWED", "2026-01-01"),
        ]),
    )
    .expect("seed");
    // The superseded lineage.
    let old_id: i64 = conn
        .query_row(
            "SELECT id FROM policy_anchor WHERE anchor_id = 'PH-GATE-OWED'",
            [],
            |r| r.get(0),
        )
        .expect("owed row");
    pa::supersede(
        &conn,
        old_id,
        &pa::NewVersion {
            anchor_id: "PH-GATE-OWED".to_string(),
            issuer: "Test Issuer".to_string(),
            instrument_no: "Test Instrument No. 2".to_string(),
            series: "2026".to_string(),
            title: "v2 of the owed instrument".to_string(),
            scope_tags_json: "[\"test\"]".to_string(),
            sha256_hex: None,
            state: pa::AnchorLifecycle::TextFetchOwed,
            corpus_path: None,
            official_url: None,
            notes: None,
            in_force_from: "2026-05-01".to_string(),
        },
    )
    .expect("supersede");

    // A verified + a fetch-owed (fresh id) + a superseded lineage + an
    // anchor the registry has never heard of.
    pa::seed(
        &conn,
        &plan_of(vec![planned_owed("PH-GATE-OWED-2", "2026-06-01")]),
    )
    .expect("seed the fresh owed");

    let required = [
        "PH-GATE-OK".to_string(),
        "PH-GATE-OWED-2".to_string(),
        "PH-GATE-SUP".to_string(),
        "PH-GATE-ABSENT".to_string(),
    ];
    // PH-GATE-SUP: supersede the OK row's sibling so a superseded lineage
    // exists under that id without a live row.
    pa::seed(
        &conn,
        &plan_of(vec![planned_owed("PH-GATE-SUP", "2026-02-02")]),
    )
    .expect("seed the to-be-superseded");
    let sup_id: i64 = conn
        .query_row(
            "SELECT id FROM policy_anchor WHERE anchor_id = 'PH-GATE-SUP'",
            [],
            |r| r.get(0),
        )
        .expect("sup row");
    pa::supersede(
        &conn,
        sup_id,
        &pa::NewVersion {
            anchor_id: "PH-GATE-SUP-V2".to_string(),
            issuer: "Test Issuer".to_string(),
            instrument_no: "Test Instrument No. 2".to_string(),
            series: "2026".to_string(),
            title: "renamed lineage".to_string(),
            scope_tags_json: "[\"test\"]".to_string(),
            sha256_hex: None,
            state: pa::AnchorLifecycle::TextFetchOwed,
            corpus_path: None,
            official_url: None,
            notes: None,
            in_force_from: "2026-07-01".to_string(),
        },
    )
    .expect("supersede the lineage");

    let outcome = pa::pack_gate(&conn, "gov.test", Some(&required)).expect("gate");
    let pa::PackGateOutcome::Decided(verdict) = outcome else {
        panic!("bindings were configured; the gate must decide");
    };
    assert!(!verdict.passed, "the pack is blocked");
    assert_eq!(verdict.verified.len(), 1, "the verified anchor passes");
    assert_eq!(
        verdict.blockers.len(),
        3,
        "EVERY blocker is named, never just the first: {:?}",
        verdict.blockers
    );

    let by_id = |id: &str| {
        verdict
            .blockers
            .iter()
            .find(|b| b.anchor_id == id)
            .unwrap_or_else(|| panic!("blocker {id} must be named"))
    };
    // Three DISTINCT reasons.
    assert_eq!(
        by_id("PH-GATE-OWED-2").reason,
        pa::GateBlockerReason::TextNotHeld,
    );
    assert_eq!(
        by_id("PH-GATE-SUP").reason,
        pa::GateBlockerReason::Superseded
    );
    assert_eq!(
        by_id("PH-GATE-ABSENT").reason,
        pa::GateBlockerReason::NotInRegistry
    );
    // The advisory sentence, VERBATIM — an unverified deployment must never
    // read as a verified one.
    assert_eq!(
        by_id("PH-GATE-OWED-2").reason.text(),
        "advisory-only — anchor text not held"
    );
    assert_ne!(
        by_id("PH-GATE-OWED-2").reason.text(),
        by_id("PH-GATE-SUP").reason.text()
    );

    // The decided verdict recorded its bindings (the durable record).
    let bound: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM policy_pack_binding WHERE pack_id = 'gov.test'",
            [],
            |r| r.get(0),
        )
        .expect("bindings");
    assert!(
        bound >= 2,
        "the verified + the known blockers bind rows; got {bound}"
    );
    let acts: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE kind = 'policy-anchor'",
            [],
            |r| r.get(0),
        )
        .expect("audit");
    assert!(acts >= 1, "the gate act carries its evidence row");

    // The fail-closed knob arm, service-level: no bindings is never
    // everything-allowed.
    let unset = pa::pack_gate(&conn, "gov.other", None).expect("gate unset");
    assert_eq!(unset, pa::PackGateOutcome::NoBindingsConfigured);
    let empty = pa::pack_gate(&conn, "gov.other", Some(&[])).expect("gate empty");
    assert_eq!(empty, pa::PackGateOutcome::NoBindingsConfigured);
}

// ── pin 5 ───────────────────────────────────────────────────────────────

/// The supersede is one transaction or nothing: an injected failure after
/// the new-row insert (a poisoned trigger) leaves the old row UNSTAMPED,
/// writes NO audit row, and the new row is ABSENT — the partial state the
/// two-transaction shape would leave behind is exactly what this pin
/// refuses. THE DRILL (run and reverted): split the supersede into two
/// separate transactions and the pin reds on the partial-state assertion.
#[test]
fn the_supersede_is_one_transaction_or_nothing() {
    let conn = db();
    pa::seed(&conn, &plan_of(vec![planned_owed("PH-TX", "2026-01-01")])).expect("seed");
    let old_id: i64 = conn
        .query_row(
            "SELECT id FROM policy_anchor WHERE anchor_id = 'PH-TX'",
            [],
            |r| r.get(0),
        )
        .expect("row");
    let audit_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .expect("audit before");

    // The poison: any INSERT carrying the old anchor_id aborts.
    conn.execute_batch(
        "CREATE TRIGGER poison_supersede BEFORE INSERT ON policy_anchor
         WHEN NEW.anchor_id = 'PH-TX'
         BEGIN SELECT RAISE(ABORT, 'poisoned'); END;",
    )
    .expect("poison trigger");

    let err = pa::supersede(
        &conn,
        old_id,
        &pa::NewVersion {
            anchor_id: "PH-TX".to_string(),
            issuer: "Test Issuer".to_string(),
            instrument_no: "Test Instrument No. 2".to_string(),
            series: "2026".to_string(),
            title: "v2".to_string(),
            scope_tags_json: "[\"test\"]".to_string(),
            sha256_hex: None,
            state: pa::AnchorLifecycle::TextFetchOwed,
            corpus_path: None,
            official_url: None,
            notes: None,
            in_force_from: "2026-04-01".to_string(),
        },
    )
    .expect_err("the poisoned supersede must fail");
    assert!(
        err.contains("supersede insert"),
        "the error names the failing step: {err}"
    );

    // THE PARTIAL-STATE ASSERTIONS — all three, or the pin has not pinned
    // the transaction boundary.
    let (state, until): (String, Option<String>) = conn
        .query_row(
            "SELECT state, in_force_until FROM policy_anchor WHERE id = ?1",
            [old_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("old row");
    assert_eq!(state, "text_fetch_owed", "the old row was NOT stamped");
    assert_eq!(until, None, "the old row's in_force_until stayed NULL");
    let new_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM policy_anchor WHERE title = 'v2'",
            [],
            |r| r.get(0),
        )
        .expect("new rows");
    assert_eq!(new_rows, 0, "the new row is absent");
    let audit_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .expect("audit after");
    assert_eq!(
        audit_after, audit_before,
        "no audit row for an act that rolled back"
    );

    // And the registry still works after the rollback (no half-open tx).
    let replay = pa::state_as_of(&conn, "2026-02-01").expect("post-rollback read");
    assert_eq!(replay.len(), 1);
}

// ── pin 6 ───────────────────────────────────────────────────────────────

/// The CLI is wiring only: the three policy-anchor commands' bodies carry
/// no SQL strings and no sha2 — flags, parse, service calls, print, exit.
/// The logic lives in the service core; a second implementation in the
/// binary is the drift this pin kills. THE DRILL (run and reverted): inline
/// the hash check into `cmd_policy_anchor_seed` and the pin reds naming the
/// line.
#[test]
fn the_cli_is_wiring_only() {
    let src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bin/brain.rs"))
            .expect("read brain.rs");
    for handler in [
        "fn cmd_policy_anchor_seed(",
        "fn cmd_policy_anchor_verify(",
        "fn cmd_policy_anchor_gate(",
        "fn policy_open_db(",
        "fn policy_print_anchor(",
        "fn db_path_of(",
    ] {
        let start = src
            .find(handler)
            .unwrap_or_else(|| panic!("{handler} must exist — the CLI's wiring set is closed"));
        let body: &str = &src[start..];
        let end = body[1..].find("\nfn ").map(|i| i + 1).unwrap_or(body.len());
        let body = &body[..end];
        for banned in [
            "INSERT INTO",
            "UPDATE ",
            "SELECT ",
            "CREATE TABLE",
            "Sha256",
            "sha2::",
            "digest(",
        ] {
            assert!(
                !body.contains(banned),
                "{handler} must stay wiring only: it names '{banned}' — SQL and hashing \
                 live in service::policy_anchor; the binary parses flags, calls the service, \
                 prints, and sets the exit code"
            );
        }
    }
}

// ── pin 7 ───────────────────────────────────────────────────────────────

/// The unpacked deployment is unchanged — driven through the REAL binary:
/// with the knob unset, `gate` refuses with the no-bindings sentence and
/// exit 2 (fail-closed on the knob too: no bindings is never
/// everything-allowed), while seed and verify behave exactly as S3's
/// receipts. THE DRILL (run and reverted): make unset mean silent allow-all
/// in the gate command and this pin reds.
#[test]
fn the_unpacked_deployment_is_unchanged() {
    let corpus = mini_corpus();
    let scratch = tempfile::tempdir().expect("tempdir");
    let db_path = scratch.path().join("scratch.db");
    let brain = env!("CARGO_BIN_EXE_brain");

    // The leash is the ENVIRONMENT: BRAIN_DB_PATH pinned to the scratch.
    let base_env = |cmd: &mut Command| {
        cmd.env("BRAIN_DB_PATH", &db_path);
        cmd.env_remove("BRAIN_POLICY_PACK_REQUIRED_ANCHORS");
    };

    // seed: byte-behavior-identical to S3's receipt.
    let out = {
        let mut cmd = Command::new(brain);
        base_env(&mut cmd);
        cmd.args([
            "policy-anchor",
            "seed",
            "--manifest",
            corpus.manifest_path.to_str().expect("utf8"),
            "--corpus-root",
            corpus.dir.to_str().expect("utf8"),
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run seed")
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "seed exits 0: {stdout}");
    assert!(
        stdout.contains("inserted 2 · skipped 0 · refused 0"),
        "the receipt is S3's: {stdout}"
    );
    assert!(
        stdout.contains("2 held / 1 fetch-owed") || stdout.contains("1 held / 1 fetch-owed"),
        "state counts ride the receipt: {stdout}"
    );

    // verify: same, with the knob still unset.
    let out = {
        let mut cmd = Command::new(brain);
        base_env(&mut cmd);
        cmd.args([
            "policy-anchor",
            "verify",
            "--db",
            db_path.to_str().expect("utf8"),
            "--corpus-root",
            corpus.dir.to_str().expect("utf8"),
        ])
        .output()
        .expect("run verify")
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "verify exits 0: {stdout}");
    assert!(
        stdout.contains("verified against the corpus bytes"),
        "the drift detector ran clean: {stdout}"
    );

    // gate, knob unset: the no-bindings refusal, exit 2.
    let out = {
        let mut cmd = Command::new(brain);
        base_env(&mut cmd);
        cmd.args([
            "policy-anchor",
            "gate",
            "--pack",
            "gov.test",
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run gate")
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unset knob is the refusal, not an allow-all: {stdout}"
    );
    assert!(
        stdout.contains("no pack bindings configured"),
        "the refusal names the knob: {stdout}"
    );
    assert!(
        stdout.contains("gov.test"),
        "the refusal names the pack: {stdout}"
    );

    // gate, knob set over the VERIFIED anchor: exit 0.
    let out = {
        let mut cmd = Command::new(brain);
        base_env(&mut cmd);
        cmd.env(
            "BRAIN_POLICY_PACK_REQUIRED_ANCHORS",
            r#"[{"pack":"gov.test","anchors":["PH-TEST-HELD"]}]"#,
        );
        cmd.args([
            "policy-anchor",
            "gate",
            "--pack",
            "gov.test",
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run gate verified")
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "the verified pack activates: {stdout}"
    );
    assert!(stdout.contains("verified"), "{stdout}");

    // gate, knob set over the FETCH-OWED anchor: exit 2, advisory verbatim
    // (the honesty arm — an unverified deployment must never read as a
    // verified one).
    let out = {
        let mut cmd = Command::new(brain);
        base_env(&mut cmd);
        cmd.env(
            "BRAIN_POLICY_PACK_REQUIRED_ANCHORS",
            r#"[{"pack":"gov.test","anchors":["PH-TEST-OWED"]}]"#,
        );
        cmd.args([
            "policy-anchor",
            "gate",
            "--pack",
            "gov.test",
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run gate advisory")
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(2),
        "a fetch-owed bound anchor refuses: {stdout}"
    );
    assert!(
        stdout.contains("advisory-only — anchor text not held"),
        "the advisory sentence is verbatim: {stdout}"
    );
}

// ── the knob's broken state is NAMED on the command line ─────────────────

/// A knob that is SET BUT BROKEN must not read as UNSET. Both are refused
/// (fail-closed either way), but they are different facts and the operator
/// is owed the true one. config's reader WARNs through `tracing`; the brain
/// CLI installs no tracing subscriber, so that warning reached nobody, and a
/// mistyped knob printed "unset or does not name it". Found while fact-
/// checking a blog post's sentence ("with a warning") against the real
/// binary: stderr was EMPTY. Now the gate parses with config's own pure
/// parser and names the parse error on stderr. Driven through the REAL
/// binary: the broken knob warns AND refuses; the unset knob refuses with NO
/// warning (unset is not broken); the valid knob activates silently.
/// THE DRILL (run and reverted): restore the silent `policy_pack_required_
/// anchors()` read and this pin reds with `a broken knob must SAY so: ""`.
#[test]
fn a_broken_knob_is_refused_and_says_so_on_stderr() {
    let corpus = mini_corpus();
    let scratch = tempfile::tempdir().expect("tempdir");
    let db_path = scratch.path().join("scratch.db");
    let brain = env!("CARGO_BIN_EXE_brain");
    let gate = |knob: Option<&str>| {
        let mut cmd = Command::new(brain);
        // The leash is the ENVIRONMENT: BRAIN_DB_PATH pinned to the scratch.
        cmd.env("BRAIN_DB_PATH", &db_path);
        cmd.env_remove("BRAIN_POLICY_PACK_REQUIRED_ANCHORS");
        if let Some(k) = knob {
            cmd.env("BRAIN_POLICY_PACK_REQUIRED_ANCHORS", k);
        }
        cmd.args([
            "policy-anchor",
            "gate",
            "--pack",
            "gov.test",
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run gate")
    };

    // Seed once so the registry holds something to gate against.
    let seed = {
        let mut cmd = Command::new(brain);
        cmd.env("BRAIN_DB_PATH", &db_path);
        cmd.args([
            "policy-anchor",
            "seed",
            "--manifest",
            corpus.manifest_path.to_str().expect("utf8"),
            "--corpus-root",
            corpus.dir.to_str().expect("utf8"),
            "--db",
            db_path.to_str().expect("utf8"),
        ])
        .output()
        .expect("run seed")
    };
    assert_eq!(
        seed.status.code(),
        Some(0),
        "seed exits 0: {}",
        String::from_utf8_lossy(&seed.stdout)
    );

    // A VALID binding followed by garbage: half of it would have parsed.
    let broken = gate(Some(
        r#"[{"pack":"gov.test","anchors":["PH-TEST-HELD"]}] oops"#,
    ));
    let stderr = String::from_utf8_lossy(&broken.stderr);
    let stdout = String::from_utf8_lossy(&broken.stdout);
    assert_eq!(
        broken.status.code(),
        Some(2),
        "a broken knob is refused, never half-applied: {stdout}"
    );
    assert!(
        stderr.contains("BRAIN_POLICY_PACK_REQUIRED_ANCHORS is set but not parsable"),
        "a broken knob must SAY so on stderr (the CLI has no tracing subscriber, \
         so a tracing::warn reaches nobody here): {stderr:?}"
    );
    assert!(
        stderr.contains("trailing characters"),
        "the warning names the parse error: {stderr:?}"
    );
    assert!(
        stderr.contains("IGNORED in full"),
        "the warning states the whole knob was dropped: {stderr:?}"
    );
    // The warning is ONE readable line, not a literal newline plus source
    // indentation printed mid-sentence.
    assert_eq!(
        stderr.trim_end().lines().count(),
        1,
        "the warning prints on one line: {stderr:?}"
    );

    // UNSET is a different fact: refused, with nothing on stderr.
    let unset = gate(None);
    let unset_err = String::from_utf8_lossy(&unset.stderr);
    assert_eq!(
        unset.status.code(),
        Some(2),
        "an unset knob is still the refusal"
    );
    assert!(
        unset_err.trim().is_empty(),
        "unset is not broken: nothing is written to stderr, got {unset_err:?}"
    );

    // VALID: activates, silently.
    let valid = gate(Some(r#"[{"pack":"gov.test","anchors":["PH-TEST-HELD"]}]"#));
    let valid_err = String::from_utf8_lossy(&valid.stderr);
    assert_eq!(valid.status.code(), Some(0), "a valid knob activates");
    assert!(
        valid_err.trim().is_empty(),
        "a valid knob warns about nothing, got {valid_err:?}"
    );
}

// ── the knob's pure parser ──────────────────────────────────────────────

/// The knob parser's arms, driven pure (no process env): unset is handled
/// by the caller; unparsable JSON is an Err naming the failure (the caller
/// WARNs and drops the knob in full); a duplicate pack id refuses (a knob
/// that contradicts itself never half-applies); an empty anchors array is
/// kept as written and the gate renders it as the no-bindings refusal.
#[test]
fn the_knob_parser_refuses_contradictions_and_keeps_the_closed_shape() {
    use brain_server::config::parse_required_anchors;

    let ok = parse_required_anchors(r#"[{"pack":"gov.education","anchors":["PH-A","PH-B"]}]"#)
        .expect("parses");
    assert_eq!(
        ok,
        vec![(
            "gov.education".to_string(),
            vec!["PH-A".to_string(), "PH-B".to_string()]
        )]
    );

    // An empty anchors array is kept — the gate turns it into the
    // fail-closed no-bindings refusal, not an allow-all.
    let empty = parse_required_anchors(r#"[{"pack":"gov.x","anchors":[]}]"#).expect("parses");
    assert_eq!(empty, vec![("gov.x".to_string(), Vec::<String>::new())]);

    assert!(parse_required_anchors("not json").is_err());
    assert!(parse_required_anchors(r#"{"pack":"gov"}"#).is_err());
    let dup = parse_required_anchors(
        r#"[{"pack":"gov","anchors":["A"]},{"pack":"gov","anchors":["B"]}]"#,
    )
    .unwrap_err();
    assert!(dup.contains("bound twice"), "{dup}");
}

// ── the fixture's shape (the registry's contract with the corpus) ───────

/// tests/fixtures/policy_anchors.json is the round's fixture: a
/// byte-identical copy of the corpus's master manifest. The shape is the
/// contract — manifest_version 1, twelve anchors, the state/hash law at the
/// boundary (verified_text_held carries BOTH digest and corpus path;
/// text_fetch_owed carries neither), unique ids. WHERE THE CORPUS ROOT IS
/// PRESENT (the sibling steward-ip checkout), the fixture's two held
/// digests are re-measured against the real bytes LIVE — the S0 receipt,
/// pinned; where it is absent (a CI checkout), the arm skips HONESTLY,
/// naming why, never silently green.
#[test]
fn the_fixture_manifests_shape_is_the_registrys_contract() {
    let raw = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy_anchors.json"),
    )
    .expect("the round's fixture must exist");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");
    assert_eq!(v["manifest_version"], 1);
    let anchors = v["anchors"].as_array().expect("anchors array");
    assert_eq!(anchors.len(), 12, "the measured census: twelve instruments");

    let mut held = 0;
    let mut owed = 0;
    let mut ids = std::collections::HashSet::new();
    for a in anchors {
        let id = a["id"].as_str().expect("id");
        assert!(ids.insert(id), "duplicate anchor id {id} in the fixture");
        match a["state"].as_str().expect("state") {
            "verified_text_held" => {
                held += 1;
                let sha = a["sha256_hex"].as_str().expect("held carries a digest");
                assert_eq!(sha.len(), 64, "64 hex chars: {id}");
                assert!(
                    a["corpus_path"].is_string(),
                    "held carries a corpus path: {id}"
                );
            }
            "text_fetch_owed" => {
                owed += 1;
                assert!(
                    a["sha256_hex"].is_null(),
                    "fetch-owed carries NO digest (the manifest's own hash law): {id}"
                );
                assert!(
                    a["corpus_path"].is_null(),
                    "fetch-owed carries NO corpus path: {id}"
                );
            }
            other => panic!("state '{other}' is outside the registry's vocabulary: {id}"),
        }
    }
    assert_eq!(held, 2, "the measured census: two held texts");
    assert_eq!(owed, 10, "the measured census: ten fetch-owed");

    // The corpus root's presence probe (honest absence, never silent green).
    let corpus_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("parent")
        .join("brain-steward-ip/modules/gov/ph-ai-policies");
    if !corpus_root.exists() {
        println!(
            "SKIP (named): the sibling corpus checkout is absent at {} — the two held \
             digests are re-measured live by the S0 receipt on the corpus machine; this \
             arm refuses to pass vacuously on a tree that cannot measure",
            corpus_root.display()
        );
        return;
    }
    let plan = pa::verify_manifest(&raw, Some(&corpus_root))
        .expect("the fixture verifies live against the corpus: hashes match, schema clean");
    assert_eq!(plan.rows.len(), 12, "every anchor plans");
    assert!(
        plan.refusals.is_empty(),
        "no schema refusals: {:?}",
        plan.refusals
    );
    let held_rows = plan
        .rows
        .iter()
        .filter(|r| r.state == pa::AnchorLifecycle::VerifiedTextHeld)
        .count();
    assert_eq!(held_rows, 2);
}
