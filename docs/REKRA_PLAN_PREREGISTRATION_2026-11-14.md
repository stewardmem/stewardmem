# REKRA PLAN PREREGISTRATION — 2026-11-14 · the KV ratchet's tier-scoped matrix (the MAIN-ADOPTION engine state)

**Status:** committed BEFORE any matrix run — git order IS KR-1. **The entry basis is THE OPERATOR'S NAMED ACT, not the tag gate:** on 2026-11-14 the operator named main-tracking ("take from main and patch as much and as good as possible and in January we could follow the release") — the record is [`plans/improvements/18_KV_MAIN_ADOPTION_2026-11-14.md`](plans repo, commit `c228f15`). The TAG GATE (a newer tag than 3.32.3) remains **UNMET** — this doc does not pretend otherwise; the tags API still returns `3.32.3` as the newest tag on the day of this commit. This engine state is a NAMED MAIN SNAPSHOT standing in for the tag; **when 3.33.0 tags, a new preregistration (lexicographically newer than this one) supersedes it, the pin re-points to the TAG, and B0's validation battery runs per the unchanged bump prompt.**
**What this gates:** the KV-scheme matrix on the main-pin engine state — which runs ONLY if the operator separately names it (the adoption record's §4 measurement law: a main-pin matrix is the measure-twice path; the January tag-era matrix is the measurement of record). The preregistration exists to keep KR-4's pins law green at the new engine state and to make an early matrix lawful if named.
**The laws of the round:** preregistration-first · measurement-first with the preregistered cost escape (rows priced out refuse BY NAME) · the honest-outcome law ("nil stands everywhere" is a VALID verdict, re-earned) · the engine-pin law (every table names the engine) · the weights-pin law (every table names the weights revision) · no kernel `src/` behavior change · zero new dependencies · one commit per step.

## ENGINE PIN

**ENGINE PIN: mlx-swift-lm main-63529075 = 6352907543270ae9c61fcaf59cb3e07b0b989ea1** — live-probed twice 2026-11-14: the GitHub tags API returns **`3.32.3` (`3b339ad6`) as the NEWEST tag** (the tag gate unmet, stated plainly); `main` is **95 commits ahead**, head `6352907543270ae9c61fcaf59cb3e07b0b989ea1` (#790 "Use MLXLogger for the model component warning", 2026-11-11 — frozen three days); `shell-macos-mlx/Package.swift:77` resolves **`revision: "6352907543270ae9c61fcaf59cb3e07b0b989ea1"`** per the named act. Every table this round produces names THIS engine.

**The disclosed dependency diff (actual, from the adoption resolve):**

| Package | Was (3.32.3 tree) | Now (this engine state) |
|---|---|---|
| `mlx-swift-lm` | `3.32.3` / `3b339ad6` | **revision-only / `635290754327…`** (the named act) |
| `mlx-swift` (the sibling) | `0.32.3` / `19601207` | **`0.33.0` / `7ede4fd6`** — the readiness plan's predicted ride, confirmed |
| `swift-huggingface` | floor `0.8.1` | **`0.13.0`** |
| `swift-jinja` | ≥ `3.0.0` floor | **`3.0.0`** — the floor holds (`EngineFloorPins` green) |
| `swift-transformers` | `1.4.0` | `1.4.0` (unchanged) |

Main carries the fixes our pin lacked — **#663** (Qwen reasoning history reconstruction, merged 2026-10-07), **#644** (report the reused cache prefix before prefill, 2026-10-12), **#737** (Qwen 3.5 text geometry read-only, 2026-10-28) — the C-6 hook's materials; and **#717** (the compressed-cache turn-2 prefill wound) is **STILL OPEN upstream — this engine state does NOT carry it**; the turn-2 measurement runs against #717's unresolved state either way.

## WEIGHTS PIN (the complete reproduction recipe — engine pin + weights pin)

Fetched live 2026-11-14 (HF models API, Way 1; identical to the 2026-11-13/14 fetches recorded in the readiness plan §1 — no repo moved):

| Wire tier (PROBE_TIER) | HF repo (PROBE_MODEL_ID) | **revision (pin this)** | lastModified | gated |
|---|---|---|---|---|
| `qwen3-8b-4bit` | `mlx-community/Qwen3-8B-4bit` | `545dc4251c05440727734bcd94334791f6ab0192` | 2025-04-28 | no |
| `qwen3.5-2b-4bit` | `mlx-community/Qwen3.5-2B-4bit` | `674aaa7240b91e8012fcad5d791b7dfe5ba90207` | 2026-03-02 | no |
| `qwen3.5-4b-uncensored-4bit` | `avan-ag/Qwen3.5-4B-Uncensored-MLX-4bit` | `fb98fbc59c71a567f3e23435386c9e93951261db` | 2026-03-15 | no |
| `qwen3.5-9b-heretic-5bit` | `pipenetwork/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-MLX-5bit` | `96e8ef90f060f47caf93111f95fea61b98301d42` | 2026-07-23 | no |

## The measured tier set (one per geometry — named, cost-scoped)

The roster is SEVEN MLX wire tiers (`MLXChatTier.tier(modelId:)`, `MLXChatTier.swift:134-141`). "Every tier at every length" is not a run, it is a fantasy (the memory arithmetic below). This preregistration commits the measured set:

| Geometry | Measured tier (wire id) | Weights | Attention | Declared ceiling |
|---|---|---|---|---|
| std 36L × 8kv × 128hd | `qwen3-8b-4bit` | 4-bit | standard (no GDN interval) | **40,960** (`MLXChatTier.swift:196`; config probed) |
| GDN 24L, interval 4 → 6 FA × 2kv × 256hd | `qwen3.5-2b-4bit` | 4-bit | hybrid GDN | 262,144 |
| GDN 32L, interval 4 → 8 FA × 4kv × 256hd | `qwen3.5-4b-uncensored-4bit` | 4-bit | hybrid GDN | 262,144 |
| GDN 32L, interval 4 → 8 FA × 4kv × 256hd | `qwen3.5-9b-heretic-5bit` | 5-bit | hybrid GDN | 262,144 |

**The transfer refusal (named now, applies at S3):** the other three tiers — `qwen3.5-9b-4bit`, `qwen3.5-9b-heretic-4bit`, `qwen3.5-4b-huihui-4bit` — stay `nil` this round. Same-geometry transfer is REFUSED by KR-2's letter: a constant moves only with its own tier's full matrix table; a geometry equivalence is not a table. Their rows are a future preregistration's scope. (CG-11 measured three tiers — 8B, 2B, heretic-9B-5bit; the 4B geometry is new ground this round.)

## Tier-scoped length ladders

Scoped by the arithmetic below and the declared ceilings — the plan's original one-size ladder (~64k, ~256k everywhere) is IMPOSSIBLE on the 8B and refused here by name:

| Tier | Ladder | Named refusals / at-risk rows |
|---|---|---|
| `qwen3-8b-4bit` | 1k · 4k · 16k · 32k | the ~38–40k ceiling row AT-RISK (attempt; an OOM refusal is a named row) · **64k CEILING-REFUSED** · **256k CEILING-REFUSED** (declared max_position 40,960) |
| `qwen3.5-2b-4bit` | 1k · 4k · 16k · 64k · **256k** | none — the only tier whose full ladder is feasible by the arithmetic (≈5 GiB at 256k-nil) |
| `qwen3.5-4b-uncensored-4bit` | 1k · 4k · 16k · 64k | the **256k rows AT-RISK** (attempted after the shorter rows, budget permitting; an OOM refusal is a named row) |
| `qwen3.5-9b-heretic-5bit` | 1k · 4k · 16k · 64k · **256k × {turbo8v4, varn4v2} only** | **256k-nil MEMORY-REFUSED by arithmetic** (≈8 GiB KV + ≈5.9 GiB weights ≈ 14+ GiB on this 16 GB machine — the refusal row is recorded, never run) · the 256k compressed rows are the feasibility question (≈2.7× ⇒ ~3 GiB; ≈5.3× ⇒ ~1.5 GiB) |

The three transfer-refused tiers carry no ladder this round (the refusal above).

**Needles (the RULER shape — arXiv:2404.06654; "standard NIAH overestimates long-context ability"; the declared ceiling ≠ the effective context):** every run prefills a synthetic fence at the row's length (the ARM L `BRAIN_UNTRUSTED_CONTEXT` shape, `multi-turn-probe/main.swift:334-366`, parametrized to the ladder lengths) with **5 needles — one at each depth 10 / 25 / 50 / 75 / 90% — all distinct keys and distinct planted codes** (≥2 required; 5 committed). One needle question per needle ("code only" + the memory id to cite). A depth where the NIL run itself misses is recorded **BEYOND-EFFECTIVE** for that tier × length; scheme rows are gated only on depths nil resolves, and the boundary itself lands in the tables (that boundary is RULER's whole point).

**Measures per row:** needle recall exactness (planted code returned) · citation-correctness (the cited hit id is the needle's) · answer fidelity vs the nil-scheme goldens (captured FIRST per tier × length — the baseline is a nil run, never a scheme run) · **turn-2 prefill time per scheme (the #717 column — CG-11 never isolated it)**: after the needle block, a second user turn is issued on the same session and its prompt-processing wall time is timed separately from decode, reported as ms per new prompt token · decode tok/s (the engine's `generateTime`) · phys_footprint per milestone (whole-process truth) · the cache's own `memoryBytes` where the scheme exposes it (`TurboQuantKVCache.swift:1454` at 3.32.3; nil carries no such counter — the config-derived arithmetic is nil's baseline) · a per-run content hash (the driver hashes each row's measurement fields; CryptoKit, a system framework — zero new dependencies).

**THE ENGINE-STATE MEASURE DELTA (recorded 2026-11-14, additive):** on this engine the stream split grew the reasoning channel (`.reasoning`, `Evaluate.swift:3301` at `63529075` — "delivered separately from the answer and with the protocol's delimiters already removed"). The rows gain `reasoning_chunks` / `reasoning_chars` (the C-6 observable: the witness that the split happened), and the answer text (`text`, the goldens' basis) is **answer-only BY CONSTRUCTION** — on 3.32.3 reasoning rode the chunk stream verbatim (the CG-11 live witness), so **main-pin goldens ≠ 3.32.3 goldens by design**. The think-filter (bridge path) is NOT touched by the adoption; its primed-phase behavior on this engine carries an OPEN QUESTION requiring the live witness before any bridge-path chat (the adoption record §3.5).

**Run protocol:** strictly sequential, GPU-exclusive (the CG-11 C-7 lesson: the lane's GPU-bound cells never run concurrently with anything that touches the bridge process namespace) · cost order, short ladders first · the nil golden run precedes every scheme run of its tier × length · every row lands as one JSONL line, machine-logged.

## Memory arithmetic (DERIVED-FROM-CONFIGS — arithmetic, not measurement; **re-derived 2026-11-14 from the live HF configs — every number and declared ceiling re-confirmed against the 2026-11-09 derivation; no config drift**)

KV bytes/token at f16 = FA_layers × kv_heads × head_dim × 2 (K+V) × 2 bytes:

| Tier (geometry) | L × interval → FA × kv × hd | bytes/token | 32k-nil | 64k-nil | 256k-nil | Verdict on this 16 GB machine |
|---|---|---|---|---|---|---|
| 8B (36 × 8 × 128, std) | 36 FA (live config: 36 layers, all full attention) | ~144 KiB | ≈4.5 GiB KV (+≈4.6 weights ≈ 9.3) | CEILING-REFUSED | CEILING-REFUSED | 32k feasible; 38–40k at-risk; 64k/256k **CEILING-REFUSED** (declared 40,960 — live config confirms) |
| 2B (6 × 2 × 256) | 24L, interval 4 → 6 FA (live config: 18 linear + 6 full) | ~12 KiB | ≈0.4 GiB | ≈0.8 GiB | ≈3.2 GiB KV (+≈1.8 weights ≈ 5 GiB) | **FEASIBLE — the full ladder runs** |
| 4B (8 × 4 × 256) | 32L, interval 4 → 8 FA (live config: 24 linear + 8 full) | ~32 KiB | ≈1 GiB | ≈2 GiB | ≈8 GiB (+≈2.7 weights ≈ 11 GiB) | ≤64k feasible; 256k **AT-RISK** |
| 9B (8 × 4 × 256) | 32L, interval 4 → 8 FA (live config: 24 linear + 8 full) | ~32 KiB | ≈1 GiB | ≈2 GiB | ≈8 GiB (+≈5.9 weights ≈ 14+ GiB) | 256k-nil **MEMORY-REFUSED**; 256k compressed feasible (the feasibility rows) |

## The scheme set (+ the symmetric refusal, named)

| Scheme | Rows it runs | Basis |
|---|---|---|
| `nil` (fp16 KV) | every ladder row — the baseline AND the goldens | CG-11's evidenced default |
| `turbo0v4` | every ladder row | the CG-11 asymmetric ladder |
| `turbo8v3` | every ladder row | upstream's "recommended default (near-lossless K, compressed V)" — adopted nowhere without our quality evidence |
| `turbo8v4` | every ladder row | the CG-11 asymmetric ladder's deep end |
| `varn4v2` | **the LONG rows only (length ≥ 16k)** | resolves on the engine's documented "for memory-bound long context" — exactly CG-11's reservation and exactly this plan's regime; a candidate, never a default-by-documentation |

**REFUSED by name — symmetric key compression (`turbo4` / `turbo4v2` / `turbo3` / `turbo2`):** upstream's own family-sensitivity table (PR #232, merged 2026-07-20): "Qwen3 / qk-norm families and small models: most sensitive to key compression; asymmetric schemes stay healthy." Our family is Qwen3/Qwen3.5. The refusal is recorded before any run; symmetric rows are not in the scope.

## The preregistered decision rule

Applied per measured tier, at S3, against the tier's FULL table (KR-2 — no constant moves on a summary):

- **Ratchet a tier to `turbo8v3` iff ALL of:**
  1. **zero needle misses** across the tier's matrix — on every depth nil resolves (BEYOND-EFFECTIVE depths excluded, recorded);
  2. **citation-correctness identical to nil** (the cited hit ids match the needle's memory ids exactly where nil cites correctly);
  3. **the answer-quality arms green** — for the 8B this REQUIRES the CG-11 kill-arm cell to **RE-EARN**: the `tests/chat_mlx_tier.rs` 3-arm battery under `BRAIN_CHAT_MODEL=qwen3-8b-4bit` × `BRAIN_MLX_KV_SCHEME=turbo8v3`, the kill arm green 3/3 (a re-red — the deterministic `text: ""` silence — is recorded and the tier is refused; **the cell must re-earn or re-red, never silently pass**);
  4. **turn-2 prefill within tolerance** — the preregistered tolerance is **1.5×**: turn-2 prefill wall time per new prompt token at the tier's dominant length ≤ 1.5× the nil run's per-token turn-2 prefill at the same length (the #717 class — 4–9× on raw/affine keys — fails this gate by design; that failure is the honest outcome, not a surprise);
  5. **memory reduction ≥ 15%** at the tier's dominant length (the cache's own `memoryBytes` vs the config-derived nil KV bytes at the same length; phys_footprint recorded alongside as whole-process truth).
- **Ratchet a tier to `turbo8v4` additionally** iff the `turbo8v3` ratchet holds on that tier AND the V-side compression shows no fidelity delta on the long-context rows.
- **`varn4v2` adopts only under the SAME gate (all five clauses), long rows only**, and only if it beats the turbo ladder on the memory axis without a fidelity delta.
- **Any single miss → the tier stays `nil` and the miss is recorded by name.** Nil standing on every tier is a valid, preregistered outcome (CG-11's own, re-earned).

**Dominant length (preregistered definition):** the longest length in the tier's ladder whose NIL row actually ran (feasibility-proven). Expected: 8B → 32k; 2B → 256k; 4B → 64k (256k only if its nil row ran); 9B → 64k (its 256k-nil row is MEMORY-REFUSED).

**The landing (if a verdict moves):** `MLXChatTier.kvScheme`'s switch per moved tier — resolved once per session at `makeSession` by `MLXBridge.resolvedKVScheme(for:)` (bridge :67-69), never per turn (the C-3 law: a mid-session plan change silently discards the prefix cache). **The rollback shape (KR-3):** a per-tier scheme MAP parsed at the same read-once static — `BRAIN_MLX_KV_SCHEME_MAP="tier=scheme,…"` alongside `BRAIN_MLX_KV_SCHEME` (read ONCE per process, `MLXBridge.swift:61-62`); a field regression is a config flip + process restart, zero kernel/wire/schema surface. `PROBE_KV_SCHEME` stays the probe's own instrument.

## The refusal ledger

Every refusal this round records a NAMED row: **tier × length × scheme × class × reason × evidence pointer**. The classes, closed vocabulary:

| Class | Meaning | Committed now |
|---|---|---|
| `CEILING-REFUSED` | the length exceeds the tier's declared max_position | 8B × {64k, 256k} × all schemes (declared 40,960) |
| `MEMORY-REFUSED` | the config arithmetic puts the row beyond this machine's RAM | 9B × 256k × nil (≈14+ GiB on 16 GB) |
| `OOM-REFUSED` | the row was attempted and the OS/runtime refused — the at-risk rows' honest end | none yet; 8B ~38–40k and the 4B/2B 256k at-risk rows may land here |
| `PRICED-OUT` | S1's calibration priced the row's prefill beyond the per-row budget | none yet; the calibration may only ADD rows here, never run-rows to the scope |

The scope may SHRINK after this commit only by a named ledger row; it may NEVER grow. No unlisted length, scheme, or tier enters a run.

## The runtime-pricing law (S1's calibration)

S1 measures per-tier prefill throughput at 16k (nil scheme, this machine, the matrix driver itself) BEFORE the long rows run. The preregistered per-row budget: **60 minutes of prefill** (a row's decode + turn-2 legs are minutes on every tier's measured class). A row priced beyond it refuses `PRICED-OUT` with its priced number in the ledger. The calibration refuses rows; it never adds them.

---

*The engine-pin line above is structural (KR-4, `tests/kv_ratchet_pins.rs` — whose expected pin is updated consciously in the SAME commit as this doc, the EngineFloorPins pattern): a preregistration without an ENGINE PIN line re-reds that pins file — and that red is the law working. This doc supersedes `docs/REKRA_PLAN_PREREGISTRATION_2026-11-09.md` (which pinned 3.32.3) as the NEWEST preregistration; when the 3.33.0 tag lands, a lexicographically newer doc supersedes THIS one, the pin re-points to the tag, and B0 runs per the unchanged bump prompt. The measurement law stands: the matrix this doc gates runs only if the operator separately names it (18_KV_MAIN_ADOPTION_2026-11-14.md §4).*
