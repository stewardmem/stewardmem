# The Document-Correction Lane (IMPROVE16)

The operator-configured LLM correction step for scan-grade documents — a
GOVERNED second artifact, never a rewrite. This doc is the lane's protocol:
what it is, the laws it reconciles, how to configure it, and what was
measured. The plan lives in the plans repo
(`plans/improvements/16_DOC_CORRECTION_LANE.md` + the verified execution
prompt); this doc is the operator-facing protocol.

## The one sentence

**The correction lane turns a measured OCR quality gap into a governed
second artifact — never a rewrite, always stamped, caged by the structure,
disposed by a human — because the verbatim truth stays the anchor even when
a better reading exists beside it.**

## The laws

1. **Two artifacts, one lineage.** The verbatim pipeline (Tier 0 / ocr /
   docling) lands its row exactly as before the lane existed — untouched,
   byte-identical when `correct` is absent or false. When the operator
   requests correction (`"correct": true` on `POST /ingest/document`), the
   route ALSO produces a corrected variant as its OWN document row:

   | field | the verbatim row | the corrected row |
   |---|---|---|
   | `sha256` | sha of the PDF bytes | **sha of the VERBATIM markdown** (the lineage anchor: the row records what was corrected) |
   | `markdown_sha256` | sha of the verbatim markdown | sha of the corrected markdown (what was produced) |
   | `tier` | `text-layer` \| `ocr` \| `mixed` \| `docling` | the verbatim tier + `-corrected` (e.g. `docling-corrected`) |
   | `extractor` | `pdf-extract` / the converter tool | `doc-corrector` |
   | `extractor_version` | the extractor version | **the stamp `<model>@<served>`** |

   Nothing is rewritten in place; the verbatim row remains the audit
   anchor; dedupe across the two variants is the operator's named act
   (clearing the corrected row is a data act — a re-request never silently
   re-corrects: the duplicate path is a READ-class act that names the
   existing artifact, and the open-path backstop refuses
   `corrected_already_exists` 409).
2. **The structure is the corrector's cage.** The corrected markdown MUST
   re-pass `verify_anchor_contract` — the same deterministic `## Page N` +
   anchor check the verbatim pipeline's own output passes (DI-2). A
   correction that breaks the structure refuses
   `correction_contract_violation` (422); the verbatim row survives whole.
   **No model sits in the gate** — the anchor contract IS the veto, the
   deterministic analogue of EditVeto's learned per-span gate
   (arXiv:2611.05539, "Rewrite, Then Veto"). The route — never the
   corrector — authors the header: a hostile or careless endpoint cannot
   forge the doc sha or the stamp (witnessed by the
   route-header-wins fixture).
3. **The provider is the GDL pattern** (`config::DocCorrectorProfile`):
   `BRAIN_DOC_CORRECTOR_BASE_URL` / `_MODEL` / `_SECRET_FILE` /
   `_SECRET_ROOT` — all-or-nothing; none = the lane is OFF; a partial
   profile is FATAL AT BOOT (`validate_doc_corrector_config`) and the
   named `corrector_config_invalid` refusal at the route. The secret rides
   the secret-file law verbatim (root-confined, symlink-refused,
   owner-only 0600). A local endpoint that ignores Authorization still
   gets the shape: point `*_SECRET_FILE` at any owner-only file.
4. **The local-endpoint posture.** `http` is admitted ONLY as the lane's
   preferred shape (ollama / MLX / LM Studio on the operator's own machine
   — the no-data-leaves law's preferred shape); a hosted endpoint is the
   operator's explicit choice, recorded in the audit row per call. The
   endpoint is operator-configured all-or-nothing, never
   request-selected — so the address screen resolves and pins the
   configured host with private/loopback admitted BY DESIGN (unlike
   webhooks, whose URLs face request influence). Redirects are refused:
   the screen validated ONE endpoint.
5. **The bounded call.** Named timeouts (connect 5 s / first-byte 120 s /
   total 600 s), a 4 MiB response cap enforced ON THE STREAM, ONE retry
   with a named 500 ms backoff on the transient classes only (connect,
   timeout, 5xx). A 4xx, a malformed body, an empty content, or an
   over-cap body refuses without a retry. Refusal vocabulary:
   `corrector_unavailable` (503), `corrector_failed` (422).
6. **The audit row per call.** Kind `doc-correction` on the hash chain —
   the model asked and served, bytes in/out, duration, attempts, and the
   request/response sha256s. The chain stores HASHES of target + detail,
   so no URL and no secret can ride a row. The `target` is the VERBATIM
   markdown's sha — what was corrected. This is the reproducible-BY-RECORD
   seam for an artifact that is honestly NOT a pure function of the PDF
   bytes (DI-1 does not hold for it; the record is what makes it
   auditable; re-correction is a new explicit request producing a new
   row, never a silent re-derive).
7. **HITL inherited.** Under the review posture the corrected variant
   files its OWN proposals (per chunk, the same 50-cap) — nothing
   auto-promotes. The stamp travels to the chunks' provenance through the
   row (chunks point at the corrected row by its unique key), so recall
   can cite "corrected by <model>".
8. **The corrector's output is untrusted input.** It crosses the SAME
   hygiene the verbatim pages cross: the reasoning-trace strip
   (`<think>` spans never survive) and the two-layer injection screen
   (Reject → the named refusal; Quarantine → the corrected chunks land
   flagged, no vector).
9. **The DI-3 extension.** `verify_document_header` requires the header's
   `corrector:` field to agree with the row's stamp EXACTLY, and the
   row's corrected posture to agree internally (the `doc-corrector`
   extractor ⇔ the `-corrected` tier suffix). Every tamper shape fails by
   name: a stripped stamp, a swapped stamp, a forged stamp on a plain
   row, a mixed-posture row.
10. **Corrected text is `origin: imported`, never `human`.** The
    corrected chunks inherit the document door's trust label and derive
    to `imported` via `origin_for_source` — a machine-processed artifact
    with a machine stamp, never human authorship (the `gate::origin_for_source`
    law's closure: a token-authenticated principal cannot forge human
    provenance, and a corrector certainly cannot).
11. **Over-correction is measured, never rewarded** (arXiv:2604.22774's
    penalty class): the pinned prompt forbids changing correct text and
    inventing content, and the witness measures BOTH artifacts against
    the reference — the honest delta, whichever way it lands.

## The pinned prompt

`CORRECTOR_SYSTEM_PROMPT` (`src/doc_corrector.rs`) is COMMITTED BY NAME —
correct text WITHIN the page/anchor structure: keep every `## Page N`
heading, every `<!-- anchor: pN -->` line, every other comment, and the
header byte-identical; never add/remove/reorder pages or structure;
preserve the document's language(s); never invent content; output only the
corrected markdown. The prompt is the courtesy; `verify_anchor_contract`
is the law. Changing the prompt changes the lane's outputs — it is part of
the lane's governance, versioned by the repo like every other law.

## Configuration

```sh
export BRAIN_DOC_CORRECTOR_BASE_URL=http://127.0.0.1:11434   # local ollama
export BRAIN_DOC_CORRECTOR_MODEL=qwen2.5:7b
export BRAIN_DOC_CORRECTOR_SECRET_FILE=corrector.key          # 0600, inside the root
export BRAIN_DOC_CORRECTOR_SECRET_ROOT=/etc/brain/secrets     # absolute
# ollama ignores Authorization — the scratch file satisfies the secret-file
# LAW's shape (one posture, no branching).
```

Then: `POST /ingest/document` with `"correct": true` (plus the usual
`data`/`title`/`tier`). Absent/false `correct` = the pre-lane pipeline,
byte-identical. Unconfigured + `correct: true` → `corrector_unconfigured`
503 and NOTHING lands (the request was correction-shaped; a silent
verbatim-only insert would hide the operator's intent).

The stamp: `<model>@<served>` where `model` is the configured
`_MODEL` and `served` is the endpoint's echoed model identity — a hostile
or missing echo degrades to `unversioned` rather than poisoning the header
parser's pipe-split field vocabulary.

## What was measured (the DepEd witness, 2026-11-12)

The corpus: DepEd Order No. 003, s. 2026 (the plan-05/06 measurement's
49-page government scan). The witness rode pages 27–29 of the operator's
searchable variant as a page-range subset (356,568 bytes — the CPG
over-cap pattern; the full variants exceed the shared 1 MiB cap). The
reference: the operator's corrected transcription (`DO_s2026_003r-1.md`).
The endpoint: local ollama, 127.0.0.1:11434. Transcripts + both artifacts
+ the WER script: the plans repo's `10_EVIDENCE_RAW/improve16-witness/`.

1. **The cage fired on real traffic.** qwen2.5:3b (20.4 s) broke page 3's
   anchor structure; the route refused `correction_contract_violation`
   (422); the verbatim row survived whole; the `doc-correction` audit row
   recorded the call. The deterministic veto works against a model that
   cannot hold the structure.
2. **The 7B pass landed two artifacts.** qwen2.5:7b (41.3 s): the
   corrected row carried the lineage anchor (its `sha256` IS the verbatim
   row's `markdown_sha256`), the stamp end-to-end, and the audit row.
3. **The measured delta, honestly:** verbatim **31.33%** WER vs corrected
   **30.62%** — **+0.71 points** on THIS class. The fixes were precisely
   the homoglyph class the plan's literature names (arXiv:2611.13609's
   mixed-script adjacency: `Al`→`AI` twice, `peonne/tache`→`person/teacher`,
   `L`→`I`) with ZERO over-correction (the word count preserved, the
   structure held). The plan's 9–15-point claim is the RAW-scan class
   (raw engines at 17–53% WER) — that class was not witnessable on this
   machine (the shell-vision sidecar absent; every raw-scan page exceeds
   the shared cap) and is recorded as the split, never claimed.
4. **The residual error class on this subset is word OMISSION** (421
   words vs the reference's 565 — the inherited layer lost form-field
   text). The pinned prompt's no-invention law correctly refuses to
   fabricate missing content: the lane measures, it never masks.

## The science rows

- **arXiv:2611.05539** — "Rewrite, Then Veto: Rewriter-Agnostic Edit
  Gating for LLM-Based Post-OCR Correction": EditVeto is a 0.6B LEARNED
  per-span gate. The lane's anchor contract is the DETERMINISTIC veto —
  no model in the gate.
- **arXiv:2611.04874** — "OCR in the Foundation Model Era": faithful
  transcription is the standard the correction is measured against.
- **arXiv:2611.13609** — mixed-script homoglyph noise (~5× error-density
  concentration); the DepEd corpus's Filipino/English adjacency is the
  same risk class — and the class the witness's fixes were actually of.
- **arXiv:2609.21595** — ICL + retrieval strategies for post-OCR
  correction: training-free correction is real, strengthening the
  local-endpoint posture.
- **arXiv:2604.22774** — over-correction is PENALIZED in evaluation:
  measure BOTH artifacts, never reward rewrites (the witness did).

## Where the code lives

| piece | place |
|---|---|
| the profile + boot validator | `src/config.rs` (`DocCorrectorProfile`, `validate_doc_corrector_config`) |
| the client, the prompt, the bounds, the audit write | `src/doc_corrector.rs` |
| the audit kind | `AuditKind::DocCorrection` (`src/audit/mod.rs`) |
| the corrected vocabulary, the cage, the DI-3 extension | `src/ingest_document.rs` (`DOC_CORRECTOR_EXTRACTOR`, `corrected_tier`, `emit_corrected_header`, `assemble_corrected_markdown`, `verify_document_header`) |
| the route | `src/server/router/memory.rs` (`ingest_document`, `run_correction`, `write_corrected_row`, `propose_corrected_chunks`) |
| the route pins | `tests/doc_correction.rs` (red-first, transcripts recorded) |
| the witness runner | `examples/deped_witness.rs` |
| the wire | `openapi.yaml` `/ingest/document` (`correct`, the `corrected` block, the six named lane refusals) |
