# docling-goldens — the two candidates' contract outputs (IMPROVE06 S1)

One golden per candidate implementation of
`docs/DOCLING_TOOL_PROTOCOL.md`, each produced by the S1 preflight probe on
`tests/fixtures/documents/two-column.pdf` and committed FROZEN AT COMMIT —
the same discipline as `../ocr-real/`. The directory name records the exact
converter version that produced the bytes:

| Directory | Candidate | Tool | Version | Produced by |
|---|---|---|---|---|
| `docling-py-2.148.0/` | A — Python Docling | `scripts/docling-kernel.py` | docling 2.148.0 | the operator venv (`.venv-docling`), artifacts pre-fetched at `~/.cache/docling-artifacts` (1.3 GB), macOS 27.2.0 arm64 |
| `docling-rs-2.67.1/` | B — docling.rs | `scripts/docling-rs-kernel.sh` | docling-rs 2.67.1 | the prebuilt aarch64-apple-darwin binary beside its `.models/` (the models-v1-f8a5d4fd8b08 release, SHA256SUMS-verified), ONNX Runtime 1.31, macOS 27.2.0 arm64 |

Each directory holds the contract's two files: `doc.md` (pages separated by
the literal `<!-- page-break -->`) and `meta.json`
(`{"tool","version","pages"}`).

**The bytes are the converters' own output, accepted as produced** (DC-2) —
including the cue-loss artifacts the S3 table measures honestly: on this
synthetic two-column fixture both converters merge paragraphs to single
lines and glue headings into paragraph bodies; candidate B additionally
displaced one line above its column heading (recorded in
`IMPROVE06_EVIDENCE_2026-11-08.md`, never corrected). A re-run of the same
tool version reproduces these bytes (deterministic model inference, the
determinism tuple: same bytes + same tool version + same models + same
machine class); a NEW tool version is EXPECTED to drift — re-run the probe
and re-commit under a new version-named directory, never overwrite.

The golden round-trip is tool-gated: the kernel-side test re-runs the
candidate ONLY where the tool + artifacts are installed and skips honestly
elsewhere (the Linux/macOS dual, the improve05 pattern) — the absence path
is the CI-green path everywhere.
