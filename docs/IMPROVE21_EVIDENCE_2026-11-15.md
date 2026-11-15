# IMPROVE21 EVIDENCE — 21 · THE SERVED-CHAT REPAIR ROUND (2026-11-15)

**The execution order:** brain-steward-ip `plans/improvements/EXECUTION_PROMPT_21_SERVED_CHAT_REPAIR_2026-11-15.md` (§0 gate, S0–S6, falsifiers F1–F6).
**The round's one line:** the first live-traffic cascade session witnessed four failure classes on the serving kernel — the metallib deployment gap that killed both writer turns, the guardrail refusal that structurally closes the AFM-only free tier to this store's health-heavy corpus, the 512-token writer truncation, and the synthesis-shaped misroute — one was fixed in place (the bundle copy), this round makes the other three scripted, measured, and guarded.

## §0 · THE GATE (executed first, 2026-11-15; all anchors held, all dup-guards zero)

1. brain-server HEAD `8dca05df` (the release-notes gate commit — the round lands on top of it, exactly the audit lineage).
2. `MAX_RESPONSE_TOKENS: i64 = 1_024` (`chat.rs:110`) · the writer call carries the request's budget (`chat.rs:950`, `max_response_tokens: Some(max_response_tokens)`).
3. `ESCALATE_CONFIDENCE_THRESHOLD: f64 = 0.5` (`cascade.rs:634`).
4. The child's clamp is the pinned string (`main.swift:662` `min(max(maxResponse ?? 512, 16), 1024)`) with the mirror pin holding it in `provider_afm.rs` (:51 comment, :514 pin test).
5. `MLXChatTier.swift` present (`shell-macos-mlx/Sources/MLXFMProbe/`) · the metallib present in the dev tree (`shell-macos/.build/out/Products/Release/mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib`).
6. **Dup-guards all zero:** no `mlx-swift_Cmlx`/`default.metallib` reference in any script or app-Rust outside `tools_pkg_pins` · no `media attached`/`WRITER_RESPONSE_TOKENS`/`ROUTE_ANSWER_FLOOR`/`synthesis_shaped` anywhere in `src/` · no `claims_record_collapsed` in `shell/src` — nobody landed this round's mechanisms.
7. **The live half:** `brain-afm` stdin turn answered `final` with `tier":"mlx_native"`, **exit 0** — the hand-copied bundle beside the live binary is still in place (the diagnosis session's in-place repair, 2026-11-15 16:45). The scripted homes remain broken — S1's scope, not F1's hard-stop (the fixer is recorded: the diagnosis session of this same round's day, recorded in §0.5 of the prompt before execution).

**Serving facts at gate time:** LaunchAgent `com.brain.server` → `~/.local/bin/brain-server` → symlink to `Application Support/StewardMem/tools/1.29.44/brain-server` (v1.29.44 = today's app build, 15:51); env carries **`BRAIN_CHAT_CASCADE=1`** (the operator's recorded act, gate `enabled=true` witnessed in the log at 08:39:58.684617Z), `BRAIN_DB_PATH=/Users/mark/.openclaw/workspace/brain.db`, serving 127.0.0.1:8765, `BRAIN_REQUIRE_AUTH=1`.

## §S0 · THE RED BASELINE (the four before-rows, witnessed live 2026-11-15)

Each row is traceable to a command or a kernel-log quote reproducible from the tree. The kernel log is `~/Library/Logs/brain-server.log` (ANSI stripped in quotes).

### Row 1 — THE METALLIB WRITER-KILL (the deployment gap; the round's S1 target)

The failure that killed both writer turns (route `synthesize` OK, then the child died mid-turn → `bridge_exited` → 503):

```
brain-server.log:1605  2026-11-15T08:41:17.774410Z ERROR brain_server::handlers::chat: chat cascade: writer turn broke (bridge_exited — the bridge process exited), falling through
brain-server.log:1606  2026-11-15T08:41:17.774557Z ERROR tower_http::trace::on_failure: response failed classification=Status code: 503 Service Unavailable latency=4499 ms
brain-server.log:1612  2026-11-15T08:42:43.846642Z ERROR brain_server::handlers::chat: chat cascade: writer turn broke (bridge_exited — the bridge process exited), falling through
brain-server.log:1613  2026-11-15T08:42:43.846724Z ERROR tower_http::trace::on_failure: response failed classification=Status code: 503 Service Unavailable latency=5577 ms
```

Root cause reproduced by command (2026-11-15, this round's S0 — a copy of the live binary in a scratch dir WITHOUT the bundle beside it, the unscripted-placement failure class; the live layout untouched):

```sh
cp ~/.local/bin/brain-afm /tmp/s21_repro/   # no mlx-swift_Cmlx.bundle beside it
printf '{"id":"c1","op":"capabilities"}\n{"id":"t1","op":"turn","session":"s21gate","prompt":"Say hello in one word.","model":"qwen3.5-2b-4bit","maxResponseTokens":64}\n' | /tmp/s21_repro/brain-afm
```

```
MLX error: Failed to load the default metallib. library not found library not found library not found library not found library not found  at /Users/mark/Sites/brain-server/shell-macos/.build/checkouts/mlx-swift/Source/Cmlx/mlx-c/mlx/c/stream.cpp:40
CHILD EXIT: 255
```

The same binary beside the hand-copied bundle (`~/.local/bin/mlx-swift_Cmlx.bundle`, copied 2026-11-15 16:45) answers `final` `tier":"mlx_native"`, exit 0. The deployment facts recorded in §0.5 of the prompt: `brain-afm` at `~/.local/bin` is UNSCRIPTED (absent from the tools pkg payload `build-tools-pkg.sh:72-84`, no pkg receipt, no resource bundle in the app's `binaries/brain-afm`); the lesson already existed in CI (`docs/gate-law-register.md`'s `shell-macos-mlx` row: "the run with `scripts/setup.sh` skipped fails with `Failed to load the default metallib`") and never reached the deployment layout.

### Row 2 — THE GUARDRAIL REFUSAL (Apple's AFM safety layer; the free-tier structural finding)

Witnessed on the wire 2026-11-15 08:39:00Z (the operator's original error report; the kernel log carries the turn start — note `enabled=false`: the AFM tier, pre-arming, the exact free-tier posture):

```
brain-server.log  2026-11-15T08:39:00.397034Z INFO brain_server::handlers::chat: chat cascade gate: enabled=false degraded=false hits=1
```

The error event on the SSE wire (the diagnosis session's capture):

```
event: error … "code":"generation_failed","message":"May contain sensitive content"
```

The typed-contract stream catch (`main.swift` ~:1448-1452) maps ANY FoundationModels stream error — Apple's safety refusal included — to `generation_failed` with the guardrail's own message as the detail. The shell renders it as "The on-device model could not finish this answer" (`shell/src/lib/i18n/en.json:425`). Grounding at the turn: the Vitamin D hit "Unlocking the Power of Vitamin D… in Ireland" (domain `gutmindsynergy`, trust `raw`). Product consequence recorded in S6: the free tier's AFM-only chat (`BRAIN_CHAT_CASCADE` OFF default) structurally cannot serve this store's health-heavy corpus.

### Row 3 — THE WRITER TRUNCATION (the 512-token budget; the round's S2 target)

Witnessed on the wire (the diagnosis session's capture; the kernel log carries the route side — the synthesize turn at 08:46:08):

```
brain-server.log  2026-11-15T08:46:08.452635Z INFO brain_server::handlers::chat: chat cascade: route synthesize (confidence 0.03, raw "synthesize")
```

The writer turn at the chat request's 512-token budget produced a 223-char text cut mid-sentence (`1. **Functions` — a numbered list severed at item 1), the citation gate returned `abstained`, 0 claims cited. The budget chain: the request clamp `chat.rs:354-357` (`unwrap_or(512)`) feeds the writer call (`chat.rs:950`) — the visible answer competes with the MLX think block for the same token budget (`MLXChatTier.swift:157-187` runs `thinkTagsWithEnableThinking`, the primed-`<think>` law, `MLXChatTier.swift:204-208`).

### Row 4 — THE SYNTHESIS-SHAPED MISROUTE (the round's S4 target)

The kernel log, verbatim:

```
brain-server.log:1618  2026-11-15T08:49:57.458774Z INFO brain_server::handlers::chat: chat cascade: route answer_from_memory (confidence 0.02, raw "answer_from_memory")
```

The question was synthesis-shaped ("full benefits of green tea AND what should I take with it" — a conjunction joining distinct asks); the route classifier returned `answer_from_memory` at **confidence 0.02** and the executor served the top hit VERBATIM — an EGCG HISTORY article (domain `gutmindsynergy`) — the evidence-as-answer arm's cost when misrouted. The store anchor for the grounding-pool hygiene (S3): the live store carries **24 `[media attached:` rows** (`knowledge.source='structured'`, `origin='imported'`, `source_path=''`, titles `media://inbound/pasted-text-…`; witnessed ids 11723/11724/11727/11728/11729/11779/11816/11817/11841/11850… — id 11850 is the variant `[media attached: 2 files]`, the prefix filter covers both shapes). The constructor of that title lives OUTSIDE the kernel tree (zero greps in `src/`, `shell/src`, `shell/src-tauri/src`) — the open origin question recorded in S6.

**The confidence-flat observation across the day:** 0.95 / 0.02 / 0.03 / 0.02 across different routes and question classes — S4(a) measures whether the model's confidences are calibrated before any mechanism is chosen (falsifier F4).

## §S1 · THE METALLIB DEPLOYMENT FIX (commit `a4ad66c9`) — the placement becomes scripted everywhere

**The measured law (four live probes, this round):** the child loads `mlx-swift_Cmlx.bundle` from the directory of the **INVOKED path** — a symlink-invoked child ignores the bundle beside the real file (probe: the metallib error), a symlinked bundle beside the link satisfies the loader (probe: `final`, exit 0), cwd is irrelevant. `current_exe()` hands the kernel the invoked path, so **every invoked path needs the bundle beside it**.

- **(a) The tools pkg:** the builder requires `brain-afm-darwin-$ARCH` AND `mlx-swift_Cmlx.bundle` (with its `default.metallib`) in the staged dir — fail-closed, named — and packs the pair into the core payload; the postinstall skips directory entries in the link loop, REFUSES a bundle-less brain-afm LOUDLY (no link, the rest of the payload installs), and extends the preserve law to the bundle (free path → symlink to OUR anchor bundle; occupied → PRESERVED named — this machine's hand-copy). `install-service.sh` (the dev lane, the unscripted placement's likely origin) installs the bundle beside brain-afm under the SAME sealed-provenance gate, refreshing only when the metallib differs.
- **(b) The app bundle — the Posture L drill's finding:** the placement CANNOT ride tauri.conf.json's `macOS.files` map — the bundler's own seal refuses an unsigned nested bundle in Contents/MacOS (`In subcomponent: mlx-swift_Cmlx.bundle … failed to sign app`, the drill's live failure at 18:0x) and offers no hook to sign one. The placement is `scripts/stage-metallib-bundle.sh` — ONE implementation, three lanes: release.yml (ad-hoc, before the verify walk), release-app-sign.sh (Developer ID, before the identity gate), release-app-store.sh (copy-only; the nested-exec sweep signs the bundle with the reduced entitlements — codesign accepts entitlements on a resource bundle, measured exit 0 — and the ITMS verify loop holds). The confs stay clean; a sidecar-bundle.test.ts pin guards the absence by name. `codesign --force --options runtime --sign - --entitlements Entitlements-sidecar.plist mlx-swift_Cmlx.bundle` → exit 0; the parent re-seal → `--verify --strict` green.
- **(c) The pin:** four new `tools_pkg_pins.rs` drills — the green pair links the tool AND places the bundle beside the link (the loader's contract drilled structurally), a bundle-less brain-afm takes NO link with the named REFUSED line, an occupied bundle path is preserved and named, and the static law (the builder's demand + payload copies + the postinstall's refusal + the dir-skip). **9/9 green.**
- **(d) The recurrence drill, GREEN:** a fake home pre-seeded as the app update (brain-afm replaced as a real file, the bundle dropped) reproduces the exit-255 metallib death; the scripted postinstall preserves the occupied path, installs the fresh pair beside it with the bundle link; **the child invoked THROUGH the versioned link answers `final` `tier":"mlx_native"`, exit 0**. The pkg build smoke (drill mode, assembled staged dir): green — the core payload carries the pair (inspected in the expanded pkg); the builder's refusal fires on a missing staged piece (witnessed on `brain-afm-darwin-arm64` before completing the staged dir).
- **The CI legs:** release.yml gains the `sidecar-staged` artifact (brain-afm + the bundle are built ONLY in the 27-SDK job — without this upload the operator's staged dir can never satisfy the builder) and the placement step; `sidecar.mjs stage` stages the bundle from the same SwiftPM build (the metallib presence verified pre- and post-copy, a bundle without its metallib refused by name) and `verify` walks the BUNDLED copy (brain-afm without the bundle beside it = the named metallib failure).

**The scripted update's own live witness (19:2x, install-service.sh):** the bundle law's loud skip fired exactly as designed — `mlx-swift_Cmlx.bundle NOT installed: no staged bundle` — the live hand-copy stayed, and the freshly installed staged child beside it answers `final`, exit 0 (the §0 repro re-run on the new layout).

## §S2 · THE WRITER BUDGET (commit `84b9cc2c`) — the ceiling, never the request default

`WRITER_RESPONSE_TOKENS: i64 = MAX_RESPONSE_TOKENS` (1_024 — the child's own clamp ceiling; the `provider_afm.rs` mirror pin owns that string) passed at the cascade writer call site; **exactly ONE `Some(WRITER_RESPONSE_TOKENS)` call site** (the new source pin counts it — the request path's 512 default is untouched; the AFM tier keeps its ceiling). The pin `s2_the_writer_budget_is_the_ceiling_not_the_request_default` green; the staged tree compiles standalone (a scratch worktree `cargo check` — the step's commit is self-contained).

**The live red→green and F2's verdict:** the writer turn now COMPLETES (the 300 s chat budget below; 41.5 s wall for the witnessed green tea turn) — but the direct child probe at the EXACT writer shape (WRITER_CONTRACT + 1024) returns `final outputTokens 1024, text ""` — **the think block alone consumes the entire budget on the stock Qwen3.5-9B-4bit tier** (the primed-`<think>` law). The witnessed truncation class is dominated by the think-block posture, not the budget. **F2 fired as preregistered: the clamp is NOT raised, the miss is recorded, and the follow-up is parked BY NAME — the think-block posture as its own probe-first round** (the `primesThinkBlock` seam, `MLXChatTier.swift:204-208`, or a no-think writer instruction measured on the bench before anything changes). The kernel's answer to an empty writer text: the citation gate judges → `abstain: true` → the honest abstain envelope (the support verdict `abstained`, 0 cited) — correct by design, never a failure.

## §S3 · THE GROUNDING POOL HYGIENE (commit `73840a42`) — read-time, additive

The chat grounding path drops `[media attached:`-titled hits at EVERY recall intake — the primary pool before the hits map/fence/count/evidence_from_results, the carry-forward intake before `merge_carried`, the read_more deep intake before the evidence rebuild. The count rides EVERY grounding event (`filtered`, the every-key-present law — witnessed live: `"filtered":0` on every turn's grounding event; the count is cumulative across a turn's intakes). READ-TIME ONLY: no row is rewritten, flagged, or deleted; `/recall` and every other reader unchanged. **The live witness:** the chat's reranked pool never surfaced media rows in the witnessed turns (three turns, `filtered: 0` each — the rerank ranks the real rows above the stubs for the probed queries); the shape is proven by the unit test (`the_media_stub_filter_drops_the_witnessed_shape_at_intake`: the stub + the padded variant + the `2 files` variant + real titles pass + the empty-pool shape) and F3's envelope holds by construction — an all-media pool → zero hits → the zero-hit abstain envelope (the `abstains_without_bridge` wiring pinned). The temptation to keep junk hits "so the user gets something" refused by name.

## §S4 · THE ROUTE GUARD + THE CONFIDENCE TRUTH (commit `0858b4e5`; the timeout repair `1c2af8a5`)

**(a) The measurement** (`docs/ROUTE_ROWS_2026-11-15.jsonl`, the round-19 JSONL shape + run-meta): the bench's judged set (106 queries, arms C+D) against a scratch kernel on a fresh store. Determinism witness true; T1 stability 28/30; C-vs-oracle routing agreement 75/96.

**THE F4 VERDICT: MISCALIBRATED —**
- **28 of 92 `answer_from_memory` rows (30%) report confidence BELOW the preregistered 0.5 floor, 75% of them strict-correct** (conf 0.00-0.49 on single-fact queries the verbatim arm answers correctly — the witnessed flat-low shape, now measured across the set).
- The distribution is bimodal-flat: 41 rows at exactly 0.95 regardless of correctness; 80% strict above the floor vs 75% below — **no calibration signal**.
- Per F4's own law the fix is **PROMPT-side, never a numeric transform**: the `ROUTE_SCHEMA` confidence description and the `ROUTER_CONTRACT` now name the anchored calibrated scale (0.9+ only when a top hit's snippet contains the answer; 0.5-0.8 composing; 0.0-0.4 topic-word overlap; "a constant value is a wrong answer"). NO numeric transform exists anywhere (the pin asserts it).
- **The guard lands regardless** (the synthesis-shape predicate is independent of confidence): the executor's verbatim arm fires ONLY when the route is answer_from_memory AND `!synthesis_shaped(&message)` AND confidence ≥ `ROUTE_ANSWER_FLOOR` (0.5, the `ESCALATE_CONFIDENCE_THRESHOLD` family); a refusal falls to the WRITER arm (no new machinery); the refusal is LOUD. The escalation law untouched (`escalation_due`'s exact preregistered conjunctive shape, asserted byte-for-byte).
- **The guard's measured counterfactual, named honestly:** on THIS set the floor arm would fall 28 AFM routes (21 strict-correct) to the grounded writer — latency/tokens, never correctness; the calibration prompt is the lever that shrinks it. The witnessed synthesis-shaped misroute class is the guard's PREDICATE arm (the bench fixture carries no synthesis-shaped joins in the AFM-routed rows — that shape is live-traffic's).
- **NEW STRUCTURAL FINDING:** 9/106 route turns died on the AFM child's own `generation_failed "May contain sensitive content"` — **the S0 row-2 guardrail finding reproduced in the CASCADE'S T1 LEG** (the coordinator is AFM too); those turns fall through to the pre-cascade flow. A route-tier fix (an MLX route tier) is a named NOT-do for this round (no new machinery; the December boundary).

**The live witnesses (the serving kernel, post-restart):**
- **The guard fired live — twice** — on the exact witnessed shape: `route answer_from_memory (confidence 0.00)` → `route guard refused the verbatim path (synthesis_shaped true, confidence 0.00 vs floor 0.50) — falls to the writer` (11:28:00.965Z and 11:32:12.271Z). The misroute CANNOT recur: a synthesis-shaped low-confidence query reaches the writer, whose empty-text answer abstains honestly (`abstain: true`, support `abstained`, 0 cited).
- **The calibration fix visibly working:** the single-fact green tea query ("Does green tea contain EGCG?") routed `answer_from_memory` at **0.90** (anchored — the top hit contains the answer), the guard passed it, **tier `deterministic`**, the verbatim document served, **11 claims all kernel-cited, support: `supported`, 0 uncited** — the evidence-as-answer arm exactly as designed, end-to-end, on the serving kernel.
- **THE 30-SECOND WALL (the live witness's own find, commit `1c2af8a5`):** the writer turn is minutes-scale when honest (a cold 9B load ~60-90 s + generation) and the handler awaits the writer BEFORE the SSE returns — the shared 30 s TimeoutLayer 408'd every cold-tier writer turn (three witnesses, three 408s at exactly 30 s, zero bytes on the wire, the child working). The fix: the IMPROVE06 document-ingest merge-order precedent — `config::CHAT_TURN_TIMEOUT_SECS = 300`, `chat::turn_router()` carries the turn route with its own TimeoutLayer, merged AFTER the shared wall (CatchPanic, RBAC, auth still cover it; the r47 line-order pin is relative and holds). The session-store routes stay inside the shared wall. After the fix: the witnessed turn completes (HTTP 200, 41.5 s), grounding first, then the writer's verdict.

## §S5 · THE CITATION SURFACE COMPACTS (commit `2d5b9eb9`)

The generated turn's answer no longer re-prints every claim sentence-by-sentence below the markdown (the witnessed "second text"): the cited claims collapse into the M3-b disclosure pattern (collapsed by DEFAULT, aria-expanded/aria-controls, the testids intact INSIDE the opened list); a `deterministic`-tier turn names its source on ONE line (`claims_record_collapsed`, "served verbatim from: <unique cited titles>"); **an UNCITED claim NEVER collapses** — it renders below the answer, dimmed with the `unsupported` chip (F6 refused by name: the record compacts its PRESENTATION, never its EXISTENCE). i18n: 4 keys ×5 locales (566 each, parity even; the nl record_count translated REALLY — the byte-identical-to-en pin caught the first draft's loanword). **The size band re-based, the named act:** R0 1_806_313 → 1_807_430 (+1,117 gzipped). svelte-check 0 errors; the shell unit suite 431 passed (the two conscious pins green after their named updates).

## §S6 · THE RECORDS — the falsifiers, the standing state, the open questions

**The falsifiers:**
- **F1 (the §0 repro does NOT reproduce):** anticipated — the hand-repair held at gate time; the scripted homes stayed broken (S1's scope). Recorded as the pass it is; the fixer named (the diagnosis session, recorded in the prompt's §0.5 before execution).
- **F2 (the budget at 1024 still truncates):** **FIRED** — the think block alone consumes the whole budget on the stock 9B tier (`text: ""` at outputTokens 1024). The clamp NOT raised; the miss recorded; the think-block posture parked BY NAME as its own probe-first round. The kernel abstains honestly on the empty text (correct by design).
- **F3 (the media filter empties a real pool):** not witnessed live (the reranked pool never surfaced media rows in the witnessed turns); the empty-pool → zero-hit abstain envelope shape is unit-proven and pinned. PASS by construction.
- **F4 (the confidences calibrated → park the parse-fix):** **MISCALIBRATED** — the parse-fix did NOT park; the fix is PROMPT-side per F4's own law (the anchored scale in the schema description + the contract); the guard landed; NO numeric transform (the pin).
- **F5 (the temptations):** all refused by name — no LLM classifier in the guard (a pure string scan), no row rewrites (read-time filter), no "ingest my chat sessions" (the named NOT-do), no clamp raise (the mirror pin owns it), no default moved beyond the operator's own `BRAIN_CHAT_CASCADE=1`.
- **F6 (drop the claim record entirely):** refused by name — the uncited arm renders dimmed with the chip; the deterministic tier names its source; the disclosure keeps the record reachable.

**The standing state (nothing else defaulted):** the metallib placement scripted in all three homes (the tools pkg, the app lanes, install-service.sh) · the writer budget 1024 (the ceiling) · the grounding pool filtered read-time (the count on the wire) · the route guard on (shape + floor) · the calibration scale in the route prompt · the chat turn route carries its own 300 s budget · the citation surface compacted · `BRAIN_CHAT_CASCADE=1` the operator's own recorded act (untouched by the round) · the writer model unchanged (`qwen3.5-9b-4bit`, the S0-measured tier) · the child's clamp unchanged (the mirror pin).

**The open questions + the named follow-ups:**
1. **The think-block posture** (F2's park): the primed-`<think>` law consumes the writer's whole budget on the stock tier — the follow-up round probes `primesThinkBlock` (`MLXChatTier.swift:204-208`) or a no-think writer instruction, measured on the bench BEFORE anything changes. Until then the writer's empty text abstains honestly.
2. **The `media://inbound` constructor lives outside the kernel tree** (zero greps in `src/`, `shell/src`, `shell/src-tauri/src`) — the title's writer is unnamed; the read-time filter keys on the prefix wherever the constructor lives. The write-posture question ("ingest my chat sessions") stays the named NOT-do.
3. **The cascade's T1 leg inherits the AFM guardrail** (9/106 bench route turns died on `May contain sensitive content`) — a route-tier on the MLX side is the candidate fix; a named NOT-do this round (no new machinery), recorded for the December S0.
4. **The bench's writer rows are not cross-round comparable** (this run rode the Heretic via mlx_lm.server with thinking disabled — named in the run-meta); a like-for-like round-17 comparison needs the stock 9B-4bit writer.
5. **The settings-surface feasibility** (recorded per the prompt): the roster is `CaseIterable`; `prewarm` exists; per-conversation `model` ids are wire-native — a settings round is buildable on existing primitives.

**The December boundary restated:** the KV matrix, the roster amendment, and any default moves wait for the tag-era S0.

## §S6.1 · THE FINAL SWEEP + THE PIN-COMPLIANCE PASS (commit `6bee61d4`, the round's own code met the standing gates)

**The pins on the round's own additions:** the comments gate flagged four label-bearing lines (the S4/F4/F5/F3 references in the guard's and filter's doc comments) — the labels dropped, the invariant sentences kept (the round-20 house form; the gold-derivation line now states the shape it guards); the log-injection pin flagged the media-filter's dropped-count interpolation — the count rides the `LogValue` sanitised seam. main_suite green 354/0 after.

**The sweep record:** 20/21 lanes green; the `all-targets` lane's reds across the sweep runs were contention flakes at TWO DIFFERENT memory-sensitive tests (`docling_goldens_round_trip_on_the_recorded_versions` ×1, `the_default_pipeline_has_no_correction_surface` ×1 — the docling/doc-correction suites load torch models; the sweep's parallel lanes contend for RAM) — **each green on immediate standalone rerun, and the lane green TWICE back-to-back standalone (37 suites, zero failures, 2×)**. The lanes: clippy-otel, the six feature lanes, test-crates, test-harness, audit, lock-freshness, docs-truth, env-truth, secrets-truth, badges, lipstyk — ALL PASS.
