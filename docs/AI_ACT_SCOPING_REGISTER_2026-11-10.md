# AI Act Scoping Register — 2026-11-10

**What this file is.** The operator's scoping decision for Regulation (EU)
2024/1689 (the EU AI Act) as applied to StewardMem — the kernel, the shell,
and the operator service (`com.brain.server`, `scripts/install-service.sh:38`)
— recorded as a sourced decision document, not a conclusion copied from a
blog. The decision itself was preregistered by the operator in
`plans/improvements/12_AI_ACT_GAP_ANALYSIS.md` §1.1 (the plans repo); this
register is the tree-side record of it with the reasoning and the sources
per row. Method: ComplianceAtlas (arXiv 2611.03121) — taxonomy-driven gap
analysis with evidence collection, adapted to the house register discipline.

**Laws this register runs under.** The operator decides scope and the round
records it — no pin, deadline row, or code assertion may claim a scope this
register did not decide. The register is **append-only**: a correction
appends a dated row, it never rewrites one in place (the THREAT_MODEL stamp
discipline).

**The calendar this register runs on (the Digital Omnibus correction,
verified live 2026-11-10).** The Digital Omnibus on AI (provisional
agreement 2026-05-07, Council final approval 2026-06-29; amending
regulation (EU) 2026/1744 — the instrument `src/reg_watch.rs` already pins
at `AI_ACT_ART50_MARKING`) rewrote the phase-in. The dates below are the
ones every other artifact in this tree must carry:

| Date | What applies | Live on 2026-11-10? | Source |
|---|---|---|---|
| 2025-08-02 | GPAI obligations applied | yes (passed) | EU AI Act Service Desk timeline (probed 200) |
| 2026-02-02 | Art 4 AI literacy applies | yes (passed) | COMPLIANCE.md §6.6.1; docs/AI_LITERACY.md |
| 2026-08-02 | **General application incl. Art 50 transparency — enforcement begins** | **yes — the live duty** | Service Desk timeline (probed 200); `src/reg_watch.rs` `AI_ACT_APPLICATION` |
| 2026-12-02 | Art 50(2) transitional end for synthetic-content systems placed on the market before 2026-08-02 | pending | reg (EU) 2026/1744 Art 111(4); `src/reg_watch.rs` `AI_ACT_ART50_MARKING` |
| 2027-12-02 | Annex III high-risk obligations (the Omnibus DEFERRAL) | pending — the re-scope horizon | Service Desk timeline (probed 200); docs/compliance.md (the L7-04 docs stamp) |
| 2028-08-02 | Annex I embedded high-risk obligations | pending | Service Desk timeline (probed 200); docs/compliance.md |

A register pinned to stale dates is the exact fiction this lane refuses;
the pre-Omnibus line ("GPAI 2026-08-02; high-risk phases later") was wrong
twice over and is corrected here.

---

## Decision 1 — Annex III high-risk: **NO for the component as released** (the 2027-12-02 re-scope horizon recorded)

**Decision.** StewardMem is not placed on the market as a high-risk AI
system under Annex III. The determination is recorded, not merely asserted,
and the deferred 2027-12-02 date is recorded so a future re-scope inherits
the real calendar.

**Reasoning.** (1) The component's only generation lane is decision-SUPPORT
with a human escalation law of its own: GDL-drafted output is human-gated —
a proposal a person approves before anything is stored or published
(`src/handlers/workflow.rs` the proposal gate; docs/compliance.md
"Generation, stated conditionally") — and the routed case law names the
terminal "a human's case". A component that surfaces evidence for a human
decision-maker is not itself the Annex III decision-making system; the
deployer who builds THAT system owes the Annex III file, and the component
supplies evidence for it (the audit chain, the logging posture, the DSAR
console — the obligation register maps them). (2) The provider-vs-deployer
distinction is the load-bearing one here (role-aware assessment:
Transparent Information Flow Modeling, arXiv 2610.16916): as released, the
component holds no deployer role over anyone's high-risk system. (3) The
Art 2(12) free-and-open-source posture (COMPLIANCE.md §6.6.1, added
2026-10-25, Apache-2.0) places the released component outside the
Regulation unless placed on the market as high-risk or as an Art 5/Art 50
system, with the recital-103 commercialisation trigger (a paying customer,
paid services, or personal-data use beyond security/compatibility) already
standing as the named re-scope trigger — Decision 3 records the Art 50 arm
of that boundary.

**Sources.** Annex III + Art 6, EUR-Lex consolidated text
(https://eur-lex.europa.eu/eli/reg/2024/1689/oj, probed 202); the Service
Desk timeline (https://ai-act-service-desk.ec.europa.eu/en/ai-act/timeline/timeline-implementation-eu-ai-act,
probed 200 — the 2027-12-02 deferral); COMPLIANCE.md §6.6.1; arXiv 2610.16916;
docs/compliance.md (the deployer-horizons stamp, verified against this
register's calendar).

## Decision 2 — GPAI-adjacent: **NO**

**Decision.** The component is not a general-purpose AI model, not a provider
of one, and carries no GPAI obligation.

**Reasoning.** The component serves models — local and open-weight ones the
operator chooses (digest-pinned model registry; the chat tier label states
the provenance per answer: "generated on-device ({tier})",
`shell/src/lib/i18n/en.json:418`). It trains nothing, ships no weights as
its own product, and makes no model available on the market. The GPAI
chapter (Art 51-55, applied 2025-08-02) attaches to model providers; an
inference host for operator-chosen open weights is not one.

**Sources.** Art 3(63) GPAI definition, EUR-Lex (probed 202);
`shell/src/lib/i18n/en.json:418`; the Service Desk timeline (the 2025-08-02
GPAI date — already passed, and not ours either way).

## Decision 3 — Transparency duties (Art 50, with Art 4 literacy): **YES — the live duty**

**Decision.** The transparency duties are IN FORCE (2026-08-02 passed three
months ago). The round treats them as a compliance arm, not readiness prep:
the kernel's Art 50 evidence already exists (below), and the one named gap —
the shell's plain-language Art 50(1) AI-interaction statement — is filled
this round (S4) and recorded in the obligation register.

**Reasoning.** (1) The kernel-level surfaces already disclose machine
origin: `/.well-known/ai-notice` (machine-readable Art 50 notice,
`src/handlers/well_known.rs:74`, public/no-auth) and the Art 50(2)
machine-readable marking on every engine-generated artifact class
(`src/provenance.rs` — the signed AIGEN|HUMAN object, wired on all four
classes, pinned by `ai_act_art50_marking_deliverable` in
`src/reg_watch.rs`). (2) What the tree did NOT have is the PRODUCT-level
plain-language statement: the shell's chat page labels each answer
("generated on-device", the tier/support vocabulary
`shell/src/lib/i18n/en.json:418`, :429-434) but never states, in plain
language, that the person is interacting with an AI system — the Art 50(1)
disclosure. (3) **The §6.6.1 tension, named honestly:** COMPLIANCE.md §6.6.1
(2026-10-27) records the released KERNEL as not an Art 50 system (FOSS,
no direct interaction — "the shell's chat is /recall retrieval with
provenance"). That analysis predates the on-device MLX chat tier as shipped
surface: the shell DOES run a local generative model (the tier label says
so per answer), and that fact is what moves the product's chat beyond
retrieval-with-provenance. This register records the product-level
completion of the §6.6.1 posture; §6.6.1 itself is NOT rewritten here
(the append-only law — the kernel-level analysis remains true of the
kernel as released).

**Sources.** Art 50, EUR-Lex (probed 202); the Service Desk timeline
(probed 200 — general application 2026-08-02, the Art 50(2) transitional
2026-12-02); `src/reg_watch.rs` `AI_ACT_APPLICATION` + `AI_ACT_ART50_MARKING`
(the tree's own clock, with the Art 111(4) operative-provision provenance);
`src/handlers/well_known.rs:74`; `shell/src/lib/i18n/en.json:418`.

## Row 4 — AI literacy (Art 4, live since 2025-02-02): satisfied, recorded

**Decision.** The Art 4 duty is in force and satisfied by artifacts that
already exist; this row records them so the live-duty picture is complete.

**Reasoning + artifacts.** docs/AI_LITERACY.md is the mechanism-level
deployer playbook (what the component is, why it is inspectable, how an
operator demonstrates literacy); `/.well-known/ai-literacy`
(`src/handlers/well_known.rs:91`) makes it discoverable beside the Art 50
notice and enumerates the inspectable controls (recall trace, proposal
gate, quarantine, audit chain, DSAR console).

**Sources.** Art 4, EUR-Lex (probed 202); docs/AI_LITERACY.md;
`src/handlers/well_known.rs:91`.

## Row 5 — PQC migration decision point (recorded; a read, not a new row)

**Decision.** The post-quantum migration decision for the release-signing
chain is scheduled to the post-2030 federal transition windows. This round
RECORDS the decision point; it lands no new threat-model row and no code.

**Reasoning + artifacts.** Plan 03 already landed the tracked row — verified
live in the tree today: THREAT_MODEL.md:295-306 (`post-quantum: tracked`,
R152, 2026-11-07: the updater's minisign/Ed25519 classical lane,
`updater_pins.rs`, `scripts/release-sign.sh:2` signs detached minisig).
`src/reg_watch.rs` carries the horizon (`PQC_INVENTORY_SEAM` = 2030-12-31,
NIST IR 8547 / OMB M-26-15 / CNSA 2.0) and the census that keeps the
inventory honest (`crypto_inventory_census_maps_every_crypto_crate`, :463);
docs/crypto-inventory.md is the SP 1800-38B-shaped inventory with the two
agility seams. The DECISION (which PQC signature scheme replaces
minisign/Ed25519 in the updater chain, and when) is taken against the
post-2030 federal windows per the NIST transition guidance.

**Sources.** https://www.nist.gov/news-events/news/2026/10/nist-report-provides-guidelines-transition-post-quantum-cryptography
(probed 200, 2026-11-10); the CISA vendor questions
(https://www.cisa.gov/resources-tools/resources/post-quantum-questions-ask-your-vendors,
re-confirmed live 2026-11-05 via the research sweep); THREAT_MODEL.md:295-306;
`src/reg_watch.rs:115`, :463; docs/crypto-inventory.md.

## Row 6 — Derived-artifact residue: the serving-layer KV cache (recorded, not built)

**Decision.** The serving-layer KV-cache persistence of the on-device
models is recorded as a deployment-surface gap outside every kernel erasure
door. Disposition: a doctor-card candidate (detect and disclose KV-cache
persistence beside the store) + this register row. Nothing is built this
round.

**Reasoning.** The October erasure wave's literature names the class live:
SARIA (arXiv 2610.31169) and Gandalf (arXiv 2610.30791) persist LLM KV
caches — conversation content — to flash storage; Phantom Forgetting
(arXiv 2610.34800) shows hidden state FAKING deletion; The Tokens Remember
(arXiv 2609.29045) names the tokenization-bypass class. The kernel covers
its own stores at forget (rows + embeddings + FTS + tombstone +
certificate: `src/handlers/forget.rs:34`, the DSAR chain
`src/server/router/compliance.rs:39-46`). The SERVING-layer caches — the
mode-2 local models' context caches on the same machine — belong to the
DEPLOYMENT's surface and are outside those doors today. The honest
posture is disclosure-first (the doctor card), not a silent assumption
that the DB purge is the whole story.

**Sources.** arXiv 2610.31169, 2610.30791, 2610.34800, 2609.29045 (all four
verified live via the export API 2026-11-10 — transcripts in the evidence);
`src/handlers/forget.rs:34`; `src/server/router/compliance.rs:39-46`.

---

## The literature this register cross-examines against

The scoping reasoning is written to survive the measurement the
expert-curated Q&A instrument represents (regenold's 2026 EU AI Act Q&A
Challenge, arXiv 2611.04546: an LLM's unaided Act answers are
measurable-unreliable — which is exactly why every row above cites its
source instead of asserting). The taxonomy siblings consulted: VERA/COMPL-AI
(2610.31588 — the 18-requirement mapping, the obligation register's shape),
AIRO/VAIR + DPV (2610.31380 — the risk-ontology vocabulary), TESSA
(2610.30822 — the SME guidance shape; this project IS the SME), PIA/DPIA/FRIA
state of practice (2610.38543 — the operational context for the
owner/date columns), and Conformance as a Distribution (2610.33112 — the
honest-posture note this register adopts: conformance is a failure rate
under a declared distribution, never a score).

## Probe receipts (2026-11-10, live)

The five anchors fetched via the export API (2611.03121, 2610.31169,
2610.30791, 2610.34800, 2609.29045 — 5 entries, 14,570 bytes); the Service
Desk timeline probed **200**; EUR-Lex consolidated text probed **202**; the
NIST PQC transition report probed **200**. Transcripts:
`/tmp/i12-probe/` (static block, i18n parity, live probes) + the plans
repo's `IMPROVE12_EVIDENCE_2026-11-10.md`.
