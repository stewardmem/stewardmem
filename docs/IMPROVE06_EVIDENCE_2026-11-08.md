# IMPROVE06 EVIDENCE — 2026-11-08 (the Docling optional tool, the Tier-2 converter)

Plan: `plans/improvements/06_DOCLING_OPTIONAL_TOOL.md` (plans repo), its §0.2
the ground record; the execution prompt of the same date is the instruction
record. Commits: S1 `ded0ae95` (the protocol + both candidates + the fixtures),
S2 `3c63ffd9` (the tier live, red-first) + the timeout-topology fix, S3/S4 this
commit. Base: brain-server `2313a1a6` (improve05's close-out).

## 1 · The tool protocol and the two candidates (correction 10, measured)

The kernel defines a language-agnostic contract
(`docs/DOCLING_TOOL_PROTOCOL.md`): `<tool> <in.pdf> <out-dir>` → `doc.md`
(pages separated by the literal `<!-- page-break -->`) + `meta.json`
(`{"tool","version","pages"}`); exit 0 ok · 3 failed · 4 self-timed ·
5 remote-refused. `BRAIN_DOCLING_PATH` names whichever executable honors it —
the kernel never knows the language. TWO candidates ran the S1 preflight on
`two-column.pdf`; BOTH passed and BOTH goldens are committed version-named
under `tests/fixtures/documents/docling-goldens/` and re-ran **byte-identical**
on the recording machine:

| | Candidate A — `docling-py` | Candidate B — `docling-rs` |
|---|---|---|
| Implementation | `scripts/docling-kernel.py` (the committed wrapper) | `scripts/docling-rs-kernel.sh` (the committed shim) |
| Engine | Python Docling **2.148.0** (docling-core 2.106.0, docling-ibm-models 4.0.4, docling-parse 7.24.2, Python 3.15.2) | docling.rs **2.67.1** (crate `docling`, released 2026-11-06; ONNX Runtime 1.31) |
| Posture | the operator venv (`.venv-docling`, ~2 GB with torch) + a 2-line venv shim the kernel spawns | one static binary (54 MB, prebuilt aarch64-apple-darwin) + `.models/` beside it (752 MB, SHA256SUMS-verified from the release `models-v1-f8a5d4fd8b08`) |
| Artifacts | 1.3 GB at `~/.cache/docling-artifacts` (`docling-tools models download --output-dir` — the plan's `--path` flag does not exist on 2.148.0; recorded) | the same models as ONNX (RT-DETR layout, PP-OCRv6, TableFormer); binary-relative `.models/` resolution verified from any CWD |
| Page breaks | `export_to_markdown(page_break_placeholder=MARKER)` — verified in the installed signature | `--page-break-placeholder` — **documented in 2.67.1's `--help`** (the plan's "undocumented" worry resolved by probe) |
| Page count | `len(doc.pages)` (the converter's own model) | brace-depth count of the JSON's `pages` object — independent of the markers, so DC-5's cross-check has two sources on both sides |
| Cold conversion (two-column.pdf, 2 pp) | ~14 s | ~1 s |
| Cold conversion (CPG subset, 6 pp) | 29.5 s | ~2 s |

The preflight caught a real wrapper bug: Python `convert()` **raises** on a
refused document rather than returning a failed status — garbage bytes
produced an unhandled traceback; the wrapper catches it into a bounded
one-line exit 3. The plan's PARTIAL_SUCCESS note is enforced: only
`ConversionStatus::SUCCESS` passes.

## 2 · The four-way fidelity table (S3, the measurement-first law)

All three text lanes on the SAME four fixtures (the ocr lane's rows extend
plan-05's WER table unchanged — see §5); WER via `scripts/wer.py` against the
generator-emitted `expected-text/` references; the structural rows are the
honest qualitative record. **The sanctioned default is decided here.**

### WER (word error rate; lower is better)

| Fixture | Tier-0 (`pdf-extract/0.12.1`) | docling-py 2.148.0 | docling-rs 2.67.1 |
|---|---|---|---|
| `formula.pdf` (the notation-cue class) | 9.41% (S=4 I=4) | **7.06%** (S=3 I=3) | **7.06%** (S=3 I=3) |
| `regulated-doc.pdf` (sections + boxed warning + dosing table + footer stamps) | **0.00%** (0 of 218 words) | **0.00%** (0 of 218) | **41.28%** (D=55 I=35 of 218) |

### The structural rows (arXiv:2610.30026's cue-loss classes, named honestly)

| Class | Tier-0 | docling-py | docling-rs |
|---|---|---|---|
| **Reading order** (two-column.pdf) | content order (columns merged by content-stream order — the declared Tier-0 limitation) | headings glued into paragraph bodies; column flow preserved in content order | the same gluing PLUS one line displaced ABOVE its column heading (visible in the committed B golden) |
| **Tables as tables** (`table.pdf`, space-aligned text; the CPG's ruled tables, §4) | keeps the column whitespace, line-per-row — readable, no structure | **flattens space-aligned rows to one line** (no table detected); on the CPG's RULED tables: real markdown tables (62 lines on the 6-page subset) | same flattening on space-aligned; same real markdown tables on ruled |
| **Formula cues** (superscripts/subscripts as positioned runs) | glues runs inline (`E = mc 2`), keeps line structure | glues inline, keeps formula-per-paragraph | glues inline AND merges adjacent formula lines into one paragraph |
| **Repeated page furniture** (the twice-drawn footer stamp) | keeps both copies | keeps both copies | **keeps only the first stamp per page** (the second copy never reaches the output) |
| **Paragraph line structure** | keeps the source line breaks | re-flows paragraphs to single lines | re-flows, and fragments paragraph tails after headings (the D/I reordering class) |

### The regulated-doc finding, spelled out

docling-rs's 41.28% on the regulated fixture is NOT word loss — the
bag-of-words is complete (0 ref-only tokens) — it is **reordering plus the
dropped second footer stamp**: paragraph tail lines displaced after section
headings (D=55/I=35), the bottom `SYNTHETIC … page N of 2` stamp missing on
every page. For a tier whose purpose is regulated documents, "the footer
stamp on every page" is exactly the cue a citation may need; the class is
recorded, never smoothed (the plan's DC-2 law).

### The sanctioned default (the named act)

**Candidate A — `docling-py` (Python Docling 2.148.0 via
`scripts/docling-kernel.py`) is the sanctioned default Tier-2 tool.**

- Fidelity on OUR fixtures: A passes (0.00% on the regulated class, equal-best
  on the formula class); B fails the regulated class (41.28%, the
  furniture-drop + reordering classes).
- On the REAL document (§4) the two are near-byte-identical — B's failure is
  specific to dense synthetic layouts, but the plan's gate is our fixtures,
  and the regulated class is this tier's stated purpose.
- B's operator-posture win (one static binary, no venv/torch, ~20× faster,
  752 MB vs 1.3 GB artifacts) is real and recorded — the re-adoption path is
  a named act on a future docling.rs version re-measured through this same
  table (the fix would be upstream: the furniture drop and the reading-order
  fragmentation live inside docling.rs's layout/export, not in the shim).
- The kernel changes zero lines either way: the seam is the protocol, and B's
  shim stays committed as the measured, unadopted candidate.

## 3 · The timeout-topology finding (the round's engineering catch)

The plan's correction 5 named `CONVERTER_TIMEOUT_SECS = 300` — and the round's
first real e2e 408'd: the shared HTTP `TimeoutLayer`
(`config::REQUEST_TIMEOUT_SECS = 30`) wraps `/ingest/document`, so the tier's
named bound was unreachable dead code — a generic 408 fired BEFORE the
kernel's kill could answer `converter_timeout`, and the measured conversion
(§4, 6 dense pages) took **29.5 s warm — inside the wall by 0.45 s**. The fix
(the `import_router` merge-order precedent): the document route moved to
`memory::document_ingest_router()`, merged AFTER the shared timeout with its
own named budget `config::DOCUMENT_INGEST_TIMEOUT_SECS` =
`CONVERTER_TIMEOUT_SECS + REQUEST_TIMEOUT_SECS` (330 s). The kernel-side kill
stays 300 s; the HTTP headroom lets the named refusal travel. Pinned by
`the_document_route_merges_outside_the_shared_request_timeout` (the r47
line-order shape) and proven by the §4 e2e (30.8 s — past the old wall).

## 4 · The real regulated-document e2e (S4)

**Document:** VA/DoD Clinical Practice Guideline for the Management of Low
Back Pain, version 2.0 (2022) — U.S. federal government work, public domain.
Source URL `https://www.healthquality.va.gov/guidelines/Pain/lbp/VADoDLBPCPGFinal508.pdf`,
full-document sha256 `8a7ea79572a877d5cf4677bd529c0d69a9c456850f1d3dcb7e64c3247938804b`
(3,282,839 bytes, 141 pages). The e2e subset is pages 95–100 (the
recommendation tables) extracted with `qpdf --pages`, 539,389 bytes, sha256
`87525926b51f70b70cc323f383faba230a7ddc95b3cf6e0a2dd8b7d16a22c80f` — under
the shared 1 MiB body cap (the FULL 3.3 MB document answers
`document_too_large` at the cap; the child refuses it named — measured; a
page-range subset is the operator's honest path for large born-digital
documents this round).

**The run (the REAL server, the REAL tool, the REAL row):**
`target/debug/brain-server` on 127.0.0.1:4173 with
`BRAIN_DOCLING_PATH` → the candidate-A venv shim,
`BRAIN_DOCLING_ARTIFACTS` set; `POST /ingest/document` with the subset at
`tier:"docling"`:

- **200 created** in 30.8 s: `document_id: 1`, `tier: "docling"`,
  `extractor: "docling/2.148.0"`, `pages: 6`, `page_tiers: ["docling"×6]`,
  `ocr_engine: null`, `low_confidence_pages: 0`, `layout_unverified: true`,
  43 chunks inserted, 5 duplicates skipped, 0 quarantined.
- The header carries the full contract: `doc: 8752… | source: document |
  pages: 6 | tier: docling | extracted: <utc> | extractor: docling/2.148.0 |
  layout: unverified | pages-tier: docling,docling,docling,docling,docling,docling`
  — and NO `ocr-engine` field (DC-6's shape).
- **Duplicate re-serve**: the same bytes re-POSTed answer `status:
  "duplicate"`, `document_id: 1` — the re-derive RE-RAN the tool (28.9 s)
  and produced **byte-identical markdown** (DI-1's stamp law + DI-3's
  extractor agreement + the engine-pin law, all holding on real bytes).
- **The markdown review** (against the original pages): the recommendation
  tables come through as REAL markdown tables — 62 table lines, 6 separator
  rows, 6 columns per row, column headers (`Recommendation`, `2017 Strength
  of Recommendation`, `Evidence`, `2022 Strength of Recommendation`,
  `Recommendation Category`) preserved; spot-checked sentences match
  verbatim. Per page, docling reads +0–19% words over the Tier-0 text layer
  (the table-structure cells the text layer's column runs miss) — a gain,
  not a loss; page 4 (a short page) is identical in count.
- **The cue-loss honesty** (arXiv:2610.30026): the tier does NOT claim the
  tables are perfect — TableFormer's cell boundaries and the layout model's
  reading order are the tool's claim, accepted as produced (DC-2), recorded
  per run; `layout: unverified` stays on every docling header (the kernel
  never verifies the tool's layout reading this round).

## 5 · The ocr lane (the fourth way) — plan-05's rows extended unchanged

Born-digital text never routes to the sidecar (OV-1: per-page emptiness), so
the ocr tier's fidelity on the TEXT fixtures is structurally the Tier-0
shape. The ocr lane's own numbers are the scanned classes, measured in
IMPROVE05 (engine `apple-vision/recognize-text@26B123`, WER vs
`expected-text/`): clean-scan **0.00%**, photo-of-page **0.00%**,
multi-column-scan **0.00%**, handwriting **2.27%**, non-latin **10%** (the
auto-detect gap, recorded honestly there). The tier split stands: the ocr
tier owns scans; the docling tier owns layout-heavy born-digital documents;
Tier 0 owns everything else.

## 6 · The pins — the red ledger and the mutation receipts

`tests/docling_pins.rs` (17 pins). Red phase measured by stashing the
S2 route/emitter changes and running the file against the S1 tree
(`ded0ae95`): **12 red, 4 green** (the file's header carries the per-pin
ledger). The genuine catches:

- **DC-1 structural red**: the S1 wrapper's own COMMENT contained the literal
  `convert-remote`, tripping the byte-literal pin — the plan says the string
  appears NOWHERE in the source; the source was reworded, the pin stayed
  literal.
- **DC-3 independence red**: the ocr arm ran the dev-tree hybrid instead of
  refusing — the test now forces the bridge absent (the measured
  `missing_env` flake, improve05's shape).
- **DC-6's mutation proof caught a VACUOUS pin**: the first draft of the
  import-whitelist filtered on column-0 `starts_with` only — the wrapper's
  imports live indented inside `main()`, so the injected VlmPipeline import
  sailed through. The pin now reads STRIPPED lines and asserts it saw ≥ 4
  docling imports (never vacuous again).

The green-on-arrival pins' mutation red-proofs (each run against the pinned
tree, each reds BY NAME, each restored, then green):

| Mutation | Pin | Result |
|---|---|---|
| inject `convert-remote` into `docling-kernel.py` | DC-1 structural | RED (`…must never touch the API subcommand`) |
| inject `from docling.pipeline.vlm_pipeline import VlmPipeline` | DC-6 structural | RED (`outside the deterministic-pipeline whitelist`) — after the vacuous-pin fix above |
| inject `--pipeline vlm` into the shim's invocation | DC-6 structural | RED (`the shim's execution surface carries a generative flag`) |
| rename the route's absent-arm refusal to `converter_failed` | DC-3 absence | RED (`expected 503 converter_unavailable`) |
| drop the SANCTIONED posture lines from the protocol doc | DC-4 | RED (`the refusal record must cite…`) |

DC-2's structural half (no rewriting path in the route) is enforced by the
same code shape the emitter pins read (the pages pass verbatim from
`ConverterOutput` through `ExtractedDocument` into the emitter; the only
transformation is the split) — its behavioral half is the odd-bytes pin, and
its mutation proof is the route diff itself: any rewrite inserted between the
split and the emitter moves the DC-2 wire assertion.

## 7 · The toolchain/artifact record (the determinism tuple)

- Hardware/OS: macOS 27.2.0 arm64 (build 25C101 class), Apple Silicon.
- Candidate A: docling 2.148.0 / docling-core 2.106.0 /
  docling-ibm-models 4.0.4 / docling-parse 7.24.2 / Python 3.15.2; artifacts
  1.3 GB (`~/.cache/docling-artifacts`, the default download set including
  the enrichment models — the WRAPPER never loads them: the standard
  pipeline only, pinned by omission and by the DC-6 import whitelist).
- Candidate B: docling-rs 2.67.1 (prebuilt binary 54 MB) + `.models/` 752 MB
  (release `models-v1-f8a5d4fd8b08`, `SHA256SUMS`-verified; the enrichments
  — code_formula ~1.3 GB, NER, embed — deliberately NOT fetched: the no-LLM
  posture refuses them by not installing them).
- The goldens (both candidates) re-ran BYTE-IDENTICAL on this machine — the
  determinism tuple (same PDF bytes + same tool version + same models +
  same machine class) measured, not assumed. A NEW tool version is expected
  to drift: the goldens are version-named; re-run the probe and re-commit
  under a new directory (the README beside them carries the law).

## 8 · The Linux/macOS split (recorded honestly)

- The kernel-side pins (the DC set) run against the stub tool on ANY OS —
  green on Linux CI (the absence path + the stub paths + the structural
  reads). Measured here on macOS.
- The real-tool paths measured here: candidate A (venv + artifacts) and
  candidate B (binary + models) both ran on this macOS host; candidate B is
  natively Linux-native (prebuilt x64/arm64), candidate A installs anywhere
  Python does. The golden round-trip pin is tool-gated: it re-runs the
  candidates only where their toolchains exist and records the split
  honestly where they do not (the improve05 dual).

## 9 · The science base

- **arXiv:2610.30026** (Silent Cue Loss in Enterprise Document Ingestion,
  2026-10-21) — the honesty frame: the fidelity table MEASURES the loss
  classes (§2's structural rows), the conversion is accepted as produced
  (DC-2), the converter is recorded per run, and the cue-loss class is a
  measured limitation — the superscript cue is flattened by ALL THREE text
  lanes; no lane "solves" it and none claims to.
- **arXiv:2604.04948** (From PDF to RAG-Ready, 2026-03-30) — the downstream-QA
  methodology: this round's table is the extraction-fidelity half (WER +
  structural); the downstream half (does the chunked docling markdown answer
  questions better) is the named follow-up that table's method would drive.
- **arXiv:2608.26192** (layout-aware Docling chunking vs fixed/semantic) —
  the downstream-value evidence for the tier's existence: the docling chunks
  carry `## Page N` provenance (the existing chunker), and the structured
  tables (§4) are the layout-aware value the Tier-0 lane cannot produce.
- Plan-05's rows (arXiv:2611.05539's no-correction law; arXiv:2611.04874's
  faithful-transcription frame) extend unchanged — the tool's config is
  pinned by the same no-LLM law (§0.2 correction 4), and the tool's bytes
  cross the doors verbatim like the sidecar's.

## 10 · The verify record (this commit)

- `cargo fmt --check` clean; clippy `-D warnings` root (all targets) + the
  six feature lanes (loom, rerank-tier, neural-embed, injection-classifier,
  compliance-pack, multivec) green; the kernel battery green, 0 failed (the
  badge re-derived 3873 → **3890** after the S3/S4 pins, `badges.sh
  --verify-count` exit 0).
- The error taxonomy's drift-guard caught the newcomer
  (`ConverterError`, `tests/error_taxonomy.rs`) — classified as the
  `OcrSidecarError` shape (`service`, `display`): Failed's payload is the
  BOUNDED stderr tail (512 bytes, whitespace-collapsed in the helper), the
  other three variants fixed classifications.
- `cargo deny check advisories licenses bans sources`: green root + the
  7-workspace policy loop (crates, fuzz, client, tools/channel-bridge,
  tools/signal-gateway, tools/steward-harness, shell/src-tauri). **The lock
  is untouched** — the wrapper adds no dependency (the std-only temp
  lifecycle; the shim is POSIX sh).
- Shell: `pnpm check` 0 errors, `pnpm lint` clean, `pnpm test` 404 passed
  (schema.d.ts regenerated in the S2 commit — gen:api was NOT a no-op).
- docs-truth: clean (openapi.yaml + the route registration + the contract
  doc agree; the 501 text is gone from the spec and the schema).
- The census re-derived (`docs/test-census.md`) and the census pin green.
- The fixture corpus round-trips byte-identically (all 9 SHA256SUMS entries
  verified after the S3 reference fix — the PDFs never moved).
