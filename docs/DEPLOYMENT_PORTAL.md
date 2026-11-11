# The Thin Customer Portal — the deployment record (IMPROVE15)

**Status (2026-11-12):** the skeleton ships (`src/bin/portal.rs` + `src/portal/`). The auth
decision is §3(c) of the round's plan, preregistered: **the kernel never becomes an identity provider** — customer auth lives in the portal; the kernel verifies the portal's per-client
roster credential exactly as it verifies any principal. The alternatives recorded in the plan
stand: (a) IdP-borne federation is the verticals' upgrade path, (b) kernel-issued customer
credentials are REFUSED (the kernel as IdP puts customer passwords near the memory server and
widens the auth surface v1.29.x hardened).

## The one-portal-per-client posture

A portal instance serves exactly ONE client (`BRAIN_PORTAL_CLIENT`). The blast radius of a
portal compromise is that client's read-only view — the credential the portal holds is scoped
to one domain, read-only, tools-empty, and the journey it can read is the client's own
projection.

## §8.1 access review — the per-client principal's grant, enumerated

The portal's kernel credential is minted through the roster door, one principal per client:

```sh
brain principal grant \
  --name portal-<client> \
  --role client-auditor \
  --domain <client-domain> \
  --out <secrets>/portal-<client>.token
```

The grant, line by line (the access review a deployment re-audits at onboarding):

| Field | Value | Why |
|---|---|---|
| role | `client-auditor` | the ship-with read-only one-domain preset; `can: ["read", "shared_pool"]` — NO write verb, NO approve, NO admin |
| domain | the client's ONE domain | the journey's own authz law scopes the read to the client's own pool; the scope's single domain IS the client's own-principal match |
| tools | EMPTY (the grant's default — no `--tools` flag) | the day-zero law: no UMP/MCP tool until the operator names one; the portal needs no tools (it proxies two customer doors + one read) |
| manages | EMPTY | the portal manages nothing; a reports-scoped read is not its business |
| ttl | optional `--ttl-days` | a deployment with a rotation calendar pins one; rotation = re-grant (the name rotates) + a portal restart |

The onboarding access review re-checks: the token file's mode (0600, the mint writes it), the
roster row's domain == the client's domain, `tools` empty, and that the client's journey
endpoint answers the existence-blind refusal to every OTHER principal (witnessed in
`tests/authz_matrix.rs::client_auditor_journey_rows` and the curation battery).

## The portal's environment (all required at boot — fail closed)

| Variable | Meaning |
|---|---|
| `BRAIN_PORTAL_BIND` | the listen address (default `127.0.0.1:8766`) |
| `BRAIN_PORTAL_KERNEL_URL` | the kernel origin (default `http://127.0.0.1:8765`) |
| `BRAIN_PORTAL_KERNEL_TOKEN` | the minted per-client credential (server-side only — never rendered, never set as a cookie) |
| `BRAIN_PORTAL_CLIENT` | the one client this instance serves |
| `BRAIN_PORTAL_PASSCODE` | the per-client access code (rotate = restart with a new value; the in-process sessions die with the process, so rotation is total) |

## The session law

Sessions live IN THE PORTAL PROCESS: an in-memory map of opaque ids to expiry (30 minutes),
minted on a passed passcode, dropped at logout, dropped at restart. The kernel carries no
customer session store and no customer login endpoint. The browser's only cookie is
`portal_session` (HttpOnly, SameSite=Lax) — an opaque id into the portal's own map.

## The offline law

When the kernel is unreachable the portal renders the named offline state: counts and ages
from the in-process metadata cache (case counts, event counts, the domain-act count). The
cache's type carries no field that could hold a sentence — metadata never becomes content
(the III-6 lesson, reused at customer scale).

## The propose door

Every customer action files through the kernel's customer doors as a PROPOSAL:
`POST /clients/{name}/appeals` (the digest-bound appeal) and `POST /clients/{name}/requests`
(the DSAR/erase request). The portal is a proxy for exactly those two writes — it writes no
memory, holds no direct write path, and nothing a customer does auto-reverses anything (the
operator disposes through the existing review queue; the dispose rides
`hitl_decision_events`).

## What the skeleton deliberately is not

- Not an identity provider (IdP): no federation, no password store, no account recovery. A
  deployment with a real IdP moves to posture (a) — the portal's session check becomes the
  IdP's assertion — with no kernel change.
- Not a memory writer: the portal cannot touch memory rows; every verb is a read or a
  proposal filing.
- Not a second projection: the portal renders the kernel's journey sentences verbatim; a
  portal-side paraphrase would be a second vocabulary drifting from the first.
