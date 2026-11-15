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
