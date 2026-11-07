<!-- doc: e96a071f4cba2aa3e237becdec8cae4d8384640477077de29ec6650555bf428b | source: golden | pages: 2 | tier: text-layer
     extracted: 2026-11-07T00:00:00+00:00 | extractor: pdf-extract/0.12.1 | layout: unverified -->

## Page 1


Left Column: Architecture

The server is a single binary. The HTTP layer owns the routes, the
handlers own the semantics, and the storage layer owns the rows.

Each layer is replaceable behind its seam. The router merges
sub-routers; the handlers call services; the services touch SQLite
through one pool. Nothing reaches across two layers at once.

The design keeps the memory envelope small. The hot path allocates
bounded buffers and the pools cap their connections.
 Right Column: Operations

Operands are verbs, not daemons. The operator runs a command,
reads the exit code, and closes the terminal.

Backups are file copies. Restore is the copy run backward. The
migration rehearsal replays a snapshot into a scratch database
before the live schema moves, so the upgrade path is measured,
not hoped for.

Every alert names its remedy. A page without a runbook is a
page that will page again.
<!-- anchor: p1 -->

## Page 2


Left Column: Verification

The suite is the largest artifact. Its shape is measured, its
classes are derived by a rule, and the census is re-runnable.

A pin without a control arm is a check that cannot fail. The
negative side ships with every positive claim.
 Right Column: Economics

Latency is a budget. The recall budget is single-digit
milliseconds on the edge profile, and every added stage must
state its cost before it lands.

Storage is the scarcest resource on the small boxes. The vector
store quantizes to int8 plus a binary sidecar and keeps the
float32 rows only for the backfill window.
<!-- anchor: p2 -->
