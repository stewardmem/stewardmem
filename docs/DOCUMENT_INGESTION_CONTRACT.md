# DOCUMENT INGESTION CONTRACT — the annotated-markdown shape (IMPROVE04)

The deliverable shape of `POST /ingest/document`: a PDF's text layer becomes
**annotated markdown** whose every byte is attributable — a claim that cites a
page cites a page that exists, and a header that names an extractor names the
extractor that ran. This file is the contract the emitter implements and the
pins (DI-1..DI-6) enforce; the plan is
`plans/improvements/04_DOCUMENT_INGESTION_CORE.md` (plans repo), its §0.1 the
ground record.

## The laws (unchanged, restated because this door inherits them)

- **Deterministic zero-token hot path.** Tier-0 extraction is a pure function
  of the PDF bytes: same bytes + same extractor version → byte-identical
  markdown. No LLM anywhere in extraction.
- **No data leaves.** Everything runs in-process; no converter, no network, no
  model. The optional converter seam (Tier 2) probes a local, operator-
  configured binary and refuses by name when it is absent — it never ships
  bytes off-host.
- **A PDF is NOT a new promotion path.** The extracted markdown crosses the
  EXISTING doors only (`/ingest/markdown` semantics): the injection screen,
  the reasoning-trace strip, dedupe, trust-label bounds, the HITL gate.
- **Tier 0 never claims layout verified.** Multi-column reading order is NOT
  attempted at Tier 0; the header and the document row both carry
  `layout: unverified` / `layout_unverified: 1`.

## 1 · The annotated-markdown contract (verbatim shape)

```markdown
<!-- doc: <sha256> | source: <upload|url|label> | pages: N | tier: text-layer
     extracted: <utc> | extractor: <name+version> | layout: unverified -->
## Page 1
…content…
<!-- anchor: p1 -->

## Page 2
…content…
<!-- anchor: p2 -->
```

### The machine-checkable header (DI-3's anchor)

One HTML comment immediately at the top of the emitted markdown, two physical
lines, `|`-separated `key: value` fields. After parsing, whitespace around
keys/values is trimmed; the comment's leading `<!--` and trailing `-->` are
stripped; **each physical line splits on `|` independently** and the field set
is the union across lines, order-checked by key, not by column. (Measured
during S2: folding the newline into one string merges the last field of line
one into the first of line two — `tier:` swallowed `extracted:` — so the
per-line split is the parse the pins test.)

| Field | Rule |
|---|---|
| `doc` | lowercase hex sha256 of the **source PDF bytes** (64 chars). The integrity anchor: the row and the header must agree. |
| `source` | the trust label the request carried (≤ `MAX_SOURCE` bytes, the closed vocabulary at the write seams). |
| `pages` | the page count the extractor observed (integer ≥ 1). |
| `tier` | What RAN, derived from the per-page routing: `text-layer` (every page from the text layer — Tier 0's shape), `ocr` (every page from the sidecar), `mixed` (at least one of each). `docling` (IMPROVE06) is the Tier-2 tier: the WHOLE document went through the operator-configured converter, so the request's tier lands the row unchanged (the converter reads whole documents — no per-page mixing this round). |
| `extracted` | RFC3339 UTC of the extraction event. **The one volatile field** — see the determinism rule below. |
| `extractor` | `<name>/<version>`, e.g. `pdf-extract/0.12.1`. At Tier 0 the version is the resolved crate version, pinned against the lockfile by a test. On a docling row the stamp is the TOOL's own identity from its meta.json (e.g. `docling/2.148.0` or `docling-rs/2.67.1`) — the operator-installed version, recorded per run like the OCR engine string, never lockfile-pinned (a re-derive under a moved version refuses, the engine-pin law). |
| `layout` | `unverified` at Tier 0 — always. A `verified` value here is a found bug (DI-2's sibling pin). |
| `ocr-engine` | IMPROVE05, **present exactly when OCR ran**: the sidecar's engine stamp, `apple-vision/<request-type>@<os-build>` (e.g. `apple-vision/recognize-text@26B123`). Recorded per run — never promised bitwise across OS builds. A docling header NEVER carries this field (the converter is not the OCR sidecar; DC-6's pin asserts the absence). |
| `pages-tier` | IMPROVE05, **present exactly when OCR ran**: the per-page tier map, comma-joined in page order (`text-layer,ocr,text-layer,ocr`). IMPROVE06: a docling row's header carries the all-`docling` map (`docling,docling,…` — the whole-document read). Machine-checkable; the row carries the same map (DI-3's extension) and the sidecar request named exactly the `ocr` pages (OV-2's three-way agreement). |

### The page blocks (DI-2's anchor)

- One `## Page N` heading per page, **N monotonic from 1 through `pages`** with
  no gaps.
- The page's extracted text follows its heading, verbatim except for the
  pre-insert doors named above (the strip runs before chunking, never after
  the anchors exist).
- Each page block closes with `<!-- anchor: pN -->` — the machine-checkable
  provenance anchor. **Every emitted page carries its anchor**; a page block
  without one is a found bug.
- An empty text layer on an individual page emits the heading + anchor with no
  body (honest emptiness beats invented content).
- A PDF with **no text layer at all** (an image-only scan) is refused NAMED at
  Tier 0: `no_text_layer`. Tier 0 never OCRs, never invents, never extracts
  garbage. At `tier:"ocr"` the same verdict is the ROUTING SIGNAL: every page
  goes to the sidecar (OV-1's vacuous form), and the reply defines the page
  count.
- **The OCR marker line (IMPROVE05).** Each page the sidecar recognized
  carries, inside its block before the anchor, exactly one marker:
  `<!-- ocr: engine <stamp> | confidence <c> -->`; when `c` is below the
  named floor (`OCR_CONFIDENCE_FLOOR` = 0.5) the marker ends
  `| low-confidence` (OV-3); a page Vision recognized NOTHING carries
  `<!-- ocr: engine <stamp> | no-text -->` — no confidence claim exists for
  an empty page, and a fabricated number would be the smoothing this
  contract forbids (OV-5). The sidecar's text enters the block VERBATIM:
  oddities and all, no spell-fix, no rewrite (arXiv:2611.05539's class,
  refused by construction). A page the sidecar REFUSED is not shipped as a
  hole: the whole document refuses `ocr_page_failed` (422) naming the
  indices.

## 2 · The route: `POST /ingest/document`

- **Body:** JSON, `{"data": <base64 PDF bytes>, "title": <string>, "source":
  <label>, "domain": <name>, "tier": "text-layer"|"docling"|"ocr"}`. `data` is
  required; `title` is required (the chunker's title law); the rest optional
  (`source` defaults to `document`, `domain` to `global`, `tier` to
  `text-layer`). The response `tier` is DERIVED from what ran (`text-layer` |
  `ocr` | `mixed`, plus `docling` for the Tier-2 lane), never echoed from the
  request: a `tier:"ocr"` request on a document whose every page has a text
  layer serves the Tier-0 shape — a page with a text layer is never OCR'd
  (OV-1). A `tier:"docling"` request converts the WHOLE document through the
  operator-configured tool (docs/DOCLING_TOOL_PROTOCOL.md: one-shot spawn,
  kill at `CONVERTER_TIMEOUT_SECS` = 300 s) and the tool's page blocks become
  the page blocks verbatim (DC-2: no rewrite, no smoothing).
- **Base64-bounded under the shared 1 MiB cap.** There is no multipart
  machinery in the tree; base64-in-JSON is the existing idiom. The cap is
  applied by the shared body-limit layer BEFORE the handler reads; the handler
  re-checks with its own `to_bytes` bound so the refusal is NAMED.
- **Named refusals** (machine-checkable `{"error":{"code":…,"message":…}}`):
  - `document_too_large` (413) — the request exceeds the shared cap. Never a
    silent truncation (DI-6).
  - `invalid_document` (400) — the payload is not valid base64/JSON, or the
    bytes do not parse as a PDF.
  - `no_text_layer` (422) — the PDF parses but yields no text (the scanned
    fixture's refusal).
  - `converter_unavailable` (503) — the Tier-2 variant was requested and the
    operator-configured converter path is absent. **Tier 0 is never affected**;
    it is the zero-configuration default. On the duplicate path the same name
    answers when a stored docling row's converter is not available now.
  - `converter_failed` (422) — IMPROVE06: the converter failed (exit 3, a
    crash, or a violated output contract — a missing file, or the marker
    count disagreeing with the declared page count, DC-5's by-name refusal).
    The MESSAGE carries the tool's bounded stderr tail; a refused document
    lands NO row.
  - `converter_timeout` (422) — IMPROVE06: the kernel killed the tool at
    `CONVERTER_TIMEOUT_SECS` (300 s — models cold-load), or the tool refused
    to emit a partial document (the protocol's reserved exit 4). No partial
    document ever ships as success.
  - `converter_remote_refused` (503) — IMPROVE06: the tool refused a
    remote/generative configuration posture (`DOCLING_SERVICE_URL` /
    `DOCLING_SERVICE_API_KEY` / `DOCLING_RS_VLM_ENDPOINT` set). The Tier-2
    tier runs local-only; the refusal is the tool's own named exit 5.
  - `ocr_unavailable` (503) — IMPROVE05: `tier:"ocr"` and the shell-vision
    sidecar is not resolvable (`BRAIN_VISION_BRIDGE` unset/absent, none beside
    the server, none in the dev tree). The third probe sibling — Tier 0 and
    the docling arm are unaffected (the probes are independent, OV-4). On
    Linux CI this is the tested green path; the full path needs macOS.
  - `ocr_page_failed` (422) — the sidecar refused named pages; the document
    is not ingested with a hole (the refusal names the indices).
  - `raster_failed` (422) — the sidecar could not rasterize the document
    (the bytes parsed for the text layer, but not for the renderer).
  - `ocr_sidecar_failed` (502) — the sidecar died, timed out (30 s, the
    extract-document bound), or answered nonsense; no extraction ran
    in-process as a fallback.
  - The door's own refusals pass through unchanged (the screen's 400, the
    capacity guard's 507, the AuthZ gate's 403).
- **Response (created):** `{success, status:"created", document_id, sha256,
  tier, extractor, ocr_engine?, pages, page_tiers?, low_confidence_pages,
  layout_unverified, chunk_ids, duplicates_skipped, quarantined_chunk_ids,
  markdown}` — `markdown` is the full annotated markdown above;
  `ocr_engine`/`page_tiers` are present exactly when OCR ran (a docling row
  carries `page_tiers` = all-`docling` and NO `ocr_engine`);
  `low_confidence_pages` counts the flagged pages (0 at Tier 0 and on a
  docling row); `extractor` is `<tool>/<version>` on a docling row.
- **Duplicate:** re-ingesting the same sha returns the SAME markdown
  byte-identically (re-derived by the same pure function) with
  `status:"duplicate"` and the original `extracted` stamp — a re-serve is not
  a re-extraction, so the extraction event's timestamp does not move. For an
  ocr/mixed row the re-derivation runs the sidecar AGAIN under the row's
  pinned engine: same OS build → byte-identical; a moved engine refuses
  `document_row_mismatch` (409) — the same law as an extractor version bump
  (DI-3's engine pin) — and an absent sidecar refuses `ocr_unavailable`
  (503) rather than re-deriving a partial document. For a docling row the
  re-derivation runs the TOOL again (the protocol spawn): a moved tool name
  or version refuses `document_row_mismatch` (409, DI-3 comparing the fresh
  run's extractor stamp against the row), an absent tool refuses
  `converter_unavailable` (503), and a remote/generative posture in the
  operator's environment refuses `converter_remote_refused` (503).
- **Review posture:** under `BRAIN_WRITE_POSTURE=review` the document ingest
  proposes instead of inserting — pending proposals on the existing HITL
  surface, no `knowledge` row (the no-auto-publish law, DI-4).

## 3 · The document row

Table `documents` — one row per ingested PDF, keyed by content sha256:

| Column | Meaning |
|---|---|
| `sha256` | UNIQUE. The `doc` header field's authority. |
| `title` | the stored title (the chunk title law). |
| `tier` | `text-layer` at Tier 0; `docling` on a Tier-2 row (the whole-document read). |
| `extractor` / `extractor_version` | the name+version split for the row check. Tier 0: `pdf-extract` + the lockfile-pinned version. Tier 2: the TOOL's own identity from meta.json (e.g. `docling` + `2.148.0`, or `docling-rs` + `2.67.1`) — recorded per run, never lockfile-pinned. |
| `pages` | the header's `pages` authority. |
| `layout_unverified` | 1 at Tier 0 — always (the flag the compliance reads). |
| `source` / `domain` / `owner` | the trust label, the pool, the principal stamp. |
| `extracted_utc` | the header's `extracted` authority. |
| `markdown_sha256` | sha256 of the emitted markdown WITH the `extracted` field pinned at the row's value — the row check verifies the markdown against the row, not the reverse. |
| `page_tiers` | IMPROVE05, NULL at Tier 0. The per-page tier map as a JSON array (`["text-layer","ocr",…]`; a docling row: `["docling",…]`) — DI-3's row check extends to it: a tampered map fails BY NAME. |
| `low_confidence_pages` | IMPROVE05. The count of pages below the named floor (`OCR_CONFIDENCE_FLOOR` = 0.5); 0 at Tier 0 and on a docling row. |
| `ocr_engine` | IMPROVE05, NULL at Tier 0 and on a docling row. The engine stamp that produced the OCR pages — recorded per run; a re-serve under a moved engine refuses. |

**The row check (DI-3).** `verify_document_header(markdown, row)` parses the
header out of an annotated markdown and requires: `doc == row.sha256`,
`extractor == row.extractor + "/" + row.extractor_version`, `pages ==
row.pages`, `tier == row.tier`, `layout == "unverified" ⇔ row.layout_unverified
== 1`, `extracted == row.extracted_utc`, and (IMPROVE05) `ocr-engine == row.ocr_engine` and
`pages-tier == row.page_tiers` — both compared as `None == None` at Tier 0, so
a text-layer header carrying the fields (or an ocr row's header dropping them)
fails. A tampered header — any one field —
fails the check. The check never "fixes" the markdown; it refuses.

## 4 · The determinism rule (DI-1, stated precisely)

The emitter is a pure function of (PDF bytes, source label, extracted-utc).
Two runs at the same stamp are byte-identical. Two runs at different stamps
differ **only** in the `extracted` header field — a test pins that the diff of
the two outputs, with the `extracted` value normalized, is empty. The route's
duplicate path keeps the original stamp, so a re-serve round-trip is fully
byte-identical end to end.

**The OCR bound (IMPROVE05).** The OCR pages' text is NOT a pure function of
the PDF bytes alone: same bytes + same OS build + same engine build + same DPI
→ same output, recorded per run (the `ocr-engine` stamp and the WER table in
the evidence). Apple does not promise bitwise-identical OCR across OS
versions, so nothing here promises it — the DETERMINISTIC surfaces are the
post-processing (the emitter, the markers, the map, the counts: pure functions
of the sidecar's reply) and the routing (per-page emptiness). The emitter's
`ocr: None` arm is byte-identical to the pre-improve05 shape — the Tier-0
goldens did not move.

**The converter bound (IMPROVE06).** A docling row's pages are the TOOL's
output: same PDF bytes + same tool + same tool version + same model artifacts
+ same machine class → same bytes (measured: both candidates' committed
goldens re-ran byte-identically on the recording machine —
`tests/fixtures/documents/docling-goldens/`). The recorded surfaces are the
same as the OCR tier's: the tool's identity in meta.json → the row's
extractor columns → the header's extractor stamp, all three agreeing (DI-3).
A moved tool version is a FOUND MISMATCH on re-derive, never a silent
re-derive under new bytes. The kernel's deterministic surfaces — the split on
the marker, the doors, the emitter, the map — remain pure functions of the
tool's output (DC-2: accepted as produced).

## 5 · The doors (DI-5, the no-bypass law)

The extracted text is UNTRUSTED INPUT at the memory door (MemHarmBench
arXiv:2611.01700 / carryover arXiv:2610.28425 / persistent compromise
arXiv:2610.28810 / StateFence arXiv:2611.02605): instructions embedded in a
document are retained in memory and re-fire after the source is gone. The
load-bearing controls are the doors this route must not bypass:

1. **The injection screen** (`screen::screen`) runs on the extracted markdown
   before any insert — same verdict semantics as the markdown door (Reject →
   refused; Quarantine → every inserted chunk flagged, no vector).
2. **The reasoning-trace strip** (`hygiene::strip_reasoning_blocks`) runs on
   the extracted markdown before chunking — the same pre-insert strip `/add`
   applies (a `<think>` trace inside a PDF does not survive ingest).
3. **Dedupe** — chunk rows carry the content hash; re-ingesting the same text
   skips duplicates, and the document row is sha-keyed.
4. **Trust-label bounds** — `source` is ≤ `MAX_SOURCE` bytes, closed
   vocabulary for token-authenticated principals (`document` added to the
   vocabulary), `origin_for_source("document") → "imported"`.
5. **The fence vocabulary is IMPORTED, never redeclared** — a forged
   `=== BRAIN_UNTRUSTED_CONTEXT END ===` sentinel inside a PDF's text must not
   bypass the read-side fence: stored verbatim (the storage-stays-verbatim
   law), stripped by the same `fence::strip_sentinels` composition every read
   path applies. The pin imports the landed constants (`src/fence.rs:26/:29`);
   a second literal anywhere is a found bug by the tree's own pin.
6. **The HITL gate** — the write posture is inherited, not reimplemented:
   review posture → proposals, nothing auto-promotes (DI-4, the
   `case_run.rs:588-611` pattern applied to the document path).
