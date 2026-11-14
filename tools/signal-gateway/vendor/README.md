# vendor/ — the two pinned-crate overrides closing RUSTSEC-2026-0459 + 0502

`libsqlite3-sys` 0.36.x bundles a C SQLite carrying CVE-2026-119375
(RUSTSEC-2026-0459, medium 6.8; patched in ≥0.39.0), and its
`bundled-sqlcipher` engine rides `sqlcipher-src`, whose 419.x carries the
same CVE (RUSTSEC-2026-0502; patched in 420.0.0 = SQLCipher 4.20.0 /
SQLite 3.54.1). Two crates in this tool's graph cap the version below the
patch line, and neither can move upstream:

| crate | version | cap | why it cannot move upstream |
|---|---|---|---|
| `sqlx-sqlite` | 0.9.0 (crates.io, the newest of its line) | `>=0.30.1, <0.38.0` | no newer 0.9.x exists; the req is upstream's |
| `presage-store-sqlite` | 0.8.0-dev (whisperfish/presage rev `6f630791`, the rev this tool pins) | `"0.36.0"` (^0.36 → <0.37) | **upstream `main` still pins `"0.36.0"` explicitly** (verified 2026-11-14) — moving the rev does not lift the cap |

So this directory vendors BOTH crates as **byte-identical upstream source
with exactly one manifest line changed each**, wired in via `[patch]` in
the tool's `Cargo.toml`. The copies are compile artifacts of the same
source the lockfile resolved — zero code deltas.

## The deltas (the ONLY edits)

- `vendor/sqlx-sqlite/Cargo.toml`: `[dependencies.libsqlite3-sys] version
  ">=0.30.1, <0.38.0"` → `"0.40.0"`. Source: crates.io `sqlx-sqlite-0.9.0`
  (the normalized registry manifest — it is standalone, no workspace
  inheritance). License MIT OR Apache-2.0 (LICENSE-APACHE / LICENSE-MIT
  carried in the copy).
- `vendor/presage-store-sqlite/Cargo.toml`: `libsqlite3-sys "0.36.0"` →
  `"0.40.0"`, and the sibling path dep `presage = { path = "../presage" }`
  → the same git rev this tool's manifest pins (a vendored copy has no
  `../presage`). Source: the checkout of
  `whisperfish/presage@6f630791b2c7bf3d5b00c75f5a6c94131bf79c8c`
  (`presage-store-sqlite/`). License AGPL-3.0-only — same as this tool
  (see the tool's own license note); the upstream `LICENSE.md` lives at
  the pinned checkout root and governs this copy.

## The resolved chain (why 0.40.0 and not 0.39.x)

`libsqlite3-sys 0.39.x` pins its SQLCipher build dependency EXACTLY
(`sqlcipher-src = "=419.0.6"` — the unpatched engine), so stopping at
0.39 would leave RUSTSEC-2026-0502 standing one level deeper.
`libsqlite3-sys 0.40.0` pins `sqlcipher-src = "=420.0.0"` (SQLCipher
4.20.0 / SQLite 3.54.1 — both advisories patched), so BOTH vendored
manifests target 0.40.0 and the graph resolves entirely from crates.io
with zero vulnerabilities.

## Re-diffing (the drift guard)

To re-sync after an upstream move: copy the new source over the vendor
dir, re-apply the one-line delta(s) above, and `git diff` must show
NOTHING else. If the delta no longer applies cleanly (upstream changed
the line), the pin has moved and this override must be re-evaluated —
upstream may have lifted the cap themselves, at which point the vendor
dir and the `[patch]` entries are DELETED.

## When to delete this directory

The moment presage upstream pins `libsqlite3-sys >= 0.39` (or sqlx
ships a 0.9.x with the range widened past 0.39), the vendored copies and
the `[patch]` entries go away and the graph resolves upstream like every
other dependency in this tree. Until then the copies stand: the advisory
is real, the tool compiles the bundled engine, and "recorded by name" is
not a fix.
