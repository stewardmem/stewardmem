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
| `tier` | `text-layer` at Tier 0. `ocr` / `docling` are plans 05/06's vocabulary, accepted by the route's Tier-2 variant only. |
| `extracted` | RFC3339 UTC of the extraction event. **The one volatile field** — see the determinism rule below. |
| `extractor` | `<name>/<version>`, e.g. `pdf-extract/0.12.1`. The version is the resolved crate version, pinned against the lockfile by a test. |
| `layout` | `unverified` at Tier 0 — always. A `verified` value here is a found bug (DI-2's sibling pin). |

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
- A PDF with **no text layer at all** (an image-only scan) is refused NAMED:
  `no_text_layer`. Tier 0 never OCRs, never invents, never extracts garbage.

## 2 · The route: `POST /ingest/document`

- **Body:** JSON, `{"data": <base64 PDF bytes>, "title": <string>, "source":
  <label>, "domain": <name>, "tier": "text-layer"|"docling"}`. `data` is
  required; `title` is required (the chunker's title law); the rest optional
  (`source` defaults to `document`, `domain` to `global`, `tier` to
  `text-layer`).
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
    it is the zero-configuration default.
  - The door's own refusals pass through unchanged (the screen's 400, the
    capacity guard's 507, the AuthZ gate's 403).
- **Response (created):** `{success, status:"created", document_id, sha256,
  tier, extractor, pages, layout_unverified, chunk_ids, duplicates_skipped,
  quarantined_chunk_ids, markdown}` — `markdown` is the full annotated
  markdown above.
- **Duplicate:** re-ingesting the same sha returns the SAME markdown
  byte-identically (re-derived by the same pure function) with
  `status:"duplicate"` and the original `extracted` stamp — a re-serve is not
  a re-extraction, so the extraction event's timestamp does not move.
- **Review posture:** under `BRAIN_WRITE_POSTURE=review` the document ingest
  proposes instead of inserting — pending proposals on the existing HITL
  surface, no `knowledge` row (the no-auto-publish law, DI-4).

## 3 · The document row

Table `documents` — one row per ingested PDF, keyed by content sha256:

| Column | Meaning |
|---|---|
| `sha256` | UNIQUE. The `doc` header field's authority. |
| `title` | the stored title (the chunk title law). |
| `tier` | `text-layer` at Tier 0. |
| `extractor` / `extractor_version` | the name+version split for the row check. |
| `pages` | the header's `pages` authority. |
| `layout_unverified` | 1 at Tier 0 — always (the flag the compliance reads). |
| `source` / `domain` / `owner` | the trust label, the pool, the principal stamp. |
| `extracted_utc` | the header's `extracted` authority. |
| `markdown_sha256` | sha256 of the emitted markdown WITH the `extracted` field pinned at the row's value — the row check verifies the markdown against the row, not the reverse. |

**The row check (DI-3).** `verify_document_header(markdown, row)` parses the
header out of an annotated markdown and requires: `doc == row.sha256`,
`extractor == row.extractor + "/" + row.extractor_version`, `pages ==
row.pages`, `tier == row.tier`, `layout == "unverified" ⇔ row.layout_unverified
== 1`, `extracted == row.extracted_utc`. A tampered header — any one field —
fails the check. The check never "fixes" the markdown; it refuses.

## 4 · The determinism rule (DI-1, stated precisely)

The emitter is a pure function of (PDF bytes, source label, extracted-utc).
Two runs at the same stamp are byte-identical. Two runs at different stamps
differ **only** in the `extracted` header field — a test pins that the diff of
the two outputs, with the `extracted` value normalized, is empty. The route's
duplicate path keeps the original stamp, so a re-serve round-trip is fully
byte-identical end to end.

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
