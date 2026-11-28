# StewardMem

Self-hosted AI agent memory. It runs on your own machine, shows where every answer came from, refuses to guess, and deletes for real.

One open-source Rust binary. One SQLite file. No cloud, no account, no subscription.

<p align="center">
  <a href="#"><img alt="Version 1.29.49" src="https://img.shields.io/badge/version-1.29.49-blue.svg"></a>
  <a href="#proof-not-promises"><img alt="4255 tests passed" src="https://img.shields.io/badge/tests-4255%20passed-brightgreen.svg"></a>
  <a href="openapi.yaml"><img alt="OpenAPI 1.29.49" src="https://img.shields.io/badge/OpenAPI-1.29.49-6b4bff.svg"></a>
  <a href="sbom/brain-server-1.29.49.cdx.json"><img alt="CycloneDX 1.5 SBOM" src="https://img.shields.io/badge/SBOM-CycloneDX%201.5-013243.svg"></a>
</p>

*The test count comes from the build, never from a hand (`scripts/badges.sh --verify-count`; not selfcheck-verified, the cheap check cannot count tests, the CI badge-drift gate does that on every release tag).*

## What you get

- **Memory that stays yours.** Everything sits on your machine in one SQLite file. Copy it, back it up, or delete it. It never leaves.
- **Answers with receipts.** Every answer shows which memory it used, with the exact quote. You can check it yourself.
- **No guessing.** If the memory is not there, it says so. It does not invent facts.
- **Deleting works.** Remove a memory and it is gone, with a record of the removal that cannot be quietly changed.
- **You hold the pen.** New memories can wait for your yes before they count. You decide what the AI keeps.

## Quick start

**On a Mac (easiest):** download the app or the tools installer from [the releases page](https://github.com/stewardmem/stewardmem/releases/latest), then run StewardMem.

**From source (macOS or Linux):**

```sh
git clone https://github.com/stewardmem/stewardmem.git
cd stewardmem
cargo build --release --features bench --bin brain-server --bin brain --bin mcp
./target/release/brain-server
```

The server listens on `127.0.0.1:8765`, reachable only from your machine. The login token lands in `~/.config/brain-server/auth-token`. On a Mac, `scripts/install-service.sh` does the whole install for you.

**Connect your tools:** the `mcp` binary is an MCP server, so Claude Desktop, Cursor, Windsurf and other MCP clients can use the memory directly. Anything else can talk to the REST API.

## The pieces

| Piece | What it is |
|---|---|
| `brain-server` | The memory server. REST API with streaming chat, on your machine. |
| `brain` | The command line tool. Ask, add, verify, back up. |
| `mcp` | The MCP server for AI clients. |
| StewardMem.app | The macOS desktop app and the web seat at `/app` (one SvelteKit + Tauri shell): chat first, then memory, review, audit, governance, journey, and decisions. See [the shell guide](docs/shell.md). |
| OpenClaw plugin | Memory for OpenClaw agents, checked before every prompt. |

Finding things works three ways at once: by meaning, by keyword, and by the links between memories. The best hits become the answer's evidence. Recall costs zero tokens and needs no outside service.

## Who it is for

Anyone who must prove what their AI did. Schools and universities. Offices that handle other people's data. Regulated work. When someone asks "what did the AI store, and where did that answer come from", this server answers with evidence.

## Proof, not promises

- The test count in the badge above is real, counted by the build.
- Every write lands in a keyed hash chain (HMAC-SHA256), a trail that cannot be quietly rewritten. `brain verify` checks the whole chain.
- The SBOM and the OpenAPI spec in the badges are version-matched to the release by CI.
- UMP 1.0 L3 conformance is checked by a CI gate (ci.yml), not self-attested.

## Learn more

- [Full documentation](https://stewardmem.github.io/stewardmem/) for when you want the depth
- [Security policy](SECURITY.md) and [support options](SUPPORT.md)
- [Compliance notes](COMPLIANCE.md): GDPR, the EU AI Act, and the Philippine DepEd AI guidelines
- [Benchmarks](BENCHMARKS.md)

## License

Apache 2.0. See [LICENSE](LICENSE).
