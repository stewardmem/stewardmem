# AI Act Obligation → Evidence Register — 2026-11-10

**What this file is.** The obligation→evidence map for Regulation (EU)
2024/1689 as scoped by [`AI_ACT_SCOPING_REGISTER_2026-11-10.md`](AI_ACT_SCOPING_REGISTER_2026-11-10.md)
(the operator's decision: NOT high-risk under Annex III; not GPAI-adjacent;
transparency-duty-only under Art 50, which is IN FORCE since 2026-08-02).
ComplianceAtlas-shaped (arXiv 2611.03121), house format: every applicable
obligation (article cite) → the tree artifact that satisfies it
(`file:line`) — or the named gap with an owner and a date. **Zero uncited
rows** is a pinned law: `tests/ai_act_pins.rs` scans this register and
fails any obligation row that carries neither an artifact cite nor
`gap:` + `owner:` + `date:`.

**Register discipline.** Append-only: a correction or a closure appends a
dated row (see the S4 closure appendix); nothing here is rewritten in
place. Conformance posture per Conformance as a Distribution (arXiv
2610.33112): the rows below are evidence cites, not a score — conformance
is a failure rate under a declared distribution, never a badge.

## The applicable obligations

| Article | Obligation (plain) | Artifact that satisfies it | Status | Source for the obligation |
|---|---|---|---|---|
| Art 4 | AI literacy: the people operating the system understand what it is and can read its decisions | `docs/AI_LITERACY.md` + `src/handlers/well_known.rs:91` (`/.well-known/ai-literacy`, the machine-readable pointer naming the five inspectable controls) | satisfied | EUR-Lex reg (EU) 2024/1689 Art 4; live since 2025-02-02 |
| Art 12 (+ Art 26(6) deployer posture) | logs the system's operation in a tamper-evident form a deployer can inspect | `src/audit/mod.rs:18` (the keyed hash chain — prev_hash link, keyed HMAC-SHA256 per-DB epoch) + `docs/compliance.md:66` (the Art 12 posture section) + `src/handlers/compliance.rs:29` (`export_audit`, the operator's export door) | satisfied | EUR-Lex Art 12; docs/compliance.md §Logging |
| Art 50(1) | the person interacting with an AI system is told so, in plain language | — | `gap:` the shell chat page labels each answer (tier + support vocabulary) but carries no plain-language AI-interaction statement; the kernel's service-level notice exists (`src/handlers/well_known.rs:74`) but the chat surface itself has none | `owner:` operator (mark) `date:` 2026-11-10 — filled at S4 this round; the closure cite lands in the appendix below |
| Art 50(2) | machine-readable marking that engine-generated artifacts are AI-generated | `src/provenance.rs:44` (`MARK_AIGEN`)/:47 (`MARK_HUMAN`)/:91 (`attach_aigen`)/:150 (`verify_artifact`) — wired on all four engine-generated artifact classes, pinned by `ai_act_art50_marking_deliverable` in `src/reg_watch.rs` | satisfied; the pre-2026-08-02 transitional horizon is pinned at 2026-12-02 (`src/reg_watch.rs` `AI_ACT_ART50_MARKING`, reg (EU) 2026/1744 Art 111(4)) | EUR-Lex Art 50(2); the Service Desk timeline (probed 200) |
| Art 50 (service-level notice) | machine-readable disclosure that the service stores, retrieves, and may return AI-generated/processed content | `src/handlers/well_known.rs:74` (`/.well-known/ai-notice`, public/no-auth, version-tagged) | satisfied | EUR-Lex Art 50; docs/compliance.md (the three public notices, one truth — COMPLIANCE.md §6.6) |
| Art 95 | voluntary code-of-conduct marker for a non-high-risk system | `src/handlers/well_known.rs:109` (`/.well-known/cop-notice`, self-attested posture, declarative only — Article 95 codes of conduct, not Article 56 codes of practice, per T4-04) | satisfied | EUR-Lex Art 95 |
| Annex III high-risk (2027-12-02 horizon) | the high-risk file a deployer of an Annex III system owes | — | not applicable per the scoping register Decision 1 — recorded, with the deferred date, for any future re-scope: `docs/AI_ACT_SCOPING_REGISTER_2026-11-10.md` (Decision 1) | `owner:` operator (mark) `date:` 2027-12-02 (the Omnibus-deferred re-scope horizon) |
| GPAI (Art 51-55, applied 2025-08-02) | model-provider obligations | — | not applicable per the scoping register Decision 2: `docs/AI_ACT_SCOPING_REGISTER_2026-11-10.md` (Decision 2 — serves operator-chosen local/open-weight models; no weights of its own on any market) | `owner:` operator (mark) `date:` 2026-11-10 (recorded; the date passed 2025-08-02 and never applied) |
| Serving-layer KV-cache residue (Art 10 data-governance analogue for any future high-risk file) | erasure claims that cover the whole pipeline, not just the DB | — | `gap:` the mode-2 on-device models' serving-layer KV caches persist conversation content outside every kernel erasure door (the kernel's own rows/embeddings/FTS are covered); disposition: doctor-card candidate (detect and disclose beside the store) — recorded, not built | `owner:` operator (mark) `date:` 2027-12-02 (rides the Annex III re-scope horizon); sources arXiv 2610.31169, 2610.30791, 2610.34800, 2609.29045; scoping register Row 6 |

## The evidence base a 2027-12-02 re-scope would map

The scoping decision says the component is not high-risk — these rows are
NOT obligation claims. They are the existing collectors the re-scope
(the deferred horizon) would map first, listed with their cites so the
register's zero-uncited discipline covers them too:

- **Evaluation records** (Art 15-analogue: accuracy posture): `src/handlers/compliance.rs:177` (`post_evaluation_record`).
- **ROPA** (the GDPR sibling a high-risk file reuses): `src/handlers/compliance.rs:335` (`list_ropa`), :362/:370 (`create_ropa`/`upsert_ropa` → `ropa_upsert_tx`), the inventory roll-up at :242 with the `gdpr_ropa` artifact row at :298.
- **DSAR / verifiable deletion** (Art 17 posture): `src/server/router/compliance.rs:39` (POST), :42 (GET ledger), :44 (`/dsar/{id}/certificate`); the handlers `src/handlers/observe.rs:180` (post_dsar), :627 (list_dsar), :692 (get_dsar_certificate); `src/handlers/forget.rs:34` (forget).
- **HITL evidence** (the human-oversight record): `src/handlers/mod.rs:583` (the `BRAIN_WRITE_POSTURE` law — agent writes route through the human promotion gate under `review`), the decision-event ledger `hitl_decision_events` (v1.32.33, written in the act's own tx — round 11) + `tests/hitl_pins.rs`.
- **Per-turn transparency vocabulary** (the Art 50(1) RAW MATERIAL — provenance per answer, not interaction disclosure): `shell/src/lib/i18n/en.json:418` (`chat.tier`) and :429-433 (the support vocabulary: grounded / partly / not grounded / abstained / degraded), consumed at `shell/src/routes/+page.svelte:1046`.
- **PQC tracked row** (the supply-chain/updater evidence): `THREAT_MODEL.md:295-306` (`post-quantum: tracked`, R152), `src/reg_watch.rs:115` (`PQC_INVENTORY_SEAM` 2030-12-31), :463 (the crypto census), docs/crypto-inventory.md, `scripts/release-sign.sh:2` (minisign detached sigs).
- **The MAS privacy manifest**: `shell/src-tauri/privacy/PrivacyInfo.xcprivacy` (NSPrivacyTracking false + empty collected-data types; the required-reason closure measured to FileTimestamp + DiskSpace) — REVIEWED at S4; the verdict lands in the appendix.

## Appendix — dated closures (append-only)

*(none yet — S4 appends the transparency closure and the privacy-manifest
review verdict here, dated 2026-11-10)*
