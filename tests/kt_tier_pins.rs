//! The KT-tier pins (IMPROVE14): the wall between intake-usable and
//! tuple-producing, cleared by measurement — with the honesty law as the
//! veto. Structural/wire pins here; the BEHAVIORAL half of KT-1/KT-2
//! lives in `gdl.rs`'s tests module (an integration test cannot name the
//! `pub(crate)` `IntakeArtifact`/`IsNotTable` — the CG-05 W-1 note's
//! precedent).
//!
//! RED LEDGER (2026-11-11, born against the pre-step tree at fa4956d7 —
//! every pin here reds BY ABSENCE before T-b's implementation lands, and
//! the transcript of that red run is the red-first proof):
//!   KT-1-S the traceability gate lives in `intake_gate`'s code, not a
//!          comment: the KT loop calls the containment discipline on the
//!          stated (non-unknown) cells against the case's own inputs.
//!   KT-2-S `what.is` refuses the literal `unknown` BY NAME (the
//!          preregistered axis: an unbounded WHAT is no problem
//!          statement); the other seven cells keep `unknown` recorded.
//!   KT-3-S the artifact schema rides the wire: `BridgeTurn.schema`,
//!          the turn body's `schema` key, the child's parse-time decode
//!          with named refusals, the GroundedAnswer contract untouched
//!          when the field is absent, the ONE generic Swift converter in
//!          the LIBRARY target, and the pure Rust builder in `gdl.rs`
//!          pinned against the struct.
//! Every scan runs against comment-stripped code (`common::code_only`)
//! and every scan carries its planted-miss control: a scanner that
//! cannot red is the one defect a structural pin must never have.

use std::path::Path;

mod common;

/// Read a repo file relative to the manifest dir (the ai_act_pins `doc()`
/// helper's twin — the pins read the tree, not copies).
fn doc(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} must exist and be readable: {e}"))
}

/// A region scanner with a FIXED byte window from a start marker — the
/// provider_afm M-4 pin's shape — so a second unrelated occurrence of a
/// token elsewhere in the file cannot satisfy a pin about one site.
fn region<'a>(code: &'a str, start_marker: &str, window: usize) -> &'a str {
    let at = code
        .find(start_marker)
        .unwrap_or_else(|| panic!("the pinned site exists (live, not a comment): {start_marker}"));
    &code[at..at + window.min(code.len() - at)]
}

/// A token the pin NEEDS in the region.
fn needs(region_text: &str, token: &str, site: &str) {
    assert!(
        region_text.contains(token),
        "the {site} law drifted: missing `{token}` in the pinned region"
    );
}

/// A token the pin FORBIDS in the region (the fail-closed twin — a region
/// that must NOT carry a shape reds when it does).
fn forbids(region_text: &str, token: &str, site: &str) {
    assert!(
        !region_text.contains(token),
        "the {site} law drifted: forbidden `{token}` present in the pinned region"
    );
}

// ── KT-1-S: the traceability gate is CODE, in the KT loop ──────────────

#[test]
fn kt1_the_traceability_gate_lives_in_intake_gates_code() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    // The KT loop's region: from the four-row table walk to the L1
    // telemetry arm — the traceability discipline must sit inside it.
    let at = gdl
        .find("for (name, row) in [")
        .unwrap_or_else(|| panic!("the KT loop exists"));
    let region_text = &gdl[at..at + 4200.min(gdl.len() - at)];
    needs(region_text, "locator_resolves(", "KT-1 traceability");
    needs(region_text, "unknown", "KT-1 traceability");
    needs(
        region_text,
        "not traceable",
        "KT-1 the refusal names itself",
    );
}

/// KT-1's planted-miss control: the scanner sees a planted occurrence —
/// a scan that cannot red is the one defect a structural pin must never
/// have (the false-pass r51 lesson).
#[test]
fn kt1_scan_control_planted_token_is_seen() {
    let planted = "locator_resolves( planted_control_token ); not traceable unknown";
    needs(planted, "locator_resolves(", "KT-1 control");
    needs(planted, "not traceable", "KT-1 control");
    assert!(
        !planted.contains("absent_from_the_fixture"),
        "the scanner's negative arm works"
    );
}

// ── KT-2-S: the preregistered unknown axis is CODE ─────────────────────

#[test]
fn kt2_what_is_refuses_unknown_by_name_in_code() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    let at = gdl
        .find("fn intake_gate(")
        .unwrap_or_else(|| panic!("intake_gate exists"));
    // The scan runs FORWARD from the gate's own definition (the schema
    // builder sits before it and legitimately names the same refusal in
    // its guides — the pin wants the GATE's enforcement, not the guide).
    let body = &gdl[at..(at + 9000).min(gdl.len())];
    // The refusal text wraps with Rust's line continuation, so the pin
    // names the within-line fragment plus the message's distinctive head.
    needs(body, "unbounded WHAT is no", "KT-2 what.is unknown refusal");
    needs(body, "is_not.what.is is 'unknown'", "KT-2 the refusal head");
    // The refusal fires on the CELL VALUE (the unknown check), not merely
    // in prose: the check reads the cell and compares it to the literal.
    // The check reads the CELL through the helper, in the gate's loop.
    needs(body, "kt_cell_is_unknown(", "KT-2 the check");
    // And the helper itself carries the literal comparison (the whole-file
    // scan — the helper lives beside the citation machinery).
    needs(
        &gdl,
        "eq_ignore_ascii_case(\"unknown\")",
        "KT-2 the literal",
    );
}

/// KT-2's axis control: the OTHER seven cells keep `unknown` recorded —
/// the pin's own fixture proves the scan discriminates (what.is refuses,
/// the is_not columns and the when/extent rows do not carry the refusal).
#[test]
fn kt2_scan_control_the_axis_is_not_a_blanket_refusal() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    let at = gdl
        .find("fn intake_gate(")
        .unwrap_or_else(|| panic!("intake_gate exists"));
    let body = &gdl[at..at + 7600.min(gdl.len() - at)];
    // The refusal must be CONDITIONAL on the what.is cell: the check
    // carries the row name and the column, not a bare unknown-ban.
    needs(body, "name == \"what\"", "KT-2 the conditional");
    forbids(
        body.split("fn triage_gate(").next().unwrap_or(body),
        "every cell refuses unknown",
        "KT-2 no blanket-ban prose",
    );
}

// ── KT-3-S: the artifact schema rides the wire ─────────────────────────

#[test]
fn kt3a_the_bridge_turn_carries_the_optional_schema_over_the_wire() {
    let bridge = common::code_only(&doc("src/agentloop/bridge_afm.rs"));
    let at = bridge
        .find("pub struct BridgeTurn")
        .unwrap_or_else(|| panic!("BridgeTurn exists"));
    let body = &bridge[at..at + 1400.min(bridge.len() - at)];
    needs(body, "pub schema: Option<String>", "KT-3 the wire field");
    // The wire body writes the schema ONLY when present — absent is
    // byte-identical to every turn that shipped before the field existed.
    let turn_at = bridge
        .find("\"op\": \"turn\"")
        .unwrap_or_else(|| panic!("the turn body exists"));
    let turn_body = &bridge[turn_at..turn_at + 900.min(bridge.len() - turn_at)];
    needs(turn_body, "request.schema", "KT-3 the wire write");
}

/// KT-3a's control: the CHAT spine's construction carries no schema — the
/// chat contract stays byte-identical (the two never blend). The chat
/// call site's BridgeTurn literal must NOT set the field to Some.
#[test]
fn kt3a_control_the_chat_spine_sends_no_schema() {
    let chat = common::code_only(&doc("src/handlers/chat.rs"));
    let at = chat
        .find(".turn(BridgeTurn {")
        .unwrap_or_else(|| panic!("the chat construction exists"));
    let body = &chat[at..at + 1200.min(chat.len() - at)];
    needs(body, "schema: None", "KT-3 control the chat absence");
    forbids(body, "schema: Some", "KT-3 control the chat never guides");
}

#[test]
fn kt3b_the_child_decodes_the_schema_at_parse_time_and_refuses_by_name() {
    let child = common::code_only(&doc("shell-macos/Sources/brain-afm/main.swift"));
    // The decode, at parse time (before any Task or model work).
    needs(&child, "object[\"schema\"]", "KT-3 the Swift decode");
    needs(
        &child,
        "schema must be a non-empty JSON-schema document",
        "KT-3 the empty-schema refusal",
    );
    // The guided path: the runtime schema API and the ONE settled answer.
    needs(
        &child,
        "WireSchemaConverter.generationSchema(from:",
        "KT-3 the converter call",
    );
    needs(&child, "to: prompt, schema:", "KT-3 the guided respond");
    needs(&child, "\"guided\": true", "KT-3 the additive label");
    // THE CONTROL: the GroundedAnswer contract is untouched — the
    // compiled typed answer still streams for every schema-absent turn.
    needs(
        &child,
        "generating: GroundedAnswer.self",
        "KT-3 control the chat contract",
    );
}

/// KT-3b's control: the schema field is a STRING on the wire and a
/// non-string value refuses — a schema can never smuggle a Swift object
/// literal through JSONSerialization's any-typing.
#[test]
fn kt3b_control_the_schema_wire_type_is_a_string() {
    let child = common::code_only(&doc("shell-macos/Sources/brain-afm/main.swift"));
    needs(&child, "as? String", "KT-3 control the string-only decode");
    needs(
        &child,
        "schema must decode as a JSON object",
        "KT-3 control the malformed refusal",
    );
}

#[test]
fn kt3c_the_generic_converter_lives_in_the_library_target() {
    // The LIBRARY target (A test target cannot import an executable —
    // the GroundedAnswer.swift header's law); the converter is shared
    // vocabulary, so it lives where the pins can reach it.
    let conv = common::code_only(&doc(
        "shell-macos/Sources/AppleFMProbe/WireSchemaConverter.swift",
    ));
    needs(
        &conv,
        "public enum WireSchemaConverter",
        "KT-3 the converter enum",
    );
    needs(
        &conv,
        "public static func generationSchema(from",
        "KT-3 the entry point",
    );
    // The dialect: the five shapes the Rust builder emits, and the
    // fail-closed refusal for anything else.
    for shape in [
        "\"object\"",
        "\"string\"",
        "\"integer\"",
        "\"boolean\"",
        "\"array\"",
        "\"number\"",
        "\"anyOf\"",
        "unknownSchemaDialect",
    ] {
        needs(&conv, shape, "KT-3 the dialect");
    }
    // The runtime API chain, named.
    for api in [
        "DynamicGenerationSchema(",
        "GenerationSchema(root:",
        "DynamicGenerationSchema.Property(",
        "isOptional:",
        "arrayOf: itemSchema,",
    ] {
        needs(&conv, api, "KT-3 the runtime API");
    }
}

#[test]
fn kt3d_the_rust_builder_is_pure_and_pinned_against_the_struct() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    needs(
        &gdl,
        "pub(crate) fn intake_artifact_schema_json() -> String",
        "KT-3 the builder",
    );
    // The struct-pin mechanism: the serialized artifact's field names are
    // walked and every one must appear in the builder's document — a
    // field change without a schema change reds.
    needs(&gdl, "fn collect_field_names", "KT-3 the struct walk");
    // The guides point AT THE TICKET (the plan's Arm A sentence).
    needs(&gdl, "quote the ticket", "KT-3 the guides");
    // `unknown` is granted per the amended axis.
    needs(&gdl, "the literal string unknown", "KT-3 the unknown grant");
}

/// KT-3d's control: the builder is PURE — no clock, no DB, no provider
/// touch in its body (the document is a const-shaped literal built from
/// string pieces only).
#[test]
fn kt3d_control_the_builder_body_is_pure() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    let at = gdl
        .find("pub(crate) fn intake_artifact_schema_json() -> String")
        .expect("the builder exists");
    let body = &gdl[at..at + 3000.min(gdl.len() - at)];
    let end = body
        .find("\npub(crate) fn ")
        .or(body.find("\nfn "))
        .unwrap_or(body.len());
    let body = &body[..end];
    for banned in ["await", ".lock()", " Instant", "conn", "provider"] {
        forbids(body, banned, "KT-3 purity");
    }
}

// ── the mirror law: the Rust mirror and the Swift child name the same ──

#[test]
fn the_schema_mirror_names_the_same_wire_vocabulary_on_both_sides() {
    // Rust side: the field + the wire key.
    let bridge = common::code_only(&doc("src/agentloop/bridge_afm.rs"));
    needs(
        &bridge,
        "pub schema: Option<String>",
        "mirror the Rust field",
    );
    // Swift side: the decode key — the SAME word.
    let child = common::code_only(&doc("shell-macos/Sources/brain-afm/main.swift"));
    needs(&child, "object[\"schema\"]", "mirror the Swift key");
    // The adapter mirror: the provider maps the request/phase schema into
    // the bridge turn.
    let provider = common::code_only(&doc("src/agentloop/provider_afm.rs"));
    needs(
        &provider,
        "schema: request_schema",
        "mirror the adapter mapping",
    );
}

#[test]
fn the_pins_scanner_itself_is_not_vacuous() {
    // The meta-control: forbids() actually reds on a planted occurrence.
    let planted = "schema: Some(planted)";
    let caught = std::panic::catch_unwind(|| forbids(planted, "schema: Some", "meta")).is_err();
    assert!(caught, "forbids() must red on a planted occurrence");
}

// ── KT-4 (Arm B): the deterministic scaffold ───────────────────────────

#[test]
fn kt4_the_scaffold_is_pure_deterministic_and_ticket_derived() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    // The pure fn exists and takes the CASE (not the model's words).
    needs(
        &gdl,
        "pub(crate) fn intake_scaffold(case: &GdlCase) -> IsNotTable",
        "KT-4 the scaffold fn",
    );
    // The instruction presents it as CONFIRM/EDIT — the fill-then-reviewed
    // posture, not free authoring.
    needs(&gdl, "CONFIRM or EDIT", "KT-4 the confirm-edit posture");
    // The scaffold's own bounds trace BY CONSTRUCTION: the frame the
    // instruction carries is built from the ticket text, so the traceability
    // gate's law holds for a CONFIRMED scaffold without a special arm.
    needs(
        &gdl,
        "fn intake_scaffold(",
        "KT-4 the derivation lives in the gate's file",
    );
}

/// KT-4's planted-miss control: the scanner sees a planted fn signature.
#[test]
fn kt4_scan_control_planted_signature_is_seen() {
    let planted = "pub(crate) fn intake_scaffold(case: &GdlCase) -> IsNotTable {";
    needs(planted, "pub(crate) fn intake_scaffold(", "KT-4 control");
}

// ── KT-5 + KT-6 (Arm D): the surgical M-4 qualification ────────────────

#[test]
fn kt5_the_model_arm_is_qualified_triage_only_by_a_gate_in_code() {
    let provider = common::code_only(&doc("src/agentloop/provider_afm.rs"));
    // The construction's model arm routes through the qualified gate fn.
    needs(
        &provider,
        "model: qualified_model,",
        "KT-5 the qualified construction",
    );
    needs(
        &provider,
        "let qualified_model = self.qualified_model(",
        "KT-5 the gate decides outside the task",
    );
    // The gate fn: a NON-triage phase returns NONE even with a table entry
    // — the axis is enforced by the gate, not by the table's contents.
    let at = provider
        .find("fn qualified_model(")
        .unwrap_or_else(|| panic!("the qualified fn exists"));
    let body = &provider[at..(at + 1400).min(provider.len())];
    needs(body, "Some(\"triage\")", "KT-5 the triage-only arm");
    needs(body, "_ => None", "KT-5 the non-triage fail-closed arm");
}

/// KT-5's behavioral witness lives beside the gate fn (provider_afm's
/// tests module drives `qualified_model` directly — the red-proof: give
/// the table an `intake` entry and drive an intake exchange → None).
#[test]
fn kt5_scan_control_planted_gate_is_seen() {
    let planted = "fn qualified_model( — Some(\"triage\") — _ => None";
    needs(planted, "Some(\"triage\")", "KT-5 control");
}

#[test]
fn kt6_the_triage_tier_lands_in_the_hashed_policy_identity() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    // The driver carries the tier and the BIND hashes it: the policy
    // snapshot's model arg annotates the tier, so a tier flip MOVES the
    // hash — the re-baseline is loud, never silent (LL-05's law).
    needs(&gdl, "triage_tier: Option<String>", "KT-6 the tier field");
    needs(&gdl, "with_triage_tier(", "KT-6 the arm wiring");
    needs(&gdl, "gdl+triage=", "KT-6 the tier in the hashed model arg");
}

#[test]
fn kt6_scan_control_planted_annotation_is_seen() {
    needs("gdl+triage=qwen3.5-2b-4bit", "gdl+triage=", "KT-6 control");
}

// ── KT-8 (Arm C): the escalation path is SACRED — it stays alive ───────

/// The ceiling-declaration posture (Arm C) rests on ONE invariant: the
/// machine's honest ROUTE — the escalation-until-entitled default — is
/// reachable on an honest empty table. If the arms A/B/D cannot clear the
/// wall, THIS is the posture that remains, and it must be impossible to
/// have closed it while adding the cures. The BEHAVIORAL witness lives in
/// `gdl.rs`'s tests module (an integration binary cannot drive the
/// machine); this structural pin holds the refusal-law shape: the intake
/// gate's empty-cell refusals (the wall itself) and the machine's
/// route-at-intake law (the existing eval fixtures' names) are both still
/// in the tree, un-amputated.
#[test]
fn kt8_the_escalation_route_survives_the_round() {
    let gdl = common::code_only(&doc("src/workflow/gdl.rs"));
    // The wall (the empty-cell refusals) — the SAME bytes the pre-round
    // tree refused with; the cures never softened it.
    needs(
        &gdl,
        "is_not.{name}.is is empty — an unbounded problem statement",
        "KT-8 the wall stands",
    );
    // The machine's honest route: gate exhaustion ROUTES (the escalation
    // the arms make rarer, never impossible).
    needs(
        &gdl,
        "gate exhausted after",
        "KT-8 the route-at-exhaustion law",
    );
}

/// KT-8's control: a scanner that cannot see the wall would pass on its
/// removal — the planted control proves the scan discriminates.
#[test]
fn kt8_scan_control_planted_wall_is_seen() {
    needs(
        "is_not.{name}.is is empty — an unbounded problem statement",
        "unbounded problem statement",
        "KT-8 control",
    );
}
