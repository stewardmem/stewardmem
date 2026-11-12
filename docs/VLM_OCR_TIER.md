# THE VLM-OCR TIER (IMPROVE16.1)

**The one sentence:** the witness measured that the correction lane's residual
is the verbatim's omission, so 16.1 points the same governed pipeline at the
raw scan itself — a 0.9B MIT-licensed VLM on the operator's own MLX stack,
measured before adopted, stamped as produced, caged if corrected — because the
cheapest path to transcription truth is a better first reading, and the tree
already knows how to govern one.

**Status:** EXECUTED 2026-11-12 at brain-server `60a8941c`. The tier is
**ADOPTED AS A SANCTIONED TIER — OFF UNLESS CONFIGURED** — by measurement, not
assertion: the recorded gate is the plans repo's
`10_EVIDENCE_RAW/improve161-vlm-gate/` (the openapi description cites it; the
citation is load-bearing — a doc that loses the numbers fails the wire pin).

## The measurement gate (the adoption IS the table)

The DepEd Order 003, s. 2026 RAW image-only scan (the plan-06 corpus), the
operator's corrected reference transcription, the committed `wer.py` as THE
declared normalizer (same script both sides — the SA-6 pattern), the
preregistered bar: docling-py 2.148.0's sanctioned raw-scan row, **17.12%**
(plan 06 §0.4). The same-subsets docling cross-run composition-proofs the
verdict (the recorded 17.12% is a whole-doc row; the subsets bracket it):

| Lane (raw scan, native 200-ppi rasters) | p27-29 (witness set, 565 words) | p30-34 (annex forms, 2,544 words) |
|---|---|---|
| **GLM-OCR on MLX** (mlx-community/GLM-OCR-4bit, mlx-vlm 0.7.12, "Text Recognition:", temperature 0) | **5.66%** | **12.15%** |
| docling-py 2.148.0, same subsets | 12.74% | 39.11% |
| ollama glm-ocr:q8_0 | void — the renderer truncates every generation at 52 tokens (recorded AS FOUND; measures the cap, not the model) | void |
| *(the bar)* docling-py whole-doc row (plan 06 §0.4) | **17.12%** | |

Both subsets beat the bar AND the same-subsets cross-run. The residuals are
recorded honestly: p27-29's 32 error tokens are 23× the reference's own `<u>`
markup encoding (tokens the page image does not contain — the true OCR
residual ~1.6%); p30-34's is the VLM table-column omission class (~242 tokens
of repeated annex-matrix row values). The composition witness then measured
the thesis end-to-end: the raw-scan class's verbatim **31.33% → 5.22%**
through the same governed doors.

## The laws (what this tier is and is not)

1. **THE SEAM: the docling tool protocol's FILE contract, NOT its name.** The
   vlm shim (`scripts/glm-ocr-kernel.sh`) honors the two-file contract —
   `doc.md` with `<!-- page-break -->` markers, `meta.json` with `pages`
   derived INDEPENDENTLY of the markers (from the PDF's own page count via
   `pdfinfo`), exits 0/3/5 with 4 reserved — because the kernel's
   markers+1 == pages cross-check (DC-5 / VO-7) has two independent sources
   on both sides. But a VLM tier is **GENERATIVE EXTRACTION**, so it does NOT
   ride the docling protocol's "No LLM anywhere" law's name: the tier is a
   new named value `vlm` with its OWN law (this doc). The alternatives
   (riding `docling`; a plan-05 Vision sidecar) were recorded and lost.
2. **MEASURE BEFORE ADOPT** (the plan-06 two-candidate law): the gate table
   above IS the adoption; the unadopted candidate (ollama) is recorded with
   its numbers — a renderer cap, not a fidelity row.
3. **DI-1 DOES NOT HOLD FOR THIS TIER, preregistered.** Temperature 0 is
   DECLARED, not guaranteed; the model+quant+mlx-vlm version is STAMPED per
   run (`meta.json`'s `version` = `<model-id>@mlx-vlm-<ver>` — the quant is
   part of the model id); a re-derive under a moved stamp refuses
   `document_row_mismatch` (the engine-pin law, extractor-shaped);
   reproducible-BY-RECORD. (An honest observation from the gate run: the shim's
   doc.md was byte-identical to the S1 lane's on the same subset — recorded,
   never claimed.)
4. **THE FENCE: LOCAL-ONLY.** The shim checks its environment BEFORE any work
   and refuses a remote/generative posture BY NAME (exit 5 → the route's
   `converter_remote_refused` 503): `GLM_OCR_ENDPOINT`, `GLM_OCR_API_KEY`,
   `GLM_OCR_BASE_URL`, `VLM_ENDPOINT`, `VLM_API_KEY`, and docling.rs's
   `DOCLING_RS_VLM_ENDPOINT` (the remote+generative doubly-refused class). No
   data leaves. The engine runs with `HF_HUB_OFFLINE=1` +
   `TRANSFORMERS_OFFLINE=1` pinned — the model loads from the PRE-FETCHED
   local cache (the docling artifacts law); a missing cache is exit 3, named.
5. **THE DOORS ARE INHERITED, no bypass exists:** the shim's markdown crosses
   the trace strip (per page, before the anchors exist), the deterministic
   emitter, the fail-closed anchor self-check, the two-layer injection
   screen, the trust-label bounds, the HITL review posture, the chunker,
   embeddings, the write — the identical path DI-5 pins for the docling lane.
   A hostile tool page that smuggles its own `## Page` structure dies at the
   self-check (500, nothing lands — the cage; internals never leak to the
   wire, the named reason rides the log).
6. **THE COMPOSITION IS FREE — and it was witnessed, not hoped:**
   `correct: true` on a vlm row lands the corrected variant
   `vlm-corrected` through 16's lane UNCHANGED (two artifacts, the lineage
   anchor, the cage, the stamps, the audit row). `corrected_tier` is
   tier-blind; ZERO new correction code is the acceptance. In the gate's
   composed witness the 9B corrector's body came back BYTE-IDENTICAL — the
   no-invention law at its cleanest (the residual is the reference-encoding
   class it rightly cannot touch).
7. **THE STAMPS ARE THE TOOL-TIER LAW:** the header's `extractor:` == the
   row's `extractor`+`extractor_version` == the shim's meta.json
   (`tool:"glm-ocr"` + the version stamp), DI-3 agrees, a re-serve under a
   moved stamp refuses named. The header NEVER carries an `ocr-engine` field
   (the VLM is not the OCR sidecar) and the per-page map is all-`vlm`.
8. **THE TIER IS OFF UNLESS CONFIGURED:** `BRAIN_VLM_OCR_PATH` unset/absent →
   `vlm_ocr_unavailable` 503, named, BEFORE any work; the DEFAULT pipeline is
   byte-identical (the four probes are independent siblings — OV-4's shape;
   VO-1's guard).
9. **THE 1:1 RASTER LAW:** the shim extracts the PDF's embedded page images
   (`pdfimages -png` — the EXACT native rasters, no resampling) and requires
   them to map 1:1 onto the PDF's own page count; a scan it cannot map
   verifiably refuses (exit 3, named) rather than guessing a page mapping.
10. **ORIGIN, HEALTH-COMPLY:** vlm text lands as `ingest_kind: document` →
    `origin: imported` (the write-side taint label, tier-blind by
    construction) + the tool's stamp on the row — **never `human`**:
    machine-extracted text never claims human authorship (the doc-correction
    lane's law 10, extended to the vlm verbatim).
11. **NO TOOL-SIDE TIMEOUT** (the protocol's exit-4 reservation): the shim
    sets no deadline — the kernel kills at `CONVERTER_TIMEOUT_SECS` (300 s)
    and answers `converter_timeout` itself; a tool-side deadline that emits
    partial pages would smuggle a half-document through as success.

## Configuration walkthrough (the operator posture)

```sh
# 1. the model, PRE-FETCHED (the artifacts law — no HF fetch at run time):
huggingface-cli download mlx-community/GLM-OCR-4bit          # MIT, ~1.2 GB
# 2. the engine's venv (mlx + mlx-vlm; poppler for pdfinfo/pdfimages):
python3 -m venv .venv-mlx && .venv-mlx/bin/pip install mlx-vlm
brew install poppler
# 3. the seam (strictly operator-configured — None is a posture, not an error):
export BRAIN_VLM_OCR_PATH=/path/to/brain-server/scripts/glm-ocr-kernel.sh
export BRAIN_VLM_PYTHON=/path/to/.venv-mlx/bin/python
# optional: GLM_OCR_MODEL (default mlx-community/GLM-OCR-4bit)
```

The route: `POST /ingest/document` with `"tier": "vlm"`. Unconfigured →
`vlm_ocr_unavailable` 503. The body-cap law applies unchanged (the shared
1 MiB cap — raw scans go through as page-range subsets or cap-compliant
renditions, the CPG pattern).

## The composed corrector posture (recorded in S4)

The 9B corrector witness (`mlx-community/Qwen3.5-9B-4bit`, the CANONICAL
plain quant) recorded three facts, all AS FOUND:

- the searchable-class delta is **+0.53** (31.33% → 30.80%) — honestly under
  the 4B uncensored's +1.95 and the 7B's +0.71: the omission-dominated
  residual is not a model-size problem;
- the preregistered `--max-tokens 4096` budget prerequisite ALONE did not
  clear the thinking model at 9B scale — the third live governance event
  (`corrector_unavailable` 503, the verbatim row whole, the audit row error;
  the lane working); the cure is operator-side, the addendum's own class
  extended: **`--chat-template-args '{"enable_thinking": false}'`** (the
  cured server answered in 37.6 s, faster than the 4B's 51.3);
- on the vlm verbatim the corrector's body came back byte-identical
  (+0.00) — the lane measures, it never masks.

```
mlx_lm.server --model mlx-community/Qwen3.5-9B-4bit \
  --max-tokens 4096 --temp 0.0 \
  --chat-template-args '{"enable_thinking": false}'
```

## The pins

VO-1 the OFF law + the named 503 + the default byte-identical · VO-2 the
stamp end-to-end + DI-3 + the moved-stamp refusal · VO-3 the doors (strip +
screen + cage; a hostile output cannot bypass) · VO-4 the measurement gate
load-bearing in the wire contract · VO-5 the fence (exit-5 honored, named) ·
VO-6 the composition through 16's lane unchanged · VO-7 the protocol
cross-check (markers+1 == pages, two independent sources) + the bounded
stderr — all in `tests/vlm_ocr_pins.rs` (born red-first against the pre-S3
tree: 11 red, 1 green-on-arrival guard, the ledger in the S3 commit), the
shim's own pins in `tests/glm_ocr_shim_pins.rs` (the stub dual; zero live
model calls in CI; the presence-gated real golden asserts the CONTRACT, not
bytes — DI-1).

## The code map

| Piece | Where |
|---|---|
| the tier vocabulary + the provenance variant | `src/ingest_document.rs` (`TIER_VLM`, `DocumentProvenance::Vlm`, `probe_vlm_ocr`) |
| the config seam | `src/config.rs` (`vlm_ocr_path`, `BRAIN_VLM_OCR_PATH`) |
| the door's vlm arm + the re-derive extension | `src/server/router/memory.rs` (the fourth probe sibling; `rederive_document_markdown`'s vlm seam) |
| the shim (the FILE contract + the fence) | `scripts/glm-ocr-kernel.sh` |
| the engine (the model-serving half) | `scripts/glm-ocr-mlx.py` |
| the shim pins / the route pins | `tests/glm_ocr_shim_pins.rs` / `tests/vlm_ocr_pins.rs` |
| the wire (additive) | `openapi.yaml` (the enum gains `vlm`; the description cites the gate) + `shell/src/lib/api/schema.d.ts` (gen:api lockstep) |
| the recorded gate + the witness | the plans repo `10_EVIDENCE_RAW/improve161-vlm-gate/` |

## The science rows

- arXiv:2611.04874 — OCR in the Foundation Model Era: faithful transcription
  (complete reconstruction within the visual scope) is the definition the
  VLM tier is measured against — and the reason omission, not substitution,
  is the residual class that matters.
- arXiv:2611.13609 — homoglyph noise runs ~5× denser on mixed-script pages:
  the DepEd Filipino/English adjacency is that class; the correction lane's
  homoglyph fixes are its remedy.
- arXiv:2609.21595 — ICL/retrieval strengthens local-endpoint post-OCR
  correction: the local-only fence is a quality posture too, not only a
  privacy one.
- arXiv:2604.22774 — over-correction is PENALIZED: measure both artifacts
  (wer.py's both-artifacts ledger does exactly this).
- arXiv:2611.05539 — EditVeto: a learned 0.6B veto gate; the lane's anchor
  contract is the DETERMINISTIC veto (no model in the gate).
