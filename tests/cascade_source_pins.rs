//! Cascade source pins (round 17, S1) — the STANDALONE-compile class: every
//! pin here reads the tree at RUNTIME (no compile dependency on the
//! coordinator module), so the set is born red against the pre-S1 tree by
//! file-absence and greens in-commit (the TC-8 file-absence precedent).
//!
//! The TC-1/TC-6/TC-7 structural class:
//! - the coordinator module exists and registers in the agentloop;
//! - the coordinator never sends a `model` field (TC-6's AFM half: one
//!   session = one tier — the coordinator IS the AFM tier, and a `model`
//!   field on its wire would be the mid-session-swap shape);
//! - the guided route schema rides the round-14 wire (the `schema` turn
//!   field), never a new socket;
//! - the audit kind lands as ONE enum variant + its `as_str` arm (the
//!   `AuditKind::DocCorrection` precedent, external_claim_pins.rs:1144);
//! - the classify path writes its audit row (TC-7) through the existing
//!   `audit::record` door;
//! - no direct memory write anywhere in the coordinator (TC-1: cascade
//!   output lands as a returned VALUE — the module carries no INSERT and
//!   no proposal write of its own).

use std::path::Path;

const MODULE: &str = "src/agentloop/cascade.rs";
const MOD_RS: &str = "src/agentloop/mod.rs";
const AUDIT_RS: &str = "src/audit/mod.rs";

fn read(p: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let full = Path::new(manifest).join(p);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("source pin read {p}: {e}"))
}

#[test]
fn tc1_module_exists_and_registers_in_the_agentloop() {
    let src = read(MODULE);
    assert!(
        src.contains("pub(crate) enum RouteDecision"),
        "the route decision must be a typed enum in the coordinator module"
    );
    let mod_rs = read(MOD_RS);
    assert!(
        mod_rs.contains("pub(crate) mod cascade;"),
        "the coordinator must register in src/agentloop/mod.rs"
    );
}

#[test]
fn tc6_coordinator_never_sends_a_model_field() {
    // One session = one tier (TC-6), BOTH halves: the coordinator's
    // classification turn stays `model: None` (absent = afm3-core by the
    // bridge's own law) and the writer turn carries exactly the configured
    // cascade writer tier — one model per tier turn, from its own seam, so
    // a mid-session swap is never expressible. The grep-law shape (the
    // docling shim's structural-pin precedent).
    let src = read(MODULE);
    assert!(
        src.contains("model: None"),
        "the coordinator's classification turn must set model: None (the \
         bridge writes the wire key only when Some — None IS the wire \
         absence, absent = afm3-core by the bridge's own law)"
    );
    // The WRITER tier (S3) carries ITS tier's model — exactly one model
    // assignment per tier turn, each from its own seam, so a mid-session
    // swap is never expressible (one session = one tier, both halves).
    assert!(
        src.contains("model: crate::config::cascade_writer_model()"),
        "the writer turn must carry the configured cascade writer tier — \
         the T3 half of one-session-one-tier (the Option rides the BridgeTurn \
         directly; the child validates the vocabulary)"
    );
    // Exactly TWO model assignments — the coordinator's None and the
    // writer's configured tier; any third is an unnamed tier sneaking in.
    assert!(!src.contains("model: Some("), "no literal Some model");
    let assigned = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("model: None")
                || t.starts_with("model: crate::config::cascade_writer_model()")
        })
        .count();
    assert_eq!(
        assigned, 2,
        "exactly two model assignments (the coordinator's None + the \
         writer's configured tier)"
    );
}

#[test]
fn tc2_route_schema_rides_the_documented_wire() {
    let src = read(MODULE);
    assert!(
        src.contains("schema:"),
        "the route classification must ride the round-14 schema turn field"
    );
    assert!(
        src.contains("answer_from_memory")
            && src.contains("read_more")
            && src.contains("synthesize")
            && src.contains("escalate_frontier"),
        "the four-case roster must be named in the coordinator"
    );
    // No new socket: the only egress is the bridge child.
    assert!(
        !src.contains("TcpStream") && !src.contains("reqwest") && !src.contains("http::"),
        "the coordinator must not open sockets — the AFM bridge (stdio) is \
         the one door"
    );
}

#[test]
fn tc7_audit_kind_is_one_variant_plus_one_arm() {
    let audit = read(AUDIT_RS);
    assert!(
        audit.contains("Cascade,"),
        "AuditKind gains the Cascade variant (the DocCorrection precedent)"
    );
    assert!(
        audit.contains(r#""cascade""#),
        "the variant's as_str arm names it"
    );
    let src = read(MODULE);
    assert!(
        src.contains("AuditKind::Cascade"),
        "the classify path must write its audit row through the existing \
         audit::record door"
    );
    assert!(
        src.contains("audit::record"),
        "the row rides audit::record (no private audit write)"
    );
}

#[test]
fn tc1_no_memory_write_in_the_coordinator() {
    // Cascade output lands as a returned VALUE; the coordinator performs no
    // INSERT of its own (memory entry is HITL-only, the gate law).
    let src = read(MODULE);
    for banned in [
        "INSERT INTO chunks",
        "INSERT INTO proposals",
        "ingest_proposal",
        "create_proposal",
    ] {
        assert!(
            !src.contains(banned),
            "the coordinator must not write memory or proposals ({banned})"
        );
    }
}

#[test]
fn tc3_the_reader_is_model_absent_by_construction() {
    // TC-3's structural law: NO model import anywhere in the reader's path.
    // The module's ONE async function is T1's classify_route (the bridge
    // turn) — the reader adds NO async surface at all, so it cannot call a
    // model; and the module names no model-serving wire anywhere.
    let src = read(MODULE);
    // The module carries exactly TWO async fns — T1's classify_route and
    // T3's writer_turn — each the ONLY async surface of its tier. The
    // READER (T2) adds none: it is pure functions over fetched results,
    // structurally unable to call a model.
    let async_fns: Vec<String> = src
        .lines()
        .filter(|l| l.contains("async fn"))
        .filter_map(|l| {
            let name = l.split("async fn").nth(1)?.trim();
            let name = name.split('(').next()?.trim();
            Some(name.to_string())
        })
        .collect();
    assert_eq!(
        async_fns,
        vec!["classify_route".to_string(), "writer_turn".to_string(),],
        "the cascade's async surface is exactly the two named fns by name — \
         the reader stays model-absent"
    );
    for banned in ["chat/completions", "MlxOut", "embed("] {
        assert!(
            !src.contains(banned),
            "the cascade module must not name a model-serving wire ({banned})"
        );
    }
}

#[test]
fn s2_the_reader_rides_the_substrate_caps_not_new_ones() {
    // The reader's limits come from the substrate (BRAIN_RECALL_* at the
    // run_recall call the caller makes) and the packer's preregistered
    // caps — the module must not invent a second recall implementation.
    let src = read(MODULE);
    assert!(
        src.contains("EVIDENCE_MAX_HITS")
            && src.contains("EVIDENCE_HIT_CAP_CHARS")
            && src.contains("EVIDENCE_TOTAL_CAP_CHARS"),
        "the packer's caps are named constants (S0's preregistered law)"
    );
    assert!(
        src.contains("wrap_fenced"),
        "the evidence block rides the kernel's own fence"
    );
}
