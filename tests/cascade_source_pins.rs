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
    // One session = one tier (TC-6). The coordinator IS the AFM tier: a
    // `model: Some(...)` on its BridgeTurn would be the mid-session-swap
    // shape (the bridge locks a session's model at first use; `model_locked`
    // is the refusal). The grep-law shape (the docling shim's
    // structural-pin precedent): the BridgeTurn's model field is `None` —
    // the bridge writes the wire key only when `Some`, so `None` IS the
    // wire absence — and no `Some` assignment exists anywhere in the
    // module.
    let src = read(MODULE);
    assert!(
        src.contains("model: None"),
        "the coordinator's BridgeTurn must set model: None (the bridge \
         writes the wire key only when Some — None IS the wire absence, \
         absent = afm3-core by the bridge's own law)"
    );
    assert!(
        !src.contains("model: Some"),
        "the coordinator must never assign a model on the wire (one session \
         = one tier)"
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
