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

*(lands with the S1 commit)*

## §S2 · THE GATE TABLE (per fast tier)

*(lands with the S2 commit — the four clauses per tier: completeness, citation identity, quality arms, latency; ratchet or refuse BY NAME)*

## §S3 · THE PREFIX-COST LEDGER + THE DRAFT

*(lands with the S3 commit)*

## §S4 · THE MTP VERDICT

*(lands with the S4 commit)*

## §S5 · THE ROUND RECORD

*(lands with the S5 commit — the standing state, the falsifiers' outcomes by name, the December boundary restated)*
