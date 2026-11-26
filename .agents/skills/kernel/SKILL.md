---
name: kernel
description: brain-server Rust kernel work — service cores, handler adapters, SQL placement, audit-per-write, wire contract, gate inventory. Load before editing src/.
---

# Kernel work

Edition 2024, rust-version 1.98.

**Layers:** `src/server/router/**` (routes) → `src/handlers/**` (adapters) →
`src/service/**` (SQL + invariants) → `src/workflow/**` (legacy, extract on
touch). `main.rs` is wiring only.

**Handler shape:** parse → `OptPrincipal` → `authorize`/`authorize_role` → one
`spawn_blocking` → service core → read-seam shaping → response. No SQL, caps,
or FK ordering here.

**New code:** a service core in `src/service/<domain>.rs` taking `&Connection`
or `WorkflowTx` — never `Pool`, `AppState`, or axum types. Audit row in the
caller's tx via `record_tenant` (SAVEPOINT-nested). Caps are Rust-side pure
arbiters, pinned by a test; SQL never decides a row's fate.

**Wire:** a route change ships `openapi.yaml` + route-coverage + route-authz
rows in the same commit. Regenerate `shell/src/lib/api/schema.d.ts`; the drift
gate byte-compares it. `x-api-version` moves only when the wire shape moves.

**Tests:** `cargo test --features bench`. Use `.expect()` in tests — lipstyk
rejects `Box<dyn Error>` test bodies. Write the pin first, watch it fail, then
fix. `cargo test` stops at the first failing binary; use `--no-fail-fast` when
hunting re-pins.

**Gates:** `no_sql_in_handlers_enforced`, `dup_guard`, `spire_inventory`,
`reg_watch`, `docs_truth`, `feature_lane_pins`. Add a feature ⇒ add its CI
lane; `tests/feature_lane_pins.rs` derives the list from `Cargo.toml`.
