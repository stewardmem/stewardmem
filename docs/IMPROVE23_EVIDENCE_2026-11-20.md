# UI2 "The just-works surface" — Evidence (2026-11-20)

Round 23. Thirteen steps, one commit each, every pin red-first. The gate
(§0 of the execution prompt) passed on the pre-round tree: HEAD `5a31ae34`
(the v1.29.46 front-door wrap), no `hiddenTitle`, the model-id span present
at +page.svelte:1443, "carry real data" at en.json:521, 238 dash-bearing
catalog values (49 en / 51 de / 49 fr / 40 es / 49 nl), the plugin pinned at
`^4.6.0`, zero searching-line and zero group keys.

## Research (fetched 2026-11-20, the protocol's citations)

- **Tauri v2** (Context7 `/websites/tauri_app`, reference/config):
  `hiddenTitle` — "If true, sets the window title to be hidden on macOS."
  `titleBarStyle` guidance unchanged: with `overlay` the page extends under
  the title bar and only the traffic lights remain; leave room and pair with
  a drag region. Confirms S1's one-line fix and that the `title` value keeps
  serving Mission Control and accessibility.
- **Tailwind CSS v4** (Context7 `/websites/tailwindcss`, upgrade guide +
  box-shadow): `ring` is now 1px / `currentColor` by default; `ring-2` sets
  2px; ring colours resolve against `--color-*` theme tokens — the repo's
  `@theme inline` maps `--color-sidebar: var(--sidebar)`, so `ring-sidebar`
  resolves. S5's classes verified before writing.
- **shadcn-svelte** (Context7 `/websites/shadcn-svelte`, button docs): the
  installed button carries variants default/secondary/outline/ghost/
  destructive/link; `secondary` is documented as "used for less prominent
  actions" — the quiet pill S6 wanted, confirmed against the tree's own
  `button.ts` (bg-secondary, no border).
- **Paraglide / inlang** (Context7 `/generaltranslation/paraglide-js`,
  variants.md): plural selectors exist, but exact-number matching (`=0`,
  `=1`) needs un-annotated locals, which need a message-format plugin newer
  than the tree's `^4.6.0` (opral/inlang#4440). The import fix therefore
  uses agreement-free noun copy (the plan's default), zero plugin risk.
- **Svelte 5**: templates match the tree's own runes patterns throughout
  (`$derived.by` for the grouped runs, snippets untouched); the one lint
  finding (`svelte/prefer-svelte-reactivity` on a mutable Date) resolved by
  moving the boundary math to a plain module, not a disable.
- **Web, the benchmark study**: the Codex desktop community's top complaint
  about session history is the flat ungrouped sidebar ("a filter, a
  grouping, or a per-row origin indicator would make the sidebar navigable
  again" — community.openai.com, 2026-07); Claude Code's UI is praised for
  live status indicators while it works ("status indicators showing when
  it's reading files… it feels responsive and alive"). Our S11 (date
  groups), S12 (palette chat search) and S9 (the searching line) land
  exactly those conventions. No brands, names, colours or literal copy
  taken.
- **Web, macOS HIG** (developer.apple.com, toolbars + sidebars): toolbar
  items belong in leading/center/trailing regions; sidebars sit on the
  leading side; show/hide toggles belong in the View menu; settings windows
  carry the standard ⌘, shortcut. Used for the Tier C list at the wrap.

## Steps: before → after

| Step | Before | After |
|---|---|---|
| S1 | Overlay titlebar painted the centered native "StewardMem" over the first session row (`titleBarStyle: Overlay`, no `hiddenTitle`) | `"hiddenTitle": true` beside the style; the paint is gone, the `title` value stays for Mission Control + a11y. Pin: `the_macos_window_hides_its_native_title_and_keeps_the_accessible_one` (red: flag absent) |
| S2 | The composer badge printed `/stats`'s `model` (the retrieval embedder `minishlab/potion-retrieval-32M`) beside "On-device" | The badge states posture only: lock + On-device + the tip sentence. `KernelFacts.model`, the `shell-status-model` span and the `shell.status.model` key (×5 catalogs) are gone. Pin: `the_badge_carries_no_model_id_even_when_stats_answered_one` (red: planted span) |
| S3 | 238 catalog values carried em/en dashes (the AI house style); the Art 50 disclosure among them | Zero dashes in any value, any locale: colons for fragment pairs (French keeps its space before the colon), period splits for the disclosure (the plan's own en wording, meaning intact) and the updater's current-version line. The pin `ui2_the_plain_language_law_holds_in_all_five_catalogs` red 238× pre-sweep; the planted-dash/seamless control proves the checker bites; the kernel's ai_act pins stay 7/7 |
| S4 | `{count} of these files are tiny empty artifacts; {real} carry real data.` read "1 carry" at real=1, in all five locales | `{count} tiny empty artifacts. {real} with real data.` — two noun phrases, no verb to disagree. Pin: `ui2_the_import_disclosure_is_agreement_free_at_every_count` (red on the old copy; the verb list names the old verbs per locale) |
| S5 | The review badge's count could smear into the icon on glass | `ring-2 ring-sidebar` on the badge (v4 semantics verified). Badge pins untouched, green |
| S6 | The three empty-state chips were ghost — real buttons that read as plain text | `variant="secondary"` (shadcn's "less prominent actions" pill). The three-chip count pin green without amendment |
| S7 | On macOS glass the active/hover row fills fell through to the scheme defaults (opaque hex): painted chips over vibrancy | Light block: `rgba(0,0,0,.08)` active / `.05` hover; dark block: `rgba(255,255,255,.10)` / `.06`; the Reduce-Transparency blocks restate the opaque set (the existing restatement pattern). Measured on the built app in both schemes; the contrast audit (scheme blocks) unaffected; the ok-dot law rides `--sidebar`, unchanged |
| S8 | A multi-line draft's last line sat ~8px from the composer card's bottom edge | `pb-4` on the textarea. No pin change |
| S9 | The send→first-arrival window rendered "Recalling…" in answer-sized type with no role | One muted `role="status"` line, `chat.searching` ×5 ("Searching your memory…" and its translations), replaced by the arriving turn. `chat.working` keeps its verify-flow consumer. Pin: `the_pending_window_announces_the_search_and_the_turn_replaces_it` (red: no testid/role under the old rendering; the delayed-grounding stream proves the replacement) |
| S10 | The kernel's lazy create wrote `label=''`; every row read "Conversation" | `first_prompt_label`: the SCREENED prompt's head, ≤40 whole chars, ≤80 bytes (CJK binds at the byte cap: 26 chars/78 bytes), invisibles stripped, trimmed; empty stays empty; second turns never rename; the rename verb untouched. Pin: `a_created_session_names_itself_from_the_first_prompt` (red: the empty-label create planted back, stale-vs-derived pair); the sessions e2e gains the wire assertion and finds rows by self-derived labels |
| S11 | The session list repeated per-row relative times down the column | Rows group under Today / Yesterday / Previous 7 days / Older (local day, one header per contiguous run, keys ×5); the visible row carries only the name; the time stays sr-only + in the row's title. Pins: `session_rows_group_by_date_under_locale_aware_headers` (red) + the amended relative-time pin (sr-only + title). The boundary math lives in `shell/local-day.ts` (a plain module — the Svelte reactivity lint forbids mutable Date instances in components/runes modules) |
| S12 | The palette had no way to reach a past conversation by name | A Chats group (between Navigate and More) over the store the frame already holds — the glance refresh at palette open is the read, no new cadence; Enter activates. `palette.chats` ×5 (a declared loanword in de/nl). Pin: `the_palette_chats_group_filters_by_label_and_enter_activates` (red: no group) |
| S13 | Five settings categories; the About pane was a two-row dl with an em-dash unknown placeholder | Three: general, about, advanced. model+rerank fold to advanced (failure sentences keep their home), update folds under about's new ONE version line ("StewardMem {app}. Kernel {kernel}." with the honest "not reported" word — the dash placeholder dies). runmode STAYS in general (the move is the operator's Tier C sub-decision). Token input gains a fixed bullet placeholder when configured (the value never echoes); locale/import selects gain `max-w-xs`. Dead copy retired with the fold (cats.model, cats.updates, about.kernel ×5 + audit rows + loanword entries). Pins: the registry's three-category law (red: retired categories named) + the rerank registry arm (red on 'model') + the mas updater arms reach through About |

## The bundle band (a named measurement, no move)

Measured at the wrap with the pin's own routine: **1,808,836 bytes
gzipped** (150 JS files) against the UI1 midpoint **1,808,365** — a delta
of **+471, inside the ±512 refactor band**. The additions (the searching
line, 4 group keys + the palette group + the about keys ×5 locales, the
grouped session markup) are nearly cancelled by the S2/S13 removals (the
badge span, `shell.status.model`, three dead settings keys ×5). The band's
midpoint does NOT move this round; the number is recorded here as the
measurement of record.

## Lanes at the wrap

- shell vitest: **450 passed | 2 skipped** (59 files; UI1 baseline 443+2;
  +7 = S1, S3×2, S4, S9, S11, S12 pins). Zero unhandled errors after the
  S13 mock completion (the Advanced pane now mounts the model/rerank
  sections — their reads joined the mas-surface/token-keychain
  kernel-facts mocks).
- svelte-check: **0 errors / 0 warnings**. eslint: clean.
- e2e: **10 passed | 1 skipped** (chromium + webkit). The skip is the
  sessions drill: this machine's AFM bridge answers the named
  `chat_bridge_unavailable` posture (the recorded Apple-Intelligence
  machine state, the R149 class) — a LOUD skip by the spec's own law. The
  S10/S11 e2e amendments are therefore witnessed by the vitest pins + the
  kernel module pin here; their first live-wire run rides the next
  bridge-available session (owed, named).
- kernel: **cargo test --features bench: 2855 passed / 1 failed → green
  after the census re-derivation** (the one red was the census table's own
  drift refusal catching the round's +1 pin — the process working). Final:
  2858 lib incl. the census pin, ai_act 7/7, memform 23/23, docs_truth
  20/20, external_claim_pins 14/14. fmt clean; clippy 0 warnings.
- census re-derived (4319 → 4320; the new pin reads
  residual_documentation under the classifier's rules); badges selfcheck
  OK; `--verify-count` reconciles at **4244** (README updated by the
  tool).
- check-doc-links: 567/567 resolve.

## The plain-language law's origin

The operator's directive, 2026-11-20: no AI language in the UI, ever — no
em-dash or spaced en-dash in any user-facing string in any locale, no
hot-air vocabulary, short sentences, plain words. S3 swept the catalogs
(238 values) and the pin (`ui2_the_plain_language_law_holds…`) makes both
halves permanent, with per-locale cognate lists that start empty and grow
only by named act. All copy written this round (the searching line, the
group headers, the import sentence, the about line) passed on its first
commit.

## Tier C (asked at the wrap; the answers or silences ride the ledger)

1. A native settings window with the standard ⌘, shortcut (today's page
   shape is the operator's recorded 2026-10-30 call; the HIG names ⌘, the
   standard. Cheap middle: auto-collapse the threads column while
   /settings is active).
2. An alt-gated developer area (hiding the whole advanced category behind
   the option key or an About toggle — hides recovery tools; the
   operator's call).
3. Sidebar toggle relocation to the top-left toolbar zone (HIG: toolbar
   regions are leading/center/trailing; needs drag-region care).
4. Brand square removal from the rail (taste).
5. A native "Check for Updates…" app menu item (feasible with the existing
   menu plumbing; a new surface, so gated).
6. An additive `chat_model` field on /stats so the badge can name the true
   writing tier (a wire addition: openapi, drift gate, schema regen — only
   worth it if identity returns to the badge).

S13's sub-decision — runmode moving to advanced — defaults to STAYS IN
GENERAL (the trust statement stays on the front page); silence keeps the
default.

## OWED, named

- The sessions e2e's first live-wire run on a bridge-available machine
  (the S10/S11 amendments' live proof; this machine's bridge refused, the
  recorded posture).
- The S7 glass literals were measured on the built app in both schemes;
  the Reduce-Transparency restatement is structural (the opaque values are
  the scheme defaults), and a live Reduce-Transparency toggle check rides
  the same bridge-available session if desired.
