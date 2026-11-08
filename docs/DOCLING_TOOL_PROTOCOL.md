# DOCLING TOOL PROTOCOL — the Tier-2 converter contract (IMPROVE06)

The contract between the kernel (`brain-server`) and the operator-configured
Tier-2 converter. The kernel defines the contract and is language-agnostic:
`BRAIN_DOCLING_PATH` points at **whichever executable honors this protocol** —
the kernel never knows or cares what language it is. The plan is
`plans/improvements/06_DOCLING_OPTIONAL_TOOL.md` (plans repo), its §0.2 the
ground record; the doc-ingest contract this plugs into is
`docs/DOCUMENT_INGESTION_CONTRACT.md`; the sidecar precedent is
`docs/OCR_SIDECAR_PROTOCOL.md`.

## Why a protocol and not the CLI directly

The annotated-markdown contract's per-page anchors (DI-2) need page breaks.
The Python Docling CLI (`docling convert`, v2.141+) does not expose the
page-break placeholder (`md_page_break_placeholder` is a docling-serve
option — verified against `cli/main.py`, 2026-11-08), and docling.rs's
delimiter option has no meta equivalent in its file output. So the kernel
pins a two-file contract any converter can satisfy through a thin shim, and
ships one committed implementation per candidate.

## The contract

```
<tool> <in.pdf> <out-dir>
```

The tool writes exactly two files into `<out-dir>` (creating it if needed):

| File | Content |
|---|---|
| `doc.md` | The converter's markdown, **pages separated by the literal marker `<!-- page-break -->`** (N pages → N−1 markers). The bytes are the converter's own output — the tool never rewrites, smooths, or post-corrects them. |
| `meta.json` | `{"tool":"<impl>","version":"<ver>","pages":N}` — one line. `tool` is the implementation name (`docling` for the Python wrapper, `docling-rs` for the Rust shim); `version` is the converter version that ran; `pages` is the converter's own page count, derived INDEPENDENTLY of the markdown markers (so the kernel's markers+1 == pages cross-check has two independent sources). |

**Exit codes:**

| Exit | Meaning |
|---|---|
| `0` | `doc.md` + `meta.json` written; the conversion is complete and whole. |
| `3` | Conversion failed (the bytes' fault, a bad invocation, the tool missing). The tool prints a bounded cause on stderr; the kernel bounds it again. |
| `4` | **Timed out — self-timed.** RESERVED for a tool that enforces its own deadline and refuses to emit a partial document. Neither committed implementation sets a tool-side timeout, so neither can emit 4: the kernel kills at `CONVERTER_TIMEOUT_SECS` (300 s — models cold-load) and answers `converter_timeout` itself. The code exists because a tool-side deadline that emits partial pages instead of exiting (docling.rs's `--document-timeout` emits the pages done so far; docling's `document_timeout` is checked between pages) would smuggle a half-document through as success — a posture the contract forbids. |
| `5` | Remote/generative configuration refused. The tool checks its environment BEFORE any work and refuses a remote-services posture by name. |

Any other exit (a crash, a signal) maps to the kernel's `converter_failed`
posture — the tool's internals are its own.

## The laws the implementations hold (the kernel pins read them structurally)

- **No data leaves.** Conversion is local: local file in, local files out,
  local pre-fetched model artifacts. No remote client is imported or invoked
  anywhere (`docling convert-remote` — the Python CLI's API subcommand —
  appears nowhere); URL sources are refused (`--no-url-fetch` on the Rust
  side; the wrapper accepts a local path only); remote image resolution
  stays off (docling.rs's `--image-sources` default `none`; the wrapper's
  artifacts come from the pre-fetched tree only).
- **No LLM anywhere.** The deterministic STANDARD pipeline (layout model +
  TableFormer + OCR-model inference — deterministic model inference, not
  generation) is each converter's default and stays the default **by
  omission**: no `--pipeline vlm`, no VLM endpoint/model/prompt options, no
  generative `--enrich-*` surface exists in either implementation. The 20+
  generative presets the Python CLI fronts are unreachable through the
  wrapper by construction; docling.rs's `vlm_*` options and
  `DOCLING_RS_VLM_ENDPOINT` env are refused named (exit 5) — a VLM endpoint
  is both a remote call and an LLM, doubly refused.
- **The output is accepted AS PRODUCED.** `doc.md` carries the converter's
  bytes verbatim (DC-2). Cue loss (arXiv:2610.30026: every mainstream
  converter loses the visual-encoding layer) is a MEASURED limitation the
  fidelity table records — never smoothed, never corrected.
- **Refusals are named, never partial.** A failed conversion produces no
  `doc.md`; a remote/generative posture refuses before any work; a partial
  conversion status is refused (`PARTIAL_SUCCESS` ≠ success).

## The environment knobs

| Variable | Role |
|---|---|
| `BRAIN_DOCLING_PATH` | (kernel-side) the tool's path — the one configuration the Tier-2 tier needs. Absent → `converter_unavailable` (503). |
| `BRAIN_DOCLING_ARTIFACTS` | (wrapper A) the pre-fetched local model artifacts (`docling-tools models download`); unset → docling's default cache with a warning — the offline guarantee only holds with the pre-fetch. |
| `DOCLING_SERVICE_URL` / `DOCLING_SERVICE_API_KEY` | **Refused** (exit 5) — the remote-services posture. |
| `DOCLING_RS_VLM_ENDPOINT` | **Refused** (exit 5, shim B) — docling.rs's remote-LLM surface. |

## The two candidate implementations

**Candidate A — `scripts/docling-kernel.py`** (Python Docling, the venv
posture). Drives the Python API: `DocumentConverter` with
`PdfPipelineOptions(artifacts_path=BRAIN_DOCLING_ARTIFACTS)` — the standard
pipeline, pinned by omission — then
`export_to_markdown(page_break_placeholder=MARKER,
image_mode=ImageRefMode.PLACEHOLDER)`; `meta.pages` is `len(doc.pages)`,
`meta.version` the installed `docling.__version__`. Runs inside the
operator's venv (`pip install docling`): `-I`-unsafe by design, stdlib +
docling only. `PARTIAL_SUCCESS` refuses (exit 3); a convert() exception is a
bounded one-line exit 3.

**Candidate B — `scripts/docling-rs-kernel.sh`** (docling.rs, the static
binary posture). Invokes `docling-rs --no-url-fetch --to md --to json
--page-break-placeholder '<!-- page-break -->' --images placeholder <in.pdf>
--output <tmp>` — the standard pipeline by omission — moves the markdown to
`doc.md`, counts `meta.pages` from the JSON's `pages` object (brace-depth
count, independent of the markers), and stamps `meta.version` from
`docling-rs --version`. Needs POSIX sh + the `docling-rs` binary on PATH
(prebuilt macOS arm64 / Linux x64+arm64, or `cargo install docling-cli`);
the binary resolves its `.models/` relative to its own (symlink-resolved)
location — install beside the models and nothing else is configured. Exit
codes map: tool 0 → 0; any tool failure → 3; the remote/generative refusal →
5 before the tool runs.

**Provenance of this split (measured 2026-11-08, the plan's correction 10):**
both candidates passed the S1 preflight on the corpus. Rust-vs-Python is
decided by S3's four-way fidelity table on our fixtures — docling.rs's own
README warns its benchmark favors itself by construction, so its conformance
claim is measured here, not trusted. The unadopted candidate stays committed
and recorded with its measured numbers (the named-act discipline).

## Why Docling — the recorded license posture (DC-4's record, verified 2026-11-08)

The Tier-2 slot is a RECORDED REFUSAL first and a tool second. The
alternatives were measured and refused on their current terms, in-repo, so
the next "why not Marker" is answered without re-litigating:

| Tool | License (verified 2026-11-08) | The refusal |
|---|---|---|
| **Marker** (`datalab-to/marker`) | code **Apache-2.0**; weights **AI PUBS Open RAIL-M (Modified)** | the weights' open-responsible-AI license carries use restrictions a commercial regulated pipeline (MemorySteward LLC's `modules/health-comply`) does not accept — the copyleft half of the old GPL-era refusal is retired, the use-restriction half survives |
| **MinerU** (`opendatalab/MinerU`) | **Apache-2.0** + commercial thresholds (above 100 M MAU or USD 20 M monthly group revenue) + an online-service attribution obligation | the thresholds and the attribution obligation are a revenue-gated posture, refused for the same reason the threshold is recorded: the pipeline must not carry a license that changes shape at a revenue line |
| **olmOCR** (`allenai/olmocr`) | Apache-2.0 | not refused — NVIDIA-CUDA-bound (vllm/FP8), so not runnable on this stack's platforms; recorded as the noted VLM-upgrade lane on plan 05's measured WER evidence, adopted (or refused) by a named act |
| **Docling** (`docling-project/docling`, Python) | **MIT** (IBM Research Zurich origin, LF AI & Data) | **SANCTIONED** — no revenue gates, no use restrictions; the deterministic standard pipeline is model inference, not generation |
| **docling.rs** (`docling-project/docling.rs`) | **MIT** ("matching upstream docling") | **SANCTIONED CANDIDATE** — the same models as ONNX (RT-DETR layout, PP-OCR, TableFormer), one static binary; sanctioned-vs-default is S3's measurement, not this table |

The model artifacts carry their own upstream licenses (Docling's
docling-models terms; docling.rs's `models.json` is digest-bound with
per-asset SPDX records and a `MODELS_NOTICE.md` attribution file) — the
operator installs them locally; no weights ship inside the kernel, the
bundle, or this repository.

The no-remote law is part of the posture: the sanctioned tools run with
pre-fetched local artifacts and refuse remote-services configuration
(exit 5) — a converter that phones home would violate the no-data-leaves
law this tier exists to keep.

## The kernel-side refusals this protocol feeds

`POST /ingest/document` at `tier:"docling"` maps the tool's outcomes to the
route's named refusals (see `docs/DOCUMENT_INGESTION_CONTRACT.md` §2):
tool absent/unconfigured → `converter_unavailable` (503); conversion failed
(exit 3 or crash) → `converter_failed` (422, the bounded stderr tail in the
MESSAGE — never a row); kernel kill at `CONVERTER_TIMEOUT_SECS` →
`converter_timeout` (422); remote/generative configuration refused (exit 5)
→ `converter_remote_refused` (503). Tier 0 and the ocr arm are never
affected (the three probes are independent, OV-4's shape).
