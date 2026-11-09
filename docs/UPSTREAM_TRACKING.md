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

## The table

| item | upstream ref | our evidence | status | review-date |
|---|---|---|---|---|
| mlx-swift-lm version posture — ON the newest tag; main accumulates | ml-explore/mlx-swift-lm tag `3.32.3` = our pin `3b339ad6` (2026-09-30); main HEAD `4ab26799` (2026-11-06) is **87 commits** past it, behind_by 0 (live-probed 2026-11-09: tags API + compare) | `docs/IMPROVE09_EVIDENCE_2026-11-09.md` §1–§2; the harness green on the current pin (swift suite 38/38 hermetic AND 38/38 live); standing numbers in the checklist below | tracking | 2026-11-09 |

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
