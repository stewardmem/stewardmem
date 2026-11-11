# KT TIER — PREREGISTRATION CONFIRMATION + T-0 MEASURES (2026-11-11)

**Round:** IMPROVE 14 · TRIAGE KT TIER — the wall between intake-usable and
tuple-producing, cleared by measurement.
**Preregistration:** `plans/improvements/14_TRIAGE_KT_TIER.md` §3 (FROZEN)
+ the §0.1 ground corrections in
`plans/improvements/EXECUTION_PROMPT_14_TRIAGE_KT_TIER_2026-11-11.md`
(the corrections supersede the plan's §0 where they conflict).
**This file is the git-order proof:** it is committed BEFORE the first live
run of the round. Every live number lands in
`docs/KT_TIER_EVIDENCE_2026-11-11.md` (T-f), under the rule below,
applied AS WRITTEN.

---

## 1 · The decision rule, confirmed FROZEN (plan §3, verbatim sense)

- **Per arm, measured:** (M1) first-attempt `intake_gate` KT pass rate —
  the pilot's A4 metric; (M2) escalations; (M3) **fabrication rate** —
  non-`unknown` bounds failing the traceability match, **0 required**;
  (M4) tuples flowed (resolves reached); (M5) wall-clock per case (the
  120 s rail — `AFM_EXCHANGE_DEADLINE`, `provider_afm.rs:61` — holds).
- **The winner:** the LOWEST-complexity arm with **M1 ≥ 70 %**, **M3 = 0**,
  **M4 > 0**, **M5 within the rail**. Preference order on ties:
  **B > A > D > C**.
- **If NO arm clears:** the wall is DECLARED a capability ceiling — the
  honest posture is escalation-until-entitled, the register records the
  ceiling with all FOUR arms' numbers. **A fabricated-bounds arm that
  passes M1 is a LOSER, not a winner — M3 is the veto.**
- Deviations after the runs are NAMED preregistered acts, never smoothing.
- **The corpus is frozen at this commit** (§2 below). The executing
  session confirms: no case is added, removed, edited, or re-probed
  between this commit and the runs.

## 2 · The corpus, enumerated and pinned (2026-11-11, at `de6c25aa`)

**The honest composition: 19 cases — 12 in-tree structural seeds + 7
gold-pack cases (the private side).** The plan §3 line "the same 19 cases
+ the gold pack (20)" is an arithmetic slip: the 7-case gold pack IS the
pack; 12 + 7 = 19. The executing session records the enumeration AS
MEASURED below and does not add a phantom twentieth case (a fabricated
case would fail the honesty law before any model ran).

- **The 12 structural seeds** — in-tree, `src/workflow/gdl.rs`
  `eval_run1::seeds()` (via `eval_fixtures::structural_seeds()`,
  `gdl.rs:14482`), file hash at pin time
  `bce2a6ab745ead406c15cb83042ea02023cca989` (`git hash-object`):
  `happy-battery` ("rebuild is slow") · `happy-thermal` ("node power
  cycling") · `happy-network` ("uplink flapping") · `happy-firmware`
  ("vault latency") · `happy-config` ("spine alarms") · `happy-memory`
  ("mem errors in SEL") — `must_resolve = true` · `fail-intake-missing-isnot`
  ("server is slow") · `fail-probably-fixed-verify` ("cache degraded") ·
  `fail-skipped-negative-check` ("packet loss") · `fail-two-variable-act`
  ("latency spikes") · `fail-off-playbook-action` ("alarms on spine") ·
  `fail-no-coverage-accept` ("mem errors") — `must_resolve = false`.
- **The 7 gold-pack cases** — `conformance/gdl_gold_pack_v1.jsonl`
  (PRIVATE side, brain-steward-ip; loaded by `gdl_eval.rs:1736` and
  `gdl_conformance_pack_run` when the sibling exists):
  `happy_path_resolves` (expected resolved) ·
  `intake_is_not_missing_routes` (routed @ intake) ·
  `act_unsupported_evidence_routes` (routed @ act) ·
  `verify_negative_check_skipped_routes` (routed @ verify) ·
  `verify_fail_hands_to_human` (verify_failed) ·
  `triage_defer_escalates` (escalated @ triage) ·
  `triage_unknown_escalation` (escalated @ triage).
  **SHA-256 (pinned):**
  `34370009421627bb08d9fec89b2fa6a821094e918137a3ca29a94954d6a44980`.
- **The ambiguity register** — `gdl_ambiguity_register_v1.jsonl` (private
  side): `triage_unknown_escalation` is the one registered-ambiguous case;
  the harness scores it `excluded_ambiguous` per its own two-door rule.
  The frozen rule's arithmetic therefore runs over the population the
  harness actually runs: **19 inputs, 18 scored + 1 excluded-ambiguous**,
  recorded per case either way.
- All 19 run by TICKET through the REAL bridge child via
  `pilot_live::run_live_case` (`gdl_eval.rs`) — the pilot's own
  population harness, env-gated `BRAIN_PILOT_LIVE=1`. No new measurement
  road.

## 3 · T-0 measures (recorded 2026-11-11, BEFORE the arms landed)

### 3.1 — The pinned-checkout MLXGuidedGeneration probe → **PRESENT**

- Both `Package.resolved` files (shell-macos, shell-macos-mlx) pin
  mlx-swift-lm **3.32.3 @ `3b339ad6`** (the round-10 ENGINE PIN, exact);
  the checkout `shell-macos/.build/checkouts/mlx-swift-lm` is AT the pin
  revision, `git describe --tags` → `3.32.3` clean.
- The target **exists in the pinned checkout**:
  `Libraries/MLXGuidedGeneration/` (GuidedGenerationLoop.swift,
  XGrammarBridge.swift, MaskSnapshot.swift, …), declared in the
  checkout's Package.swift (:38-39, :251-257).
- **The trait caveat, read from the manifest:** the
  `FoundationModelsIntegration` trait conditions only **MLXFoundationModels'**
  dependency on MLXGuidedGeneration ("linked only when
  FoundationModelsIntegration is enabled, since the adapter references the
  engine exclusively inside that gate"). The engine itself is
  **standalone** — "depends only on CXGrammar + MLXLMCommon + MLX, with no
  FoundationModels coupling and no @available floor beyond the package's
  macOS 14 / iOS 17 minimum". A consumer target that adds the
  `MLXGuidedGeneration` product compiles against it on this graph.
- The API surface, read from the pin (T-0 receipts
  `/tmp/i14-probe/t0_guided_api*.txt`):
  `GrammarConstraint.init(tokenizer:jsonSchema:fastForward:hostTokenizer:)`
  compiles a JSON-schema STRING into a grammar matcher
  (XGrammarBridge.swift:239-243);
  `GuidedGenerationLoop.run(input:context:constraint:maxTokens:vocabSize:…)`
  runs constrained decoding on any MLX model context;
  `GenerationComponents.logitProcessorFactory` (MLXLMCommon/
  GenerationComponents.swift:43) rides grammar-constrained decoding into
  the parameters-driven APIs.
- **Verdict recorded for T-d:** the pinned checkout ALLOWS the guided
  shape — Arm D MAY run the traceability gate WITH the schema via
  `GrammarConstraint`/`logitProcessorFactory` (pre-warmed off the main
  actor; the cold grammar compile can block hundreds of ms), or BARE +
  Arm B's scaffold. The arms actually run are decided by what the build
  admits, and the evidence file records which.

### 3.2 — The AFM capability re-measure (the existing suites, hermetic)

**Fresh-walk correction (recorded, not smoothed):** the execution prompt's
§0 comment names `swift test --filter CapabilityPins` — **no such test id
exists in this tree.** The AFM capability pins live in
`AppleFMProbeTests` (the on-device window/budget pins), the brain-afm
bridge protocol suite, and `ChatSessionRoutingPins`. The re-measure rode
the REAL suites: `swift test` in shell-macos, 2026-11-11.

- **40 tests, 39 passed, 1 failed** — the failure is
  `historyPressureTrimsAndNeverRewrites` ("May contain unsafe content",
  AppleFMProbeTests.swift:817): an Apple-framework SAFETY REFUSAL inside
  a LIVE-model test, on the UNTOUCHED tree at `de6c25aa`, before any
  round change. The same test **passes in isolation** (39.6 s). Recorded
  as a pre-existing live-model flake under parallel suite execution —
  the T-f verify lane re-runs and records the same comparison honestly.
- The capability facts re-confirmed live in-suite: window 4,096 combined;
  the measured conversation budget **2,710** today (the suite's own
  `ContextBudget.measureOnDeviceDefaults()` — measured, not assumed; the
  number moves per machine/day and the pins read it live); PCC refused
  (M1) on this machine — the 24k+ prompt probe correctly did not run;
  greedy sampling pinned on every GenerationOptions site (the pin held).
- The escalation posture re-confirmed: `escalationPermitted=false`, the
  deep tier refused in this build — the escalation path's sacredness is
  a ROUTE law, not a PCC dependency.

### 3.3 — The seven-tier enumeration (CG-11's standing numbers; never re-probed)

The roster (`ChatModelLayer.swift:72-152`): **`afm3-core` + SEVEN MLX
tiers** — `qwen3.5-9b-4bit` · `qwen3-8b-4bit` · `qwen3.5-2b-4bit` ·
`qwen3.5-9b-heretic-5bit` · `qwen3.5-4b-uncensored-4bit` ·
`qwen3.5-9b-heretic-4bit` · `qwen3.5-4b-huihui-4bit`. The `model_excluded`
arm is VACUOUS since CG-11 (the re-arm law stands, `main.swift:32-36`).
Standing numbers, CG-11/round-10 MEASURED (M1 Pro 16 GB —
`docs/UPSTREAM_TRACKING.md` §the honest baseline):

| tier | decode tok/s | energy | prefill-class tok/s (round 10) |
|---|---|---|---|
| qwen3.5-2b-4bit | 43.6 | 1.8 | 669 |
| qwen3.5-4b (uncensored/huihui) | 41–43.7 | 2.5–2.8 | 279 |
| qwen3.5-9b-4bit | 26–28.7 | 5.9–6.2 | — |
| qwen3.5-9b-heretic (5bit/4bit) | 24–26 | 6.2 | 137 |
| qwen3-8b-4bit | 28.5 | 4.6 | 120 |
| afm3-core | the AFM window (4,096; measured budget 2,710 today) | — | the propose tier to date |

Round-10's KV fact binds M5 (§0.1.5): the Qwen3.5 tiers re-prefill the
fence every ask (cached=0); only the 8B shows true prefix reuse
(cached=227). The triage tier is picked BY this table at T-d; no tier is
re-probed for this round.

## 4 · The gating laws, restated (they shape every step)

(a) **THE HONESTY LAW IS THE VETO** — M3 = 0 on whichever arm wins; a
non-`unknown` bound that fails containment-match against the case's own
inputs refuses BY NAME; a fabricated-bounds arm that passes M1 is a
LOSER. (b) **THE PREREGISTERED RULE IS FROZEN** — §1 above, applied AS
WRITTEN. (c) **THE ESCALATION PATH IS SACRED** — the round makes it
rarer, never impossible; KT-8 proves it fires on an honest empty table.
(d) **RED-FIRST, NON-VACUOUS** — every pin born red-first against the
pre-step tree, transcript recorded; every scan carries a planted-miss
control. (e) **THE M-4 QUALIFICATION IS SURGICAL** — only the
`model: None` arm qualifies, triage-exchanges-only; fresh `gdl-`
sessions, no transcript seeding, and the non-triage red-proof stand; the
policy-identity law honored LOUDLY (`bind_policy`/`policy_hash`,
`run_loop.rs:431`/:1699 — the triage-tier mapping lands in the hashed
table, the hash moves, the re-baseline is the measurement, never silent).

## 5 · The one sentence

**The empty table was the model telling the truth — this round hands it a
schema it can fill at runtime, a scaffold it can confirm, a local tier
that finally speaks guided generation, and a frozen rule that picks the
cure whose bounds the data can prove, because a loop that learns is
starved until its intake stops refusing honest tickets.**
