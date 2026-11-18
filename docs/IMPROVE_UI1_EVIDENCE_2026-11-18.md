# UI1 · THE QUIET SURFACE — S0 baseline receipt (2026-11-18)

Gate (§0): tree clean (`git status --porcelain` empty) · HEAD `84310c3f`
(at the required commit) · the three pins present
(chat-home-state.test.ts:160, chat-spine.test.ts:941, ai_act_pins.rs:246) ·
`chat.disclosure.ai_interaction` = 1 in each of the five catalogs ·
zero hits for TurnJourney/turn-meta/chat-meta-row in shell/src ·
no other session mid-round.

## The five baseline tallies

| Lane | Tally |
|---|---|
| vitest (`pnpm --dir shell test`) | **432 passed \| 2 skipped** (434 tests; 59 files passed, 1 skipped) |
| svelte-check (`pnpm --dir shell check`) | **0 errors, 0 warnings** |
| eslint (`pnpm --dir shell lint`) | **clean** (no output) |
| e2e chromium (`pnpm --dir shell test:e2e --project=chromium`) | **9 passed \| 1 skipped** (42.1s) |
| the band pin (`the_built_js_stays_within_half_a_kib_gzipped_of_the_r0_baseline`) | **green** — `R0_BASELINE = 1_807_430` gzipped (chat-spine.test.ts:1140, band ±0.5 KiB) |

The band pin runs in ~5.3s standalone (fast incremental vite build on this
machine); it is included in the full vitest run.

## S2 · the meta row (committed at its step)

- Lanes: vitest **437 passed | 2 skipped + the band red** (the band: 1,808,084
  gzipped = **+654 over R0 1,807,430** — past ±512; recorded as the mid-round
  band state, the ideal close is S5's deletions pulling the net back within
  the band so no re-baseline is owed), svelte-check 0/0, eslint clean,
  e2e chromium **9 passed | 1 skipped**.
- RED-proofs (run against the pre-S2 page, then restored):
  `the_turn_meta_is_one_row` → `Unable to find an element by:
  [data-testid="chat-meta"]`; `a_streaming_turn_carries_the_row_with_its_min_height_reserved`
  → the same absence. Both green on the new tree.
- The contract held through the restyle: `chat-tier` textContent carries the
  RAW tier word; `chat-support` carries `data-support` + the full sentence as
  visible text AND `title`; the dot is aria-hidden decoration.
- Axe AA green over the generated arm (F3's judge).
- New key `chat.support.counts` ×5 catalogs (parity green).

## S3 · the empty state (committed at its step)

- The greeting: serif moment (`--font-serif: ui-serif, 'New York', Georgia, serif`
  — ONE @theme token, system stacks only, F7 clean), 28px, tracking tight.
- The disclosure: demoted to one muted 12px (`text-xs`) line — still rendered,
  still visible, still consumed in all five catalogs (F4's law).
- The chips: `variant="ghost"` — the registry's `emptyChips` untouched, the
  COUNT law held by the untouched pin.
- The composer: `turns.length === 0` → centered float (`mx-auto max-w-xl`,
  unstuck); first turn → docks sticky at the foot.
- Lanes: chat-home-state + chat-surface + route-a11y + theme-retune +
  i18n-parity all green (56 tests), svelte-check 0/0.

## S4 · the composer (committed at its step)

- The permanent hint ROW deleted: `chat.composer.hint` stays CONSUMED as an
  sr-only span bound through `aria-describedby="composer-hint"` on the textarea
  (keyboard discovery kept, the canvas loses the row). Key live in all five.
- The on-device badge: lock glyph + label in the card's right cluster,
  `data-testid="composer-posture"`, the honest posture sentence in `title` AND
  `aria-label` (the glyph law). NEW keys `chat.badge.ondevice` +
  `chat.badge.ondevice.tip` ×5 catalogs.
- The badge pin: `ui1_the_composer_posture_badge_sentence_exists_in_all_five_catalogs`
  (presence + non-emptiness + the tooltip says more than the label).
- No fake affordances added (no +, no mic, no picker — the badge states a fact).
- Lanes: i18n-parity 4 green, chat-surface/home-state/transport 27 green,
  svelte-check 0/0, eslint clean.

## S5 · THE NAMED ACT — the footer dies (committed at its step)

- RED-first (run against the pre-S5 tree, all three red, then green):
  `the_frame_renders_one_nav…healthy_rail_dot` → `expected <footer …> to be null`;
  `the_sidebar_facts…never_guessed` → the status-context tooltip arm;
  `the_model_fact_rides_the_composer_badge…` → `Unable to find [data-testid="shell-status-model"]`.
- The re-homing map (testid → new home, one line each):
  - `shell-status` (the footer) → DELETED; wire state → the rail dot's `data-wire`
    (the fault SENTENCE stays in the chat banner alone, its one home)
  - `shell-status-model` → the composer's posture badge (+page.svelte, inside
    `composer-posture`); the refusal arm's null-shape re-asserted there
  - `shell-facts` → unchanged home, gains `data-wire` + the healthy word + the
    registry's status-context in its tooltip and sr-only line
  - rows/integrity/context → the dot's tooltip (`factsTitle()`), never-"0 rows"
    law verbatim
- `refreshFacts`/`refreshChain` still run (7 references) — the facts feed the
  dot and the badge.
- Greps: `shell-status|status-bar` outside the model testid = **0** in src;
  `shell-status-model` exactly once in markup; `shell-facts` once.
- The registry unit pin (`resolveStatusChain`) untouched and green.
- Lanes: vitest 439 passed | 2 skipped (band red: 1,808,343 = +913, recorded),
  svelte-check 0/0, eslint clean, e2e chromium 9 passed | 1 skipped.

## S6 · the rail (committed at its step)

- Tooltip labels: the nav links already carried `title` = their label key;
  the three icon CONTROLS (new-chat, settings, sidebar-toggle) gain `title`
  beside `aria-label` — discoverability without text nav, no census change.
- Session rows quiet: the row is a `group`; the pencil/trash cluster reveals
  on hover (`group-hover`) and stays keyboard-recoverable
  (`group-focus-within`); the per-button opacity multiplications removed (the
  span is the single visibility control). Hover tint + active tint unchanged.
- Lanes: appshell-chrome + default-nav + route-a11y 35 green, svelte-check 0/0.

## S7 · i18n + audit (verified at the wrap)

- 3 keys added, 0 removed: `chat.support.counts`, `chat.badge.ondevice`,
  `chat.badge.ondevice.tip` — ×5 catalogs. i18n-parity green (4 tests);
  text-audit green (the new keys sit OUTSIDE the eight chrome families by
  their names — `chat.support.*` and `chat.badge.*` were never audit families;
  the audit's exhaustiveness held).
- Every demoted key STILL consumed (grep receipts): `chat.composer.hint`
  (sr-only), `shell.status.healthy` (dot title), `shell.status.model` (badge),
  `chat.recalled`/`chat.confidence[.unknown]` (recalledSentence), `chat.tier`
  (tier token title), `chat.support.*` (meta row), `shell.facts.*` (dot title),
  `support.status.context` (status context → dot title),
  `chat.disclosure.ai_interaction` (the demoted line — F4's fence, the kernel
  pin `ai_act_transparency_deliverable_is_declared` green).

## S8 · the wrap

- The band: **1,807,430 → 1,808,365 gzipped (+935)**, moved as a NAMED ACT —
  the accounting in the pin's own comment and in CHANGELOG §[Unreleased].
  Removed (DOM/structure, not bytes): the support/tier `<p>`s, the old
  evidenceDisclosure block, the answered heading + inline button, the visible
  hint `<p>`, the footer + its wireFaultMessage import. Added (the mandate):
  turn-meta.ts + the three snippets + the helpers, the badge (+LockIcon,
  factsTitle), the counts through the model, 3 keys ×5 locales. The quiet
  came from collapsing STRUCTURE — every sentence stays rendered by law, so
  the i18n payload could not shrink.
- Register: two rows appended (the meta-row law, the footer re-homing) —
  the register's own parser (`the_gate_law_register_rows_parse…`) caught the
  first draft's unescaped pipe (5 columns in a 4-column table), fixed; the
  pin green (main_suite 354 passed).
- The dry-run: vitest **440 passed | 2 skipped**, svelte-check 0/0, eslint
  clean, e2e chromium **9 passed | 1 skipped**, lipstyk-gate **green**
  (base 84310c3f, exit 0), kernel fmt green, clippy default `-D warnings`
  green, **the FULL kernel suite 4,225 passed / 0 failed** (zero kernel
  files in the diff — §3 held), badges selfcheck OK.
- CHANGELOG §[Unreleased] engineering record + the AGENTS header entry landed.

## The falsifiers, closed

- F1 (e2e reds): never — the lane ran green at S0/S2/S5/close (9 passed |
  1 skipped each time).
- F2 (the band): moved as the named act WITH the accounting (above).
- F3 (axe on the meta row): green — every token carries its sentence; the
  dot is aria-hidden with the sentence visible beside it.
- F4 (the kernel Art-50 pin): green — `ai_act_transparency_deliverable…`
  4 passed; the disclosure still rendered and visible at 12px.
- F5 (a replay row renders "0"): the omit law pinned red-first at S1; the
  counts token renders only when non-null.
- F6 (a fact deleted, not re-homed): the S5 map is in this file, every
  testid accounted; the chrome pin rewrite names each home.
- F7 (webfont/remote/new dependency): none — the serif is a system stack
  (one @theme token), no fetch, no CSP change, no new dependency (Cargo
  untouched; pnpm-lock untouched).
