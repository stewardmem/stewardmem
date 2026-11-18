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
