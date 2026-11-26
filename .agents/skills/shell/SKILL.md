---
name: shell
description: The Tauri 2 + Svelte 5 desktop shell and the openclaw plugin — typed wire client, SSE chat spine, i18n, settings registry, e2e. Load before editing shell/.
---

# Shell

**Stack** (verify in `shell/package.json`): Tauri 2.12, Svelte 5.57 (runes),
SvelteKit 3.2 (static adapter), Tailwind 4.3, shadcn-svelte 1.8, zag-js 1.47,
paraglide 2.29, Vite 8, Vitest 5, Playwright 1.65, Node 24, pnpm 12.
TypeScript 6.0 stable; `@typescript/native` 7.0.2 drives `svelte-check --tsgo`.

**Rules:**

- The shell is a protocol client: zero business logic, every read a typed
  kernel call. `openapi-fetch` + generated `schema.d.ts`.
- One wire exception, pinned: the chat SSE spine (`src/lib/chat/spine.ts`).
  Do not add a second raw-fetch path.
- Settings sections are an exact registry set; adding one updates the census.
- i18n: five catalogs, parity gate. Design for +40% string growth.
- The bundle band is measured; a size move is a named act.

**Gates:** `pnpm test`, `pnpm check` (svelte-check --tsgo), `pnpm lint`,
`pnpm test:e2e`, plus the bundle pin.

**OpenClaw:** brain-server is the memory backend; the plugin calls `/recall`
via `before_prompt_build`. Config at `~/.openclaw/openclaw.json`.
