# IMPROVE19 EVIDENCE — 19 · FAST-CHAT LATENCY (2026-11-14)

**The plan:** brain-steward-ip `plans/improvements/19_FAST_CHAT_LATENCY.md` · **The execution order:** `EXECUTION_PROMPT_19_FAST_CHAT_2026-11-14.md` (§0 tree-state gate, S0–S5, falsifiers F1–F4).
**The round buys LATENCY** (the 18 round bought memory): L1 the thinking budget, L2 the prefix-stability ledger, L3 the MTP acceptance measurement — every lever env-gated OFF at rest.

## §0 · THE GATE (executed first; all three held 2026-11-14)

1. Pin: `Package.swift:81` still `revision: "6352907543270ae9c61fcaf59cb3e07b0b989ea1"` — the adopted main snapshot, NOT re-pointed.
2. HEAD `3777e375` (the G-1 journal entry), ancestor-verified ≥ `75f1185d` — exactly the tree note the prompt anticipated (post-G-1, Rust-kernel-only since; disjoint from this round's Swift surfaces).
3. `noteEngineReasoningSplit` present in `MLXBridge.swift` (2 hits) — the witness's outcome (a) landed.

**The upstream re-probe (prompt §0's live re-probe):** the tags API returned latest **3.32.3** — no NEW tag beyond the known set; no 3.33.0 exists at probe time. The hard stop does not apply; the round completes on the adopted snapshot and the tag-era re-verification rides December B0 as planned.

**The checkout:** `.build/checkouts/mlx-swift-lm` at `63529075` ("Use MLXLogger for the model component warning (#790)") — the pin, verbatim.

**The suite baseline:** `swift test` = **38 tests in 13 suites, all passed** (re-run at S0, live).

## §S0 · THE L1 WIRING DESIGN (the design note, written BEFORE code)

**The verified negative, re-probed live at the pin:** `ThinkingBudget` appears ONLY in `Libraries/MLXLMCommon/ThinkingBudget.swift` and `GenerationComponents.swift` (the API extension itself) — **zero consumer wiring** in `GenerateParameters`/`ChatSession`, exactly plan §0.1's ground. Wiring it is this round's S1.

**The chosen apply shape: the SESSION WRAPPER at the bridge's session-setup seam** (plan §0.1's third shape), not a transformer or iterator-stage change:

1. `ChatSession.components` is a **public, settable** `var components: GenerationComponents` (upstream `ChatSession.swift:437`), documented "applied on each generation", with the factory invoked FRESH per `respond(to:)` so stateful processors never leak across turns — the engine's own contract.
2. `GenerationComponents.applyingThinkingBudget(_:reasoning:tokenizer:diagnosticHandler:)` (upstream `ThinkingBudget.swift:490–532`) builds via `appendingParameterValidator` + `appendingLogitProcessor`, both of which **COMPOSE with whatever is already present** (upstream `GenerationComponents.swift:59–95` — the existing factory is captured and chained via `ChainedLogitProcessor`, never replaced).
3. Our bridge ALREADY rides this exact seam: `applyGuidedDecoding` sets `session.components.logitProcessorFactory` (MLXBridge.swift:182, the round-14 guided path). The budget joins the same pattern — no new surface class.
4. **The ordering law this creates:** `applyGuidedDecoding` ASSIGNS `logitProcessorFactory` directly (it would CLOBBER a budget applied first). Therefore the budget is applied **AFTER** guided decoding in the session-setup sequence — the only ordering where both compose.
5. **No wire shape moves.** The Rust kernel ↔ child bridge protocol is untouched; the budget is session-local generation behavior, invisible to the wire. The Rust census (4282, the G-1 baseline) does not move — a Swift-side round.

**The apply-point:** `MLXBridge.makeSession(container:tier:instructions:seed:maxTokens:speculative:)` (MLXBridge.swift:137–156) — the same construction seam that already resolves the KV scheme (`resolvedKVScheme(for:)`) and the spec-decode posture. Session-stable BY CONSTRUCTION (the config rides the session's construction, never a per-turn flip — the MLXChatTier.swift:231–236 law), mirroring the KV posture exactly.

**The config surface (the KR-3 rollback shape):**

- `BRAIN_MLX_THINKING_BUDGET="max=N[,minAnswer=M]"` — read-once `static let thinkingBudgetOverride: ThinkingBudgetConfiguration?` beside `kvSchemeOverride` (MLXBridge.swift:62's pattern), parsed once at process start. Malformed strings (non-numeric, negative, missing `max=`) parse to **nil + a named stderr line** — a mis-typed knob degrades to OFF, never to a guessed budget.
- OFF by default everywhere: unset = `nil` = the current behavior byte-for-byte (the plan §4 law: no default moves without witnesses).
- Rollback = a config flip + restart (env unset), zero kernel/wire/schema surface — the KR-3 shape.

**The transition decision:** the fast tiers' `MLXChatTier.configuration.reasoningConfig` is `.thinkTagsWithEnableThinking` (MLXChatTier.swift:157/163/181 — the E1c shape), which **deliberately declares NO `budgetTransition`** (upstream `ReasoningConfig.swift:190–199`: sharing `<think>`/`</think>` with Qwen3 "does not mean a family was trained to respond to the same forced early-stop sequence"). The engine's own doc names the opt-in: `transitionOverride` is "how an application opts a model in anyway, after validating the behavior for its exact weights and tokenizer." The seam therefore supplies **`transitionOverride: .immediate`** — the documented conservative choice ("closes the span with the model's own end delimiter and adds nothing else") — and the REQUIRED validation is this round's S2 gate: answer completeness + citation identity + the quality arms, per tier, before anything ratchets. This is also the CALM caution (plan §0.3) honored structurally: the gate is consistency-shaped, not truncation-happy.

**The reasoning argument:** taken from the tier's own `configuration.reasoningConfig` — non-nil for every Qwen3.5 tier. A tier whose reasoningConfig is nil (the Qwen3-8B plain config) **refuses the budget BY NAME** — the plan's delimiters must come from a declared protocol, never guessed. (The 8B is outside this round's fast set: 2B / 4B-unc / 9B-heretic.)

**The headroom law (preregistered here):** `applyingThinkingBudget` validates `maxTokens >= budget + unicode-completion(3) + protocol-headroom + minAnswer` and throws `.insufficientGenerationTokenLimit(required:actual:)` at setup (upstream ThinkingBudget.swift:506–527). At the witness clamp (200) a `max=150,minAnswer=1` budget requires ≈155 — the seam lets this throw propagate as the refusal (fail-closed at session construction, the P-1 shape: an over-large budget is refused BY NAME, never silently clamped).

**The refusal path (the F2 pre-check, recorded here in advance):** if `applyingThinkingBudget` cannot be reached from the ChatSession path without forking, the finding is recorded and L1 PARKS (never forked — house law) and the upstream request joins S3's draft. **The pre-check RESOLVED at S0:** the path exists (`components` public var + the append API); no fork is needed. F2 does not fire.

**The measurement instrument (S2's rider):** the driver's ARM B two-turn shape (multi-turn-probe/main.swift:595–607) at the 200 clamp — `askChat`'s `TurnMeasure` already carries `reason=` counters (reasoningChunks/Chars), the token split, wall, tok/s, and (E2) acceptance. The budget application for the driver's sessions rides the SAME static helper the bridge uses (the driver imports MLXFMProbe) — one seam, two consumers, no duplicated apply logic.

**The witness corpus for the S2 gate (preregistered):** the clamp-burn class's own prompts — recall-shaped turn-2 questions at the 200 clamp on the tiers that burned (`out=200`, empty answer, `reason=645c/877c`), plus the ARM B secret-recall pair (the citation-identity arm: the budgeted run must still recall the SAME remembered value and the needle corpus's citation behavior must hold).

## §S1 · THE SEAM (the implementation record)

**The env surface (the KR-3 shape):** `BRAIN_MLX_THINKING_BUDGET="max=N[,minAnswer=M]"` — parsed by `MLXBridge.parseThinkingBudget` (total, fail-closed: malformed → nil, the reason named on stderr once; extra fields/negatives/non-numbers are malformed, NOT ignorable), read once per process into `MLXBridge.thinkingBudgetOverride` beside `kvSchemeOverride`/`specDecodeOverride` (MLXBridge.swift, the override family). `.immediate` transition baked into every parsed config (the S0 decision). OFF by default; the rollback is a config flip + restart.

**The apply seam (one core, two consumers):**

- `MLXBridge.applyThinkingBudget(session:reasoning:container:)` — the env wrapper: nil override → returns untouched (byte-identical sessions); else delegates to the explicit core.
- `MLXBridge.applyThinkingBudget(session:budget:reasoning:container:)` — the explicit core (the pins' deterministic shape): nil reasoning config → `ThinkingBudgetRefusal` (named); else fetches the container's tokenizer, appends via the upstream `applyingThinkingBudget` (which CHAINS any existing factory — guided decoding survives), wires the diagnostic handler (the engine's stand-down failsafe → named stderr line, never swallowed).
- **Consumer 1 — the wire child** (shell-macos `brain-afm/main.swift`, `seededMLXSession`): applied AFTER the guided-decoding block — the S0 ordering law (that step ASSIGNS `logitProcessorFactory`; the budget appends to what it left). Session-stable like the KV posture: the knob rides the session's construction.
- **Consumer 2 — the driver** (multi-turn-probe/main.swift, `applyBudgetIfSet` over `probeConfiguration`'s reasoningConfig): applied in ARMs B/D/C/L for the measurement runs. Matrix mode deliberately does NOT apply the budget (it measures KV schemes, December's round — a budgeted think block would confound the KV rows' fidelity gates) and NAMES that exclusion on the record when the knob is set alongside `PROBE_MATRIX`. Every run prints its budget posture (ON with the parsed values / off) — the instrument records its own state.

**The pins (`Tests/MLXFMProbeTests/ThinkingBudgetPins.swift`, 13 tests):**

- Hermetic (10): the parser (max-only default, max+minAnswer, whitespace tolerance, the legal zero budget, NINE malformed forms → nil, nil input) · the static-consistency pin (the read-once static IS the parser on the live env value — the wiring shape, so no future edit forks the static from the grammar) · the OFF-at-rest pin (unset env → nil override) · the tier gate (all six Qwen3.5 tiers declare a reasoning protocol; the plain Qwen3-8B declares none and refuses BY NAME).
- Live under `BRAIN_FM_MEASURE=1`, CI 0.6B with a declared think-tags protocol (3): the apply core composes (`session.components.logitProcessorFactory` non-nil after) · **the headroom law witnessed through OUR seam** (budget max=40 + clamp 1 → `.insufficientGenerationTokenLimit` at generation start, BEFORE prefill — the P-1 shape on the budget path) · the declared-protocol refusal (nil reasoning → `ThinkingBudgetRefusal` naming the law).

**The gates:** `swift build --build-tests` green (both packages: shell-macos-mlx AND the wire child shell-macos/brain-afm) · `scripts/setup.sh` · `swift test` **51 tests in 14 suites, all passed** (the 38 originals untouched + 13 new pins; the 3 live pins additionally ran green under `caffeinate -is`) · the seam OFF at rest (env unset) — no default moved.

**F1–F4 status:** none fired at S1. F2 (no-fork wiring) was pre-resolved at S0; the others wait on their measurements.

## §S2 · THE GATE TABLE (per fast tier, budgeted vs unbudgeted)

**The instrument:** the driver's ARM B/D/C + ARM L (the ~2,000-target fence, measured 3,098 tokens) at the 200 clamp, `PROBE_TEXT_ONLY=1`, `caffeinate -is` on every run, one run per row, raw logs `/tmp/round19/*.log` (the run printout carries the posture line). Budget shape `max=120` (the round's first measure) and `max=180` (the 9B's ratchet candidate — chosen because the tier's naturally-correct turn 1 reasons ≈175 tokens while the burners exceed 200).

**THE WITNESS REPRODUCED BYTE-EXACT (the unbudgeted 9B-heretic run):** turn 2 `out=200` EMPTY `reason=645c` and ARM C cold `out=200` EMPTY `reason=877c` — the plan §0.2's witnessed numbers (`645c`/`877c`), deterministically at temperature 0, on today's tree. The class is live, the corpus is real, and the budget's target is on the wire.

### The per-tier gate (plan §1's four clauses)

| Tier | Budget | turn1 | turn2 (the burn) | tool (ARM D) | cold (ARM C) | needle (ARM L, 3098 tok) | Verdict |
|---|---|---|---|---|---|---|---|
| **2B** (`Qwen3.5-2B-4bit`) | OFF | `ok` 1 tok 26.9 t/s | `184/97` 6 tok 52.6 t/s 0.11s | tool WORKS `184/97` (cached=288) | honest refusal 53 tok | `LX-4471` 7 tok | baseline |
| **2B** | max=120 | `ok` 1 tok 57.5 t/s | `184/97` **identical** 55.3 t/s | tool WORKS **identical** | honest refusal **identical** | `LX-4471` **identical** 55.4 t/s | **REFUSE the budget** — nothing to bound (the tier closes its think block naturally; outputs byte-identical, latency within noise +1.7–5%); a knob with zero gain is not a posture |
| **4B-unc** (`avan-ag/Qwen3.5-4B-Uncensored`) | OFF | `ok` 81 tok, rsn 214c | `184/97` 57 tok, rsn 190c | **BURN**: out=200 EMPTY rsn 489c | honest refusal 90 tok | `LX-4471` 150 tok 3.77s | baseline — **the clamp-burn class reproduced live on the tool turn** |
| **4B-unc** | max=120 | `ok` **identical** | `184/97` **identical** | out=140, rsn 377c → non-empty `1.8969…` (still not `184/97`) | honest refusal **identical** | `LX-4471` **identical**, 129 tok, **wall 3.40s (−9.8%)** | **RATCHET-ELIGIBLE (all four clauses hold)** — but the one burn it fixes remains wrong-content (model-intrinsic, see below); the gain is the needle turn's −10% wall. The December S0 holds the adoption; OFF at rest this round |
| **9B-heretic-5bit** (`pipenetwork/…Heretic-MLX-5bit`) | OFF | `ok` 178 tok rsn 583c | **BURN: out=200 EMPTY rsn 645c** (the witness, byte-exact) | decimal `1.8969…454` rsn 274c | **BURN: out=200 EMPTY rsn 877c** (the witness, byte-exact) | **BURN: out=200 EMPTY rsn 759c → needle MISSED** | baseline — the tier answers NOTHING on the burn turns; warm recall verdict NO RECALL |
| **9B-heretic-5bit** | max=120 | **NEW MISS**: forced close mid-thought → the "answer" is a scratchpad fragment | **FIXED**: `184/97`, out=58, **wall 2.71s (−69%)** | decimal (unchanged — budget never fired, rsn 274c < 120) | `7` (fragment, **no secret leak**) | **FIXED**: `LX-4471`, wall 6.27s (−31%) | the aggressive shape: the burns die, but turn 1 (naturally correct unbudgeted) breaks — **the CALM lesson measured**: the shape must clear the tier's natural reasoning length |
| **9B-heretic-5bit** | **max=180** | `ok` **natural close, identical to OFF** (rsn 583c ≈175 tok < 180 — budget never fires) | **FIXED**: `184/97`, out=188, wall ≈ burn wall (the burn's 200 tokens buy NOTHING; the same wall now buys the answer) | decimal (unchanged) | `7` (no leak — the compliance separation HOLDS) | **FIXED**: `LX-4471`, out=189 | **RATCHET (all four clauses hold at max=180)**: unbudgeted the witness turns NEVER produce answers (wall-to-answer = ∞); budgeted every burn turn answers; the naturally-correct turns are byte-identical (the budget never fires under 180) |

### The four clauses, read across the table

1. **Answer completeness** — 9B at max=180: every witness-corpus burn turn returns a non-empty answer, **zero new empty-answer events**, and the naturally-correct turns are unchanged (the budget is inert under 180 on this tier). 2B: identical. 4B: identical except the tool turn (empty → non-empty).
2. **Citation identity** — the needle corpus (ARM L at 3,098 tokens, this round's measured depth): the budgeted runs cite the SAME needle (`LX-4471`) as unbudgeted on all three tiers; the recall value (`184/97`) identical; the CG-11 citation law holds under the budget. (The depth ladder is the matrix instrument — December's; this round's depth is the one ARM L length, recorded as such.)
3. **Quality arms** — memory works (warm recalls, budgeted), cold control does not leak the secret (the 9B's cold `7` fragment contains no `184/97`; the compliance separation holds), tool continuation unchanged (see the model-intrinsic note).
4. **Latency** — recorded per turn above. The headline: **9B turn 2 at max=120 wall 2.71s vs the 8.84s burn (−69%), needle turn −31% at max=120**; at max=180 the wall equals the burn wall but buys the answer the burn never delivered (unbudgeted wall-to-answer on those turns is infinite — they never answered). 2B: zero gain (nothing to bound). 4B: −9.8% wall on the needle turn.

### The model-intrinsic tool-turn content failure (NOT a budget miss)

The tool turn answers the decimal `1.8969…` instead of echoing `184/97` on BOTH the 4B and the 9B — and the F1 diagnostic settles causation: at `max=180` (the budget effectively out of the way) the 4B produces the **same decimal**; the heretic produces the same decimal budgeted AND unbudgeted (identical rsn 274c — the turn's reasoning never reaches the budget). The fine-tunes compute the division; the budget did not cause it. **F1 does not fire.** ARM D's `.unreachable` is the tiers' recorded posture, unchanged by this round.

### Falsifier ledger after S2

- **F1** (budget forces a non-empty answer failing citation identity): **does not fire** — the needle identity holds on every tier/shape; the tool-turn content failure is model-intrinsic (proven above), not budget-forced. The max=120 turn-1 fragment on the 9B is a COMPLETENESS/quality miss on a non-needle turn and is recorded by name in the table (the shape lesson, not a falsifier).
- **F2** (no-fork wiring): did not fire (pre-resolved at S0).
- **F3** (MTP acceptance < 40%): pending S4.
- **F4** (prefix cost within the 1.5× framing): pending S3.

**Standing state after S2:** the seam OFF at rest everywhere — no default moved; the ratchet verdicts are the December S0's adoption inputs.

## §S3 · THE PREFIX-COST LEDGER + THE DRAFT (L2)

**The prefix-cost table** — the re-prefill per assistant turn, two scales:

**The witness scale (short conversations, the S2 runs' t2 legs):**

| Tier | turn-2 prefill | cached | tool-turn cached | the split |
|---|---|---|---|---|
| 2B | 55 tok | **0** | **288** | the assistant-turn rebuild vs the tool-shaped cache hit |
| 4B-unc | 53 tok | **0** | **322** | 〃 |
| 9B-heretic | 53 tok | **0** | **317** | 〃 — byte-identical to the 2026-11-14 witness's 317 |

Short conversations pay 53–55 re-prefilled tokens (~70–190 ms at the measured per-token rates) — the accepted cost, documented.

**The long-context scale (the matrix-mode rider, fence 3,098–3,107 tokens, nil scheme, budget OFF, `caffeinate -is`; JSONL rows `/tmp/round19/prefix_*.jsonl`):**

| Tier | rebuild tokens (t2 leg) | rebuild ms | rebuild ms/tok | fresh ask1 ms/tok | ratio | the turn's generation ms |
|---|---|---|---|---|---|---|
| 2B | 3,189 (cached=0) | 4,159.6 | 1.3043 | 1.3121 | **0.994** | 224–257 |
| 4B-unc | 3,190 (cached=0) | 10,860.0 | 3.4044 | 3.3072 | **1.029** | — |
| 9B-heretic | 3,187 (cached=0) | 21,542.8 | 6.7596 | 6.8193 | **0.991** | — |

**The read:** the rebuild's per-token cost equals a fresh prefill (0.99–1.03×, all within the 1.5× framing — no penalty rate); the cost is purely the QUANTITY — the ENTIRE prompt re-prefills every assistant turn (`cached=0`), which at the ~3,100-token context dominates the turn wall (~95%+: the 9B pays 21.5 s prefill for ~0.2–0.5 s of generation), while the tool-shaped continuation keeps `cached=288/317/322` and pays only the new tokens. **The pain class is long contexts, exactly as plan §2 framed; short conversations are accepted-cost.**

**The budget × replay interaction (measured, recorded):** the budget changes the REPLAYED reasoning length, and the replay rides the rebuilt prompt — the 9B's turn-2 prefill was 53 tok at max=180 (the natural close → the natural replay) but 109 tok at max=120 (the forced close's different reasoning → a longer replay). A budgeted posture's prefix cost follows its shape; the December S0's adoption tables carry both columns.

**The 9B rider's fidelity note (honest, December's domain):** the 9B matrix row recorded recall 3/5 (2 misses, ask1 `out_tokens=434` — the unbudgeted long-reasoning shape at the 512 clamp; the burn class's shadow at long context). This is the KV-matrix instrument's fidelity column — the 18 round's December measurement owns it; recorded here because the rider produced it.

**The upstream feature-request draft:** `19_PREFIX_STABILITY_ENGAGEMENT_DRAFT_2026-11-14.md` (brain-steward-ip `plans/improvements/`, beside the 18 drafts' home) — **DRAFT-NOT-FILED**: the `reasoning_content` replay toggle (per-session or per-`ModelConfiguration`, defaulting to today's behavior), with the witness split and this table as the motivation, the explicit not-asking list (no default change, no template patch, no rewind across recurrent layers), and the companion-relationship to 18's draft (a) named. **Filing is the operator's act** — nothing filed without the explicit yes.

**F4 outcome:** the 1.5× framing HOLDS per-token on all three tiers (0.99–1.03) — the accepted-cost branch is satisfied at the per-token reading; the absolute long-context quantity is the documented pain class the draft carries. Both readings recorded; the ledger does not hide the split. The December matrix's own t2 columns will re-read this at the tag era.

## §S4 · THE MTP VERDICT (L3)

**The head probe per tier (the plan's load-and-probe discipline, executed at the weights level):**

| Tier | config declares | safetensors carry | probe verdict |
|---|---|---|---|
| 2B (`mlx-community/Qwen3.5-2B-4bit`) | `mtp_num_hidden_layers: 1`, `mtp_use_dedicated_embeddings: false` | **0 mtp keys of 991** (1 shard) | **HEAD ABSENT in the shipped weights** |
| 4B-unc (`avan-ag/Qwen3.5-4B-Uncensored-MLX-4bit`) | same declaration | **0 mtp keys of 924** (1 shard) | **HEAD ABSENT** |
| 9B-heretic-5bit (`pipenetwork/…5bit`) | same declaration | **0 mtp keys of 1,260** (2 shards) | **HEAD ABSENT** |

The probe method: the full safetensors header (the u64-length JSON) of every shard of every tier repo, read locally (`/tmp/round19/scripts/st_keys.py`, path-argument, `-I`). The upstream machinery is PRESENT and REGISTERED at the pin (`Qwen35TextMTPRegistration` registers `qwen3_5_text`/`qwen3_5` → `Qwen35MTPDraftModel`; `MTPDrafterModelFactory` loads from a checkpoint directory; `SpeculativeDecodingConfig(mtpDrafter:blockSize:)` ≥ 2) — so the absence is NOT an engine gap: **the quantizers dropped the MTP layer weights while the configs still declare the layer** — the plan's anticipated re-quantization fact, pinned like any weights fact. A drafter load from these checkpoints cannot succeed (no `mtp.*` parameters exist to load).

**The classic drafter fallback (E2's existing knob, `PROBE_SPEC_DECODE=1`, the Qwen3-0.6B-4bit drafter, numDraftTokens=5) — REFUSED on all three fast tiers, BY NAME:**

`KVCacheError(message: "Speculative decoding requires a trimmable or stageable main KV cache.")` — turn 1, every Qwen3.5 tier (2B / 4B-unc / 9B-heretic), the same named engine error at the full-precision default posture. The hybrid cache (standard KV + the GDN recurrent class) is not trimmable/stageable for the classic spec-decode path — a STRUCTURAL refusal of the engine, not a harness artifact:

**The control proves it:** the standard-attention Qwen3-8B **runs** the same knob on the same harness — and delivers the second half of the verdict: acceptance **37% / 38% / 28%** (turn 1 / turn 2 / cold) — **below the preregistered 40% gate** (`specDecodeEvidenced`, MLXBridge.swift:90) — at **11.7 tok/s** (vs the 8B's unassisted ~26–28 tok/s in the CG-11 tables): below the gate, the drafter is pure overhead, the P-3 law's own shape, measured. (The 8B also re-showed the burn class on turn 1/cold — out=200, empty.)

**The verdict table:**

| Path | 2B | 4B-unc | 9B-heretic | 8B control |
|---|---|---|---|---|
| MTP (self-drafter shape) | weights ABSENT | weights ABSENT | weights ABSENT | n/a (not this round's tier) |
| Classic drafter (0.6B) | **refused by name** (hybrid cache) | **refused by name** | **refused by name** | runs; accept 28–38% < 40%; 11.7 tok/s (slower) |

**F3 verdict (recorded BY NAME, stronger than the preregistered threshold):** the falsifier's letter was "acceptance < 40% on every measured tier" — the measured reality is stronger: **no acceptance was obtainable at all** (the MTP head is absent in the shipped weights AND the classic drafter is structurally refused on the hybrid cache), and the one tier where the path RUNS (the 8B control) measures BELOW the gate AND slower than unassisted. **The decode lever parks until a model/weights change** — MTP weights shipped in the quants, or the hybrid cache gaining trimmable/stageable speculation support (either is upstream/weights work) — never adopted by documentation. The tag-era re-verification rides December B0 as planned (the #351/#506/#516 surface may move; the quant ecosystem may start shipping the MTP layer).

**The scheme-interaction note (December's domain):** the classic refusal fired at the full-precision default posture; whether a quantized KV scheme makes the hybrid cache "stageable" is the KV matrix's question — out of this round's scope by law (§4), named here so December's S0 reads the refusal with this note.

## §S5 · THE ROUND RECORD

*(lands with the S5 commit — the standing state, the falsifiers' outcomes by name, the December boundary restated)*
