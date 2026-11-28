# brain-server — agent instructions

**Release:** v1.29.50 (2026-11-28) · notes in `CHANGELOG.md` §[1.29.50].
Round history is `docs/AGENTS_HISTORY.md` (append-only, load on demand).
New rounds are appended there — not here.

**Start every session with `scripts/repo-brief.sh`** — versions, HEAD, dirty
paths, guard inventory, stale-marker probe. Under a second.

## Environment truths (not discoverable from the tree)

- macOS dev machine. The kernel is a launchd service on `127.0.0.1:8765`
  (`com.brain.server`, KeepAlive, RunAtLoad).
- On this deployment the **served store is `~/.openclaw/workspace/brain.db`** —
  the same store the workspace uses. Not a copy.
- Newly copied macOS executables carry `com.apple.provenance`; Gatekeeper
  SIGKILLs them on first exec (exit 137). `scripts/install-service.sh` strips
  it; manual `cp` does not.
- Private-repo Actions are disabled (billing). **The `v*` tag is the CI
  trigger**; main is never pushed public. Nothing publishes unless the public
  matrix is green for the tagged SHA.
- Customer-domain assets and StewardMem engine IP live in private repos.
  Never commit them here — this repo is public.

## Non-standard practices (the code cannot show these)

- Handlers are adapters; SQL lives in service cores. Enforced by
  `no_sql_in_handlers_enforced` — any statement under `src/handlers/**` fails CI.
- `main.rs` is wiring only (≤300 lines, no `cfg(test)`); routes live only under
  `src/server/router/**`.
- Every write audits inside its own transaction. Error paths deny loudly;
  `let _ =` on a write is forbidden.
- Every emitted text field passes `sanitize_read`; untrusted content is
  screened at write time; LLM-facing payloads are fenced.
- A wire change ships `openapi.yaml` + route-coverage + authz rows in the same
  commit. `x-api-version` moves only when the wire shape moves.
- Never hand-type a test count. `scripts/badges.sh --verify-count` derives it
  and refuses on drift.

## Before any push

The local suite does not cover CI. Run the gate suite — commands, the CI
dry-run, and the release ritual are in `.agents/skills/release/SKILL.md`.

## Open commitments

- **v1.29.49** — the claim gate should also scan the newest changelog section.
- **SI0 S3–S7** — the sign service core, the four routes, the wire, the drill, and the close; the registry stays empty until a sign passes every law.
- **c0-close S4–S7** — the remaining direct bumps (sha2, model2vec-rs,
  jsonwebtoken, ureq), the unused-dep scan, the lipstyk scoping decision,
  and the round's own close section are ASKED and OWED (the S0 baselines
  in `/tmp/c0close/` serve them).
- **UI2** — the sessions e2e drill proof rides the next bridge-available session.
- **R152** — the next stage run exercises the cryptex guard and the arm64 build
  natively; the site copy syncs to the front door's voice.
- **R149/R150** — the operator's first real workspace import is ASKED and OWED.
- **R148/R145/R144** — Windows presence, the x86_64 Tools variant, the arm64-VM
  clean-machine witness, and the release lane's Tools-step e2e.

## History

Per-release detail: `CHANGELOG.md` §[version]. Per-round detail:
`docs/AGENTS_HISTORY.md` (load on demand). Nothing older than the open
commitments above is kept here.
