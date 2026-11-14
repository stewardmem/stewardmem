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

## §S4b · THE MTP SELF-DRAFTER PROBE (the operator-named addendum, 2026-11-14)

**Why it ran:** the S4 park said "until a model/weights change" — same day, the weights change appeared (the oQ4e-mtp quant family ships the MTP layer the pipenetwork quants dropped), and the operator named the probe. The instrument: the driver's new `PROBE_MTP_SELF=1` arm — `Qwen35TextMTPRegistration.register()` → `MTPDrafterModelFactory.shared.load` on the TARGET'S OWN checkpoint (the self-drafter shape: one shared layer, no second resident model) → `SpeculativeDecodingConfig(mtpDrafter:blockSize:2)` (the `max(2, narrowest+1)` law at full precision). Checkpoint: `TheWirelessPhoenix/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-oQ4e-mtp` (~6.2 GB, oQ 4-bit; 29 `language_model.mtp.*` tensors — the naming the upstream registered-factory fixtures cover). Raw logs `/tmp/round19/mtp_oq4e_self.log` + `/tmp/round19/oq4e_baseline.log`.

**The four S4 questions, answered:**

1. **Does the factory load the checkpoint's naming?** YES — the drafter load succeeded against the `language_model.mtp.*` naming (no named error, the session built).
2. **Does the MTP iterator run on the hybrid GDN cache?** YES — no `KVCacheError`, accept= telemetry flows, and the continuation machinery works across turns AND across the tool turn (the mtpStateIsRecoverable path held). The classic path's structural refusal does NOT govern MTP.
3. **Acceptance vs the 40% gate?** **83–94%** — the gate clears by more than 2×.
4. **Is it faster?** **NO — measured 20–27% SLOWER on this machine.** The paired run (same checkpoint, MTP off):

| Turn | without MTP | with MTP (accept) | delta |
|---|---|---|---|
| turn 1 | 34.2 tok/s, gen 5.50 s | 27.4 tok/s, gen 6.86 s (**94%**) | −20% tok/s, +25% wall |
| turn 2 | 33.6 tok/s, gen 4.47 s | 26.8 tok/s, gen 5.60 s (86%) | −20% tok/s, +25% wall |
| tool | 33.7 tok/s, gen 1.07 s | 26.4 tok/s, gen 1.36 s (85%) | −22% tok/s, +27% wall |
| cold | 33.6 tok/s, gen 5.95 s | 24.6 tok/s, gen 8.14 s (83%) | −27% tok/s, +37% wall |

**The structural read:** the naive MTP arithmetic (draft ≈ 1/40 of a pass; a 2-token verify ≈ 1 batched pass; at α=0.9 expect ≈1.8×) promised a large win — the hybrid cache eats it. The GDN recurrent layers process the verification block SEQUENTIALLY through the recurrent state, so the "batched" 2-token verify on this machine costs ≈2.3 passes, not 1: the measured cost per cycle (1.86 tokens ÷ 0.78× speed ⇒ ≈2.38 pass-equivalents) shows the batching benefit does not materialize on the recurrent class — the same architectural fact that blocks the classic path's trimmable/stageable requirement and the prefix rewind. **The user's/operator's instinct ("MTP would be slower on this machine") is now a measured verdict, at acceptance levels that would be spectacular on standard attention.**

**The losslessness held:** outputs and token counts are IDENTICAL between the paired runs (188/150/36/200 out-tokens; 630c/569c/239c/836c reasoning) — speculation verified against the target's own distribution, as constructed. Quality cannot degrade; only speed moved, and it moved the wrong way.

**F3 FINAL: the park converts from presumed to MEASURED.** The 40% acceptance gate alone is insufficient as an admission gate on the hybrid class — acceptance cleared it by 2× and the lever still lost 20–27%. **The December lesson (preregistered here): the decode gate on this machine class must be a NET tok/s delta gate on the SAME checkpoint, never an acceptance-rate gate.** The lever parks until an engine/weights change that makes the hybrid verification batchable — not merely a weights change that ships the MTP layer.

**The secondary finding (the real speed candidate):** the oQ4e checkpoint ITSELF — MTP off — decodes at **33.6–34.2 tok/s vs the roster's pipenetwork 5-bit heretic at ~22 tok/s (~1.5× faster**, oQ 4-bit vs 5-bit, a different fine-tune), AND its tool turn answers **correctly** (`184/97`, 36 tokens, cached=319) — the pipenetwork heretic's model-intrinsic decimal miss is GONE in this fine-tune (a weights fact, not an MTP effect: MTP is lossless). Memory works, cold honest posture unchanged (the cold turn still burns the 200 clamp — the L1 budget's finding applies to this tier equally). **This is the December S0's named admission candidate for the agentic tier** — its admission is the December vehicle (the probe battery + the operator's provenance read on the DavidAU→TheWirelessPhoenix chain); this round only measured it, the roster is untouched.

## §S5 · THE ROUND RECORD

**The commit ledger (brain-server, one commit per step):** `b000ddd0` (S0 the design note) → `b1e256af` (S1 the seam + 13 pins) → `df64026d` (S2 the gate table) → `88eda02a` (S3 the prefix ledger) → `902c5b56` (S4 the MTP verdict) → this commit (S5 the record). Plans repo: `8a3bb6a` (the L2 draft, DRAFT-NOT-FILED) + the plan §7 record row.

**The standing state (everything env-gated OFF at rest):**
- `BRAIN_MLX_THINKING_BUDGET` — unset in every posture; the seam is inert without it (the OFF-at-rest pin); NO default moved in `MLXChatTier` (no budget posture field added — the ratchet verdicts are the December S0's adoption inputs, recorded in the S2 table).
- `MLXChatTier.kvScheme` — still nil everywhere (untouched); `MLXChatTier.specDecode` — still false everywhere (untouched). The decode lever parks BY NAME (S4).
- The wire: NO wire shape moved; the Rust kernel untouched; **the census stays 4282** (a Swift-side round; any drift would be a misread, not this round's doing).
- No fork, no template patch, no new dependencies, no upstream toggle beyond the DRAFT-NOT-FILED.

**The falsifiers' final ledger (all four named):**
- **F1** (budget forces a non-empty answer failing citation identity): **did not fire.** The needle identity holds on every tier/shape (LX-4471, 3,098 tok); the tool-turn content miss is MODEL-INTRINSIC — proven twice (the 4B at max=180 produces the same decimal; the heretic produces it budgeted AND unbudgeted with identical rsn 274c).
- **F2** (`applyingThinkingBudget` unwireable without forking): **did not fire — pre-resolved at S0.** The path existed: `session.components` is the engine's public extension seam and `applyingThinkingBudget` chains (never replaces) the existing factory.
- **F3** (MTP acceptance < 40% everywhere): **fires, and the S4b addendum converts the park from presumed to MEASURED.** At S4 no acceptance was obtainable (absent weights + the classic path's structural refusal + the below-gate 8B control). The operator-named S4b probe then shipped the missing weights (the oQ4e-mtp self-drafter) and measured **83–94% acceptance — the gate clears by 2× — and the lever still runs 20–27% SLOWER** (the hybrid cache's sequential verification costs ≈2.3 passes; the batching benefit does not materialize on the recurrent class). **The decode gate on this machine class must be a NET tok/s delta gate on the same checkpoint, never an acceptance-rate gate** — the December lesson, preregistered in §S4b. The park holds on a measured verdict.
- **F4** (prefix re-prefill within the 1.5× framing): **the split verdict, both readings recorded.** Per-token the rebuild pays the fresh rate everywhere (0.994/1.029/0.991 — within framing, the accepted-cost branch holds at the per-token reading); the absolute long-context quantity (the whole-prompt rebuild, 4.2/10.9/21.5 s per assistant turn at ~3,100 tokens, `cached=0`) is the documented pain class the S3 draft carries. The December matrix re-reads at the tag era.

**The verify-after checklist (plan §6):** the budget's gate table exists (quality + latency, per tier, S2) · the prefix cost table exists (two scales, the ledger's split verdict recorded, the draft DRAFT-NOT-FILED beside the 18 home, S3) · the MTP verdict exists (head-absent per tier + the structural refusal + the below-gate control, S4) · everything env-gated OFF at rest · the falsifiers' outcomes recorded by name · the plans repo carries the draft + the record row · both repos pushed.

**The December boundary (restated):** the KV-scheme matrix and the roster amendment wait for the tag-era S0 — this round did NOT touch them (the matrix mode ran only as the prefix-cost rider at nil scheme, its fidelity column noted and left to December's owner). The tag-era re-verification rides December B0 (the L1 seam's behavior at the REAL 3.33.0, the MTP surface's movement, the prefix ledger's re-read).

**The round's one-line verdict:** the fast tiers got their latency lever measured and wired (L1: the 9B-heretic's deterministic silence class is FIXABLE with a budget shape that clears its natural reasoning length — every witness burn turn answers at max=180, the naturally-correct turns untouched; the 2B needs nothing; the 4B gains −10% wall on the needle turn), the prefix ledger is honest at both scales (the accepted cost at the witness scale, the real pain at long contexts — now with numbers and a draft), and the decode lever parks on two independent, named, weight-and-structure facts. Nothing adopted, nothing defaulted, everything measured.

## §S2b · THE CHAT-EXPERIENCE ADDENDUM (the operator-named instrument, 2026-11-14)

**Why:** "rich answer, least hallucination, good chat" was the one axis the standard arms don't instrument (terse by prompt design). The driver gains `PROBE_RICH=1` (grounded rich summary + the invention-temptation turn, at the PRODUCT clamp 1024 — the 200 witness clamp crushes rich answers to ~20 answer tokens) and `PROBE_NOTHINK=1` (upstream's OWN prompt-level knob: `enable_thinking=false` via the session's `additionalContext` — no fork, no template patch). Raw logs `/tmp/round19/rich_*.log` + `oq4e_b180_full.log`.

**The budgetless discovery first:** at the 1024 clamp BOTH the official 9B-4bit AND the oQ4e burn open-ended prompts budgetless — the official reasons **3,476–4,307 chars (~900–1,000+ tok)** and never closes; the oQ4e reasons 3,576c on the grounded turn. **The overthinking scales with task openness** — the burn class is NOT a 200-clamp artifact; no clamp size fixes it. The thinking-ON posture cannot produce a rich answer at ANY budget shape on this machine class (a budget that clears ~900 tok of deliberation leaves ~100 answer tokens — not rich; a tight budget forces the mid-thought close whose continuation pollutes the answer shape — the official's grounded answer at max=180 was scratchpad-continuing-as-answer).

**THE NOTHINK POSTURE — the finding (all three 9B-class tiers, one run each):**

| Tier (nothink @ 1024) | grounded rich | wall | invented numbers | cold rich (temptation) | wall | cold specifics |
|---|---|---|---|---|---|---|
| oQ4e-mtp | 1,253 ch, structured, cited | 13.3 s | **0 real** (1 = `2,340`→`$2,340` reformat, the arm's documented noise) | disclaims + hedged real-world alternatives | 8.4 s | **0** |
| official 9B-4bit | 1,286 ch, structured, cited | 13.1 s | **0 real** (1 noise) | disclaims + confident real-world assertions | 21.8 s | **17** |
| heretic 5-bit (roster) | 1,315 ch, structured, cited | 15.6 s | **0 real** (1 noise) | disclaims, fully hedged | 7.6 s | **2** |

- The grounded arm: ALL THREE produce clean, structured, memory-cited summaries — vs the thinking-ON burns that produced NOTHING at any clamp. **~3× faster walls** (13–16 s vs the 36–38 s deliberations).
- The temptation arm: all three refuse the fictional premise; the asymmetry is in HOW — the oQ4e and the heretic hedge (0–2 specifics, "might be thinking of", "perhaps"); the official 9B emits **17 confident real-world specifics** (a "Meridian Bridge in Tacoma… famous for its Art Deco design… major renovation completed recently" lecture). One adversarial probe, one seed, no mechanical tripwire for real-world confabulation — **a data point, not a verdict**; the official alignment's least-hallucination prior did NOT hold on this arm.

**The posture conclusion (named for December and for the product path):**
1. **Rich chat: thinking OFF at the product clamp** — the direct-answer shape is the fast (3×), rich, structured, cited posture; the tier choice on the honesty axis favors the oQ4e/heretic hedge shapes on this probe.
2. **Recall/citation work: thinking ON + the budget (max=180)** — the S2 gate's measured shape. **The nothink needle recall is UNMEASURED** — the named gap; the two postures are alternatives (with thinking OFF the budget is moot — no reasoning span exists to bound), never stacked silently.
3. **The product-path wiring note:** the bridge's `streamTurn` passes `primedThinkBlock: tier.primesThinkBlock` (true for Qwen3.5) — with thinking OFF the template does NOT prime the think block, so the primed flag must follow the session's thinking posture (a small named act for the product path; the driver is unaffected — it reads raw).
4. The oQ4e row is COMPLETE (needle ✓ `LX-4471` budgeted, clean; cold budgeted = the `7` fragment — the CALM shape; tool ✓; memory ✓). The December S0's admission table now carries: speed 33.6–34.2 tok/s, tool-literal ✓, needle ✓, honesty hedge ✓ on the temptation, battery outstanding.
