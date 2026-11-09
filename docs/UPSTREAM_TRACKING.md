# UPSTREAM TRACKING — the one home

**Law (plan 09):** upstream tracking is the posture of a tenant that measures
before it moves. This table owns every upstream fact with a date; the harness
proves the floor is green today; the engagement offers our data instead of
demanding their time; and the only act that ever changes a pin is a measured
bump with its receipts.

- **Closed status vocabulary:** `tracking | proposed | landed | declined+reason`
  (a `declined` row MUST carry its reason; `proposed` carries its annotation,
  e.g. `proposed — draft filed by the operator`). The vocabulary is pinned by
  `tests/upstream_tracking_pins.rs` (UT-3) — a row without a `status` or a
  `review-date` reds the battery.
- **Review discipline:** every row carries the date of its last live re-check.
  A stale row is re-probed before it is believed.
- **Seed lineage:** the CG-10 corrections (the swift-jinja 1.3.0 slice bug →
  the 3.0.0 floor) and the CG-11 corrections (the tags-API vs releases-page
  misread; the C-5/C-6 findings) are the first rows' ancestors
  (`plans/closing_gaps/CG10_EVIDENCE_2026-11-05.md`,
  `plans/closing_gaps/CG11_EVIDENCE_2026-11-06.md`). The IMPROVE09
  re-verification (2026-11-09) is the second seeding.
- **Filing discipline:** NO upstream filing by the agent. Drafts live in this
  repo (`§D-1`, `§D-2`); filing on GitHub is the operator's outward-facing act.
  `§D-1` is positioned AFTER the bump re-probe (main's #663 may close the
  finding — it files as a confirmation with fresh data or a closed-it report,
  by measurement).

## The table

| item | upstream ref | our evidence | status | review-date |
|---|---|---|---|---|
| mlx-swift-lm version posture — ON the newest tag; main accumulates | ml-explore/mlx-swift-lm tag `3.32.3` = our pin `3b339ad6` (2026-09-30); main HEAD `4ab26799` (2026-11-06) is **87 commits** past it, behind_by 0 (live-probed 2026-11-09: tags API + compare) | `docs/IMPROVE09_EVIDENCE_2026-11-09.md` §1–§2; the harness green on the current pin (swift suite 38/38 hermetic AND 38/38 live); standing numbers in the checklist below | tracking | 2026-11-09 |
| C-6 · the Qwen3.5 primed-think prefix instability — the prefix cache cannot hit | #155 (prompt-cache round-trip, IN our build) does NOT cover it; the hook: main's #663 (reasoning-history reconstruction) + #644 (reused-prefix report) may close it ON THE BUMP | `plans/closing_gaps/CG11_EVIDENCE_2026-11-06.md` §C-6 (the `cachedPromptTokenCount == 0` split, the 8B's `cached=227` contrast); `PrefixCachePins` (green live, the C-6 boundary recorded) | proposed — upstream report after the bump re-probe | 2026-11-09 |
| spec decode on the GDN/hybrid towers — the tower-native MTP question | main LANDED MTP: #545 (Qwen 3.8 text/VLM/MTP), #581 (resumable ChatSession MTP); `MTPSpeculativeTokenIterator.maximumBlockSize(for:)` sliding-window-aware, min block 2 | `SpecDecodePins.swift` (the preregistered ≥40% `specDecodeEvidenced` gate + the seven-tier OFF posture + the drafter-not-loaded law); `plans/closing_gaps/CG11_EVIDENCE_2026-11-06.md` §C-5 (the draft-model refusal: acceptance 25–47% yet tok/s collapse 28.5 → 6.3–9.6, ~1.5–4 GiB drafter cost); the literature set in `§D-2` | proposed — draft filed by the operator | 2026-11-09 |
| swift-jinja floor — the Qwen3.5 multi-turn engine floor | huggingface/swift-jinja 3.0.0 (2026-10-27) newest AND main == our pin `a091d6ae` (identical, ahead_by 0 — live-probed 2026-11-09); 2.5.x = the retired 2.x line, below the floor, irrelevant | `Tests/MLXFMProbeTests/EngineFloorPins.swift` (UT-1: reads `Package.resolved`, pins ≥ 3.0.0 — green in the S1 harness run); `docs/IMPROVE09_EVIDENCE_2026-11-09.md` §4 (the fresh re-check) | tracking | 2026-11-09 |
| swift-transformers — the tokenizer/template surface | 1.4.0 (2026-11-02) newest (live-probed 2026-11-09); our pin 1.4.0 (`402a8a11`) — current, nothing to take | `shell-macos-mlx/Package.resolved`; CG-10's bump receipts (`plans/closing_gaps/CG10_EVIDENCE_2026-11-05.md`) | tracking | 2026-11-09 |
| mlx-swift — the sibling (NEW tracked surface) | 0.33.0 (2026-10-27, `3d066d3a`) is out; our pin 0.32.3 (`19601207`) — NOT an independent bump: it rides the next mlx-swift-lm tag; tracked so the dependency diff is never a surprise (the CG-12 E0 gate shape) | `shell-macos-mlx/Package.resolved`; the bump-validation checklist act 2 in this doc | tracking | 2026-11-09 |
| Tauri (the shell, `shell/src-tauri`) | stable 2.x head **2.12.5** (2026-10-21; live-probed 2026-11-09); the manifest floor `2.12.4` (:35) and the lock ALREADY resolves 2.12.5 (caret took the patch); OSV **ZERO advisories** at 2.12.5; the v3 line is alpha (`3.0.0-alpha.6`, 2026-11-06) — NOT an adoption target; CVE-2025-31477 stays plan-03's lesson | `shell/src-tauri/Cargo.toml:35` + `Cargo.lock`; plan-03's advisory pins (`plans/improvements/03_ADVISORY_PINS_AND_THREAT_MODEL.md`); `docs/IMPROVE09_EVIDENCE_2026-11-09.md` §1 | tracking | 2026-11-09 |
| upstream's own pinning guidance — #668 | "Recommend upToNextMinor in installation instructions" — aligns with our tag-based `from:` pins only in spirit: we pin FLOORS (a caret floor + a resolved lock), not floating ranges; the difference is recorded, no act | `shell-macos-mlx/Package.swift:77` (the `from:` pin); the posture row in this table | tracking | 2026-11-09 |
| the KV ratchet's matrix — **the bump RE-RUNS it** (the engine-pin law's follow-through) | main carries the #717-class fix UNRELEASED (3.32.3 is still the newest tag, live-probed 2026-11-09): `quantizedScaledDotProductAttention` (the #703 fused surface — inline dequantization inside the kernel, the f32 score matrix never materializes), "prefill stays raw fp16, compresses on the first decode step" (`kv-cache-quantization.md`), the verified-common-prefix rewind (may flip the Qwen3.5 tiers' `cached==0` boundary), `KVCacheStatus.memoryBytes` (the cache-scoped memory axis, #639) | the MEASURED wound on our pin: the 8B's turn-2 prefill 2.28–8.15× fp16 across the whole scheme ladder at 16k and 2.82–7.66× at the 32k dominant — all over the preregistered 1.5× gate; the GDN tiers wound-masked (`cached==0` re-witnessed at 16k, P-5's boundary); the 2B's length-onset decode collapse (0.87× @16k → 0.38× @64k); nil re-earned on all tiers — the receipts: `plans/improvements/IMPROVE10_EVIDENCE_2026-11-09.md` + `10_EVIDENCE_RAW/` (61 rows); `docs/REKRA_PLAN_PREREGISTRATION_2026-11-09.md`; plan 18 `plans/improvements/18_KV_BUMP_MATRIX_RERUN.md` (+ its execution prompt) — the bump re-runs the matrix as a named act with the decode clause added | tracking — executes via plan 18 when the next tag lands | 2026-11-09 |

## The bump-validation checklist (the named act for when the next mlx-swift-lm tag lands)

The posture decision (2026-11-09, re-verified live): **stay on tags; adopt the
next tag as a named, measured bump.** `3.32.3` is STILL the newest tag at
execution; the 87 main commits are the accumulated validation debt, itemized
below. Nothing is adopted unmeasured; main-tracking stays an explicit named act
for a specific needed fix. **Any red declines the bump with the named reason —
the measurement-first law cuts both ways.**

Standing numbers (CG-11's own measurements, M1 Pro 16 GB — the honest baseline
any bump re-derives against, carried forward from
`plans/closing_gaps/CG11_EVIDENCE_2026-11-06.md`):
8B 28.5 tok/s turn-2 / ~4.6 GiB · 2B 43.6 / ~1.8 GiB · heretic 9B-5bit 24–26 /
~6.2 GiB · 9B-4bit 26–28.7 / ~5.9–6.2 GiB · the 4B tiers 41–43.7 / ~2.5–2.8 GiB.

The checklist, in order:

1. **The pin move** — `swift package update mlx-swift-lm` in
   `shell-macos-mlx/`; the `Package.resolved` diff named commit-by-commit.
2. **The dependency diff, disclosed** — the sibling rides along: mlx-swift
   `0.32.3` → `0.33.0` (released 2026-10-27, main already carries it); the
   CG-12 E0 gate shape applies (diff disclosed + audit + the battery).
3. **The swift suite** — `swift build --build-tests` → `scripts/setup.sh` →
   `swift test`: 38/38 hermetic, then `BRAIN_FM_MEASURE=1` live (the 0.6B CI
   model + the 9B measure arm). `EngineFloorPins` (swift-jinja ≥ 3.0.0,
   UT-1) must hold through the bump — it is the regression alarm.
4. **The CG-11 multi-turn probe harness re-measures the per-tier tables** —
   `Sources/multi-turn-probe` (`PROBE_KV_SCHEME`, ARM L), tok/s + phys_footprint
   against the standing numbers above.
5. **The C-6 re-probe** — `cachedPromptTokenCount` on the Qwen3.5 tiers'
   turn 2: main's #663 (reasoning-history reconstruction) + #644
   (reused-prefix report) may close the finding. It closes BY MEASUREMENT or
   stands with fresh data (P-5's family re-probes).
6. **The MTP spec-decode measurement against the 40% gate** — main LANDED
   tower-native MTP speculative decoding (#545 Qwen 3.8 text/VLM/MTP; #581
   resumable ChatSession MTP; `MTPSpeculativeTokenIterator.maximumBlockSize(for:)`
   sliding-window-aware, min block 2). The tiers STAY OFF unless
   `SpecDecodePins`' gate (`specDecodeEvidenced`, ≥ 40% acceptance) clears on
   the Qwen3.5 GDN tiers; the measurement table lands either way. CG-11's
   draft-model refusal (acceptance 25–47% yet tok/s collapse 28.5 → 6.3–9.6,
   ~1.5–4 GiB drafter cost) is the class precedent: the gate is
   necessary-not-sufficient, the measured throughput decides.
7. **The streaming end-to-end witness** — the deltas/final path against the
   held-text flush (`4ea4225a`) + tokenizer-provided detokenizers
   (`3f793783`) + reasoning events (`ef36109b`).
8. **The REAL kernel battery** — `tests/chat_mlx_tier.rs` 3/3 under
   `BRAIN_CHAT_MODEL` on the moved pin.

Main-classes checklist extracted from the compare `3b339ad6...main`
(87 commits, live-verified 2026-11-09), by family:

- **GDN:** `c4084528` gated-delta-net q/k L2 eps fix · `128f49f1` MLXFast
  gated delta kernel · `1830ae4f` fused-projection freeing (known).
- **KV/memory:** `282c2f50` KVCacheStatus memory · `52e6001f` model tensor
  buffer memory · `991266ad` per-chunk buffer-cache clears · `6e0a0b28`
  Metal sync before the cached-buffer-memory check.
- **KV-rotation:** `12d935f9` exact prefix rewinds · `0ea44653`/`72a4673e`/
  `2775ddde` mask sizing/pinned prefix.
- **Streaming:** `4ea4225a` held-text flush · `3f793783` tokenizer-provided
  detokenizers · `ef36109b` reasoning events.
- **ChatSession:** `#641`/`7a9b7e66` cache fork · `#608`/`173c163a` mid-turn
  steering · `#714`/`db2e7bf4` request-scoped observations ·
  `#644`/`f3da8683` reused-prefix report.
- **Qwen3.5:** `#737`/`f5b0677f` text geometry · `#761`/`4ab26799` VLM YaRN
  from rope_parameters · `#662`/`7eb77efa` compiled decode ·
  `#642`/`ce3950e3` MTP-weight dropping · `#614`/`3615fe46` unused vocab
  projections at prefill.
- **MTP spec decode (the class change):** `#545`/`4cc55045` Qwen 3.8
  text/VLM/MTP · `#581`/`ffe7647b` resumable ChatSession MTP · `411d8e76`
  feed-the-sampled-token · `260ada02` ScriptedLogitsModel · `c2f10736`
  GrammarConstraint.freshMatcher.
- **Model wave — NOT adoption targets (the roster is fixed at seven MLX
  tiers):** Qwen 3.8 · Nemotron 3.5 (#712; arXiv 2610.38095) · Gemma 4 ·
  LFM2.5/LFM3 · MiniCPM5 · Mellum 2.1 · EmbeddingGemma 2.

## §D-1 · DRAFT — the C-6 upstream report (file ONLY after the bump re-probe)

> Status: `proposed — draft filed by the operator`. The bump re-probe comes
> FIRST (the checklist's act 5): main's #663/#644 may close the finding; this
> draft then files as either a confirmation with fresh data or a
> closed-by-the-fix report. Target repo: ml-explore/mlx-swift-lm.

**Title:** Qwen3.5 chat template: the primed-think render is not
prefix-stable across turns — the native prompt cache cannot hit on
multi-turn (`cachedPromptTokenCount == 0` on turn 2)

**Body (verbatim draft):**

We run a local-first memory server on mlx-swift-lm **3.32.3** (`3b339ad6`,
so #155's `ArraysCache`/`MambaCache`/`CacheList` prompt-cache round-trip IS
in our build), swift-transformers 1.4.0, swift-jinja 3.0.0, macOS 27.2
arm64 / M1 Pro 16 GB.

On every Qwen3.5 tier we serve (the 2B/9B/9B-heretic/4B GDN-family 4-bit
MLX repos), the second turn of a same-session conversation reports the
native split as **`promptTokenCount` = N (all fresh), `cachedPromptTokenCount`
= 0** — the prompt cache never serves the shared prefix, on any KV scheme we
run. The control tier (`Qwen3-8B-4bit`, non-GDN) shows **`cached=227`** on
the same two-turn shape.

As far as we can trace it (the `template-debug` bisect): once the final
assistant turn carries content, the primed-think template renders the
conversation differently turn-over-turn (the reasoning block handling), so
turn 2's rendered token sequence is NOT an extension of turn 1's — the miss
happens upstream of the cache classes, in the rendered prompt bytes. #155's
round-trip fix does not change this (verified at 3.32.3).

We see #663 ("Fix Qwen reasoning history reconstruction") and #644 ("Report
the reused cache prefix before prefill") on main as likely-related. **We are
holding this report until our next tagged bump** and will re-probe the
instrument then: if the render is fixed, this closes with our fresh
measurement attached; if not, it files with fresh data on the new pin.

Our instrument: a multi-turn probe reading the engine's own
`promptTokenCount`/`cachedPromptTokenCount` split per turn (no wire-sum
inference), plus per-tier tok/s and `phys_footprint` tables. Happy to share
the full tables.

## §D-2 · DRAFT — the spec-decode engagement (the MTP question + the measurement offer)

> Status: `proposed — draft filed by the operator`. No bump gating on this
> one — the question stands now that #545/#581 have landed; the operator
> files when ready. Target repo: ml-explore/mlx-swift-lm (discussion or
> issue per the maintainers' preference).

**Title:** Speculative decoding on GDN/hybrid towers: does the new
tower-native MTP path clear a ≥40% acceptance gate on the Qwen3.5 tiers?
(measurement offer + our preregistered harness)

**Body (verbatim draft):**

We run a local-first memory server with a fixed roster of seven MLX tiers
(Qwen3-8B + five Qwen3.5 GDN-family 4-bit/5-bit repos) on M1 Pro 16 GB, and
we preregister our spec-decode posture: **a tier moves OFF → ON only above
40% acceptance** measured off the engine's own
`SpeculativeDecodingTelemetry.acceptanceRate` (pinned in our suite — the
gate, a seven-tier OFF posture, and a drafter-not-loaded law; the pins are
the harness).

What we measured on the **draft-model path** (mlx-swift-lm 3.32.3, 0.6B
drafter beside the 8B target): acceptance **25–47% across arms** — crossing
our gate on some arms — but turn-2 throughput **collapsed 28.5 → 9.6 tok/s**
(cold → 6.7; long-context → 6.3) and `phys_footprint` grew **~1.5–4 GiB**
with the drafter resident. The gate proved necessary-not-sufficient; the
measured throughput decides. That is why our GDN tiers stayed OFF.

The question, now that #545/#581 have landed the **tower-native MTP path**
(`MTPSpeculativeTokenIterator`, sliding-window-aware, min block 2): **does
MTP clear ≥40% acceptance on the Qwen3.5 GDN tiers without the throughput
collapse?** We will measure at our next tagged bump and share the per-tier
table either way — the tiers stay OFF unless BOTH the acceptance gate and
the measured throughput clear.

For context, the 2026 literature we are tracking on this exact class:
TreeWY (2608.20961), Twig (2608.01651), SpecLA (2607.16673), ReaDy
(2610.20503), component-aware self-speculation (2605.01106); HybridSpec
(2611.04055 — verification-length choice for hybrid GDN-MoE targets),
SpecOPD (2611.02846), ReplaySSM (2610.13778), H-Spec (2609.24197); the MTP
class: AngelSpec (2607.25852), Windowed-MTP (2607.21535), DLoop
(2610.07659), HyCo (2610.21436), Long-Context Drafting (2610.20865); and
Nemotron 3.5 (2610.38095) as the MTP reference design behind #712.

Offer: our harness is preregistered and reusable (the 40% gate, the OFF
posture, the per-tier probe tables with tok/s + phys_footprint). If a
tagged release carries MTP for the Qwen3.5 family, we will publish our
table; if the maintainers want a specific workload shape measured, we will
take it.
