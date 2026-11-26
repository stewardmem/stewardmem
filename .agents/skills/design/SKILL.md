---
name: design
description: UI, UX, and copy for StewardMem surfaces — the quiet-surface and plain-language laws, accessibility, motion, and layout. Load before any UI or user-facing copy.
---

# Design law

**The answer is the screen.** One accent, receipts on every claim, nothing else.

**Copy (plain-language law):**

- No AI language in the UI: not "AI", "model", "assistant", "prompt".
- Short sentences. No em-dashes, no exclamation marks, sentence case.
- FR keeps its space before `:` `!` `?`.
- Errors name the fault and the next action — never a raw code alone.
- Empty states name what is missing and what to do.

**Visual:**

- System serif for the greeting only; UI sans elsewhere. 11–12px muted for meta.
- 8px spacing scale, one column, no nested cards, no decorative gradients.
- Dark mode via tokens only — no hardcoded hex.

**Accessibility (WCAG 2.2 AA):**

- 4.5:1 text, 3:1 UI, visible focus, targets ≥24px.
- `role="status"` for async state; respect `prefers-reduced-motion`.
- Every control keyboard-reachable; the raw tier/state stays in textContent.

**Motion:** 150–250 ms, ease-out, no bounce.

**Conflict rule:** this overrides personal "avant-garde" taste. The product is
quiet, not loud.
