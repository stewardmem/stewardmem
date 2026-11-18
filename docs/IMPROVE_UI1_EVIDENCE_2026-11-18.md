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
