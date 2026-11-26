---
name: release
description: Cut a brain-server release — the six-part wrap, version lockstep, CI dry-run, tag flow, and the failure modes that have bitten. Use when bumping a version or tagging.
disable-model-invocation: true
---

# Release

**Six artifacts:**

1. `Cargo.toml` version + `Cargo.lock` lockstep (exactly one line).
2. `openapi.yaml` `version` + `x-api-version`; regenerate the typed client.
3. `CHANGELOG.md` `## [x.y.z]` + Engineering record naming every range commit.
4. `docs/roadmap.md` current-status names the release.
5. README badges via `scripts/badges.sh` (never hand-typed).
6. `AGENTS.md` release line; the round record goes to `docs/AGENTS_HISTORY.md`.
   SBOM: `sbom/brain-server-<version>.cdx.json`.

**Gate suite before push:**

```sh
scripts/repo-brief.sh
cargo test --features bench
cargo clippy --all-targets --features bench -- -D warnings
cargo fmt --check
scripts/lipstyk-gate.sh
export RUSTFLAGS="-D warnings"
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --manifest-path crates/Cargo.toml --all-targets
cargo clippy --manifest-path crates/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/steward-harness/Cargo.toml
cargo clippy --all-targets --features otel -- -D warnings
cargo test --all-targets --features otel
scripts/badges.sh --verify-count
scripts/badges.sh --selfcheck
```

**Tag flow:** `scripts/release.sh vX.Y.Z` pushes the tag; the public matrix runs
CI; `release.yml` publishes only on green for the tagged SHA. The lane also
re-derives `schema.d.ts` and refuses a mismatch (the 1f drill).

**Known bites:** a dirty tree refuses the manifest; a stale tag-era client;
`cargo test` stops at the first failing binary; the docling goldens flake under
RAM contention (rerun solo).
