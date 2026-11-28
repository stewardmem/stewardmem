# The StewardMem shell

The shell is the product's one GUI: a **SvelteKit + Tauri** app (`shell/`).
The same build ships two ways:

- **The web seat.** The kernel serves the built shell at `/app` by default
  (`BRAIN_CLIENT_DIST`, default `shell/build`). Open
  `http://127.0.0.1:8765/app/` in a browser. See
  [Deployment](./deployment.md) for the serving posture.
- **The macOS desktop app.** `cd shell && pnpm tauri build` produces the
  StewardMem `.app`. It talks to the same kernel over the same typed wire.

The shell is a protocol client: it holds no business logic, and every read
and write is a typed kernel call generated from the kernel's
[`openapi.yaml`](../openapi.yaml). One exception exists on purpose, pinned in
review: the chat's streaming spine.

The chat is the home screen. Everything else is one keystroke or one click
away.

## Chat — the home route

The conversation is the app. Each turn shows what the kernel did with it:

- **Receipts.** An answer carries the memories it used, with the exact
  quoted span, so you can check the answer against the memory yourself.
- **The support verdict.** The kernel grades its own answer against its own
  citations (pure index arithmetic, no model in the gate) and shows one of
  four verdicts: **supported**, **partial**, **unsupported**, or
  **abstained**. An unsupported answer renders dimmed with its chip — you
  see the honest state, never a confident guess.
- **The tier badge.** The turn names the tier that served it:
  `deterministic` (the recall reader, zero model tokens), `on_device` (the
  on-device writer), `mlx_native` (the local writer tier), or the frontier
  (an explicit, audited escalation). See
  [Tiered cascade](./TIERED_CASCADE.md).
- **Sessions.** A session names itself from its first prompt; the list
  groups by day.
- **Markdown, code, and diagrams** render in place; a forget action rides
  the same surface.

`Cmd/Ctrl-K` opens the command palette (in-page — there is no OS global
shortcut). It navigates and searches chat.

## The other surfaces

Six more, each one route:

| Surface | Route | What it is |
|---|---|---|
| **Memory** | `/recall` | The recall grid over your store, and the trace viewer (`/recall/{trace_id}/trace`) that shows how one answer was built. |
| **Review** | `/review` | The write-back queue: new memories wait here for your yes. Approve-only and digest-bound (see below). |
| **Audit** | `/audit` | The keyed hash chain of every recorded act, with the verify action. |
| **Governance** | `/governance` | What the kernel enforces: registered domains, role bundles, the retention policy. |
| **Journey** | `/journey` | The customer-facing chain projection, read verbatim from the kernel. No local merge, no derived timeline. |
| **Decisions** | `/decisions` | The decision-run ledger, with one detail view per run. |

Every surface renders the kernel's own sentences or typed values. The
registry that contributes these entries is a single const array in
`shell/src/lib/extensions/` — the navigation, the palette, the chat's empty
state, and the settings sections all read from that one source.

## Review — the approve-only queue

New memories land in Review and wait for you. The card shows the scoring
breakdown (novelty, salience, and any conflict), the injection screen's
verdict (`clean` or `quarantined`), and the provenance badges the kernel
stamped on the proposal: who or what produced it (`derived_from`,
`producer_ref`).

Approve is the only write this surface offers, by law: there is no reject
and no edit here. The approve call rides the digest shown on the card — no
displayed digest, no approve. Rejection and editing stay kernel-side verbs
(`POST /proposals/{id}/reject`), not shell buttons; see
[Human in the loop](./human-in-the-loop.md) for the API-side flow and the
why.

## Settings — three categories

The settings dialog has three categories:

- **General** — the session locale (five languages: English, German,
  Spanish, French, Dutch), how StewardMem runs (the gate's one-time
  decision, re-runnable as a section), the workspace import, and the
  kernel-origin override.
- **About** — version facts and, on the desktop app, the software update.
- **Advanced** — read-only fact surfaces: the retrieval model's facts, the
  rerank tier's facts, the signal section, and the detection cards.

The kernel token section appears only in the desktop app, where the app can
hold it; the web seat never asks for it in-page.

## Languages and accessibility

Five locales ship with a parity gate (a missing key in one catalog fails
the build): **English, German, Spanish, French, Dutch**. The shell carries
the WCAG 2.2 AA lane: keyboard-operable surfaces, focus management, and the
review queue's readable states are pinned by tests.

## What the shell never does

- No business logic: a decision lives in the kernel or it does not exist.
- No second raw-fetch path: reads go through the generated typed client;
  only the chat spine streams.
- No silent fallback: when the kernel refuses, the surface shows the
  refusal, never a queue that quietly emptied.
- No offline content presented as live: cached metadata is named as cached,
  with its age.

## Testing

Seven Playwright lanes cover the shell end to end against a real kernel
(`shell/e2e/`): the chat spine, decisions, the import posture, the palette,
sessions, the trace viewer, and the WebKit engine. `pnpm test` runs the unit
pins; `pnpm test:e2e` runs the lanes.
