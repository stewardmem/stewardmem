# OCR SIDECAR PROTOCOL — `shell-vision` (IMPROVE05)

The wire contract between the kernel (`brain-server`) and the OCR sidecar
(`shell-vision`, the SwiftPM executable sibling of `brain-afm` in
`shell-macos/`). The plan is `plans/improvements/05_OCR_SIDECAR_APPLE_VISION.md`
(plans repo), its §0.2 the ground record; the doc-ingest contract this plugs
into is `docs/DOCUMENT_INGESTION_CONTRACT.md`.

## The lifecycle (the extract-document pattern, brain-afm's framing)

**One-shot per document.** The kernel spawns the sidecar, writes ONE request
(a single JSON line) followed by the raw PDF bytes, reads ONE typed reply
line, and reaps the child. There is no session, no second request, no
long-lived process — OCR is stateless, so a hung or crashed child poisons no
later document, and the kernel's bounded wait (30 s, the extract-document
bound) applies cleanly to exactly one document. The JSONL *framing*
mirrors brain-afm's: one writer for stdout, sorted-key JSON, typed replies,
named error codes — never prose a caller parses.

## The request

```json
{"id":"o1","op":"ocr","dpi":300}
```

then, immediately after the request line's newline, the raw PDF bytes on the
same stdin, to EOF. (Two-body stdin: line one is the request; everything
after it is the document. The sidecar reads line one, then reads the
remainder to EOF as the PDF bytes.)

| Field | Rule |
|---|---|
| `id` | required, echoed verbatim in the reply. Unreadable/absent → the `bad_request` reply carries `"id":null`. |
| `op` | required, exactly `"ocr"`. Anything else → `bad_request`. |
| `dpi` | optional, integer, default 300, accepted band 72–600 (outside → `bad_request`). The KERNEL always sends its named constant (`config::OCR_RASTER_DPI` = 300); the sidecar rasterizes at the requested DPI, so the DPI is part of the determinism tuple (see below). |

**Additive-optional `pages` field** (the per-page routing, OV-1): the kernel
may mark WHICH 1-based page indices it wants recognized —
`{"id":"o1","op":"ocr","dpi":300,"pages":[2,4]}`. Absent (or empty) means ALL
pages. A page index outside `1..=pageCount` → `bad_request`. The sidecar
still rasterizes nothing it does not need: only the requested pages are
rasterized and recognized; the reply carries exactly the requested pages (all
pages when the field is absent). **The kernel never sends a page that has a
text layer** — the routing signal is per-page emptiness of the Tier-0
extraction, and a text-layer page is never OCR'd.

## The reply (one JSON line on stdout, sorted keys)

Success — every requested page answered:

```json
{"engine":"apple-vision/recognize-text@25C101","id":"o1","ok":true,
 "pages":[{"confidence":0.87,"index":1,"text":"…"},
          {"index":2,"ok":false,"code":"ocr_page_failed","message":"…"}]}
```

(Shown wrapped for the doc; the wire is ONE line.)

| Field | Rule |
|---|---|
| `id` | the request's id, verbatim. |
| `ok` | `true` when the pipeline completed and every requested page carries a verdict (a per-page `ok:false` is still a verdict — the page ANSWERED, with a named refusal). |
| `engine` | the engine stamp: `apple-vision/<request-type>@<os-build>` — the request type (`recognize-text`; `recognize-documents` is the macOS 26+ optional enhancement, not used by this build) and the running macOS build number (`kern.osversion`). Present when `ok:true`. |
| `pages` | one entry per requested page, in ascending `index` order. |

A successful page entry: `{"index":N,"text":"…","confidence":0.87}` —
`text` is the recognition output **verbatim** (the sidecar NEVER rewrites,
smooths, or "corrects" it — arXiv:2611.05539's class, refused by
construction); `confidence` is the page's MINIMUM top-candidate confidence
(0..1, conservative honesty: one weak region marks the page). A page where
Vision recognized nothing carries `"text":""` with `confidence` **absent**
(JSON null on the wire is not used here: no text → no confidence claim —
there is nothing to distrust, and a fabricated number would be exactly the
smoothing this protocol forbids).

A failed page entry: `{"index":N,"ok":false,"code":"ocr_page_failed"|"ocr_assets_unavailable",
"message":"…"}` — a page Vision refused is a NAMED failure, never a fabricated
empty, never a retry. The message is the framework's error, verbatim.

Top-level failures — the reply line is still written, then the process exits
non-zero:

```json
{"id":"o1","ok":false,"code":"raster_failed","message":"…"}
{"id":null,"ok":false,"code":"bad_request","message":"…"}
```

| Code | Meaning |
|---|---|
| `bad_request` | The request line is not valid JSON, `op` is not `"ocr"`, `dpi` is out of band, a page index is out of range, or the stdin carried no request line at all. |
| `raster_failed` | The bytes after the request line are not a PDF CoreGraphics can open (document-level), or a requested page could not be rendered (document-level refusal — the kernel sees the named code). |
| `ocr_page_failed` | Per-page only (inside `ok:true`): Vision refused that page's recognition. Never fabricated into an empty. |
| `ocr_assets_unavailable` | Per-page only (inside `ok:true`): the machine itself failed the sidecar's capability probe — one recognition request on a synthetic CoreText-rendered text stimulus, run once per process before any page work (`SHELL_VISION_PROBE_FAIL=1` forces it — the test surface). The probe either throws (fresh CI volumes without the on-demand recognition model are the measured case) or reads nothing on obvious text; either way the machine cannot honestly OCR, every requested page carries this verdict, and the probe's evidence rides verbatim in the message. An environment verdict, not a recognition failure — the pins may honestly skip on this code and only on this code. |

## Exit codes

| Exit | Meaning |
|---|---|
| `0` | A complete reply was written and every requested page carries a verdict (all-success or per-page `ocr_page_failed`). |
| `2` | `bad_request` (the reply was written; no page could be asked). |
| `3` | `raster_failed` (the reply was written; no page could be asked). |
| other | The sidecar died before writing a reply (crash, abort) — the kernel maps this to its own named refusal, `ocr_sidecar_failed`. |

## The laws this protocol exists to keep

- **On-device, no data leaves.** Vision text recognition is on-device; the
  sidecar touches no network (no URLSession, no sockets), ships no bytes
  anywhere, and has no entitlements that could.
- **No LLM anywhere.** No FoundationModels import, no language-model call,
  no post-OCR correction path exists in the sidecar (arXiv:2611.05539:
  LLM post-OCR correction rewrites text that was already correct — so the
  correction surface does not exist; arXiv:2611.04874 frames the standard:
  faithful transcription, honesty flags where it fails).
- **Sequential recognition (OV-6).** One `RecognizeTextRequest` at a time,
  pages in ascending order — parallel recognition tasks are the measured
  hang hazard (Apple Developer Forums, the plan's §0.2.2). This is pinned
  structurally: the recognition loop performs one request per iteration and
  no task-group/concurrent construct exists in the source.
- **Determinism, honestly bounded.** Same PDF bytes + same OS build + same
  engine build + same DPI → same output, measured per run (the WER table in
  the evidence). Apple does not promise bitwise-identical OCR across OS
  versions, so nothing here promises it: the engine string RECORDS what ran,
  it never promises what will run.
- **Named, never smoothed.** A refused page is `ocr_page_failed`; an
  unrecognizable document is `raster_failed`; a malformed request is
  `bad_request`. No code path fabricates text or confidence.
- **Languages come from the runtime.** The recognition language set is the
  framework's own (`supportedRecognitionLanguages()`, 26 languages at
  2026-11-08, device/OS-dependent) — never a hardcoded list in this
  protocol or the sidecar.

## The kernel-side refusals this protocol feeds

`POST /ingest/document` at `tier:"ocr"` maps the sidecar's outcomes to the
route's named refusals (see `docs/DOCUMENT_INGESTION_CONTRACT.md` §2):
sidecar absent → `ocr_unavailable` (503); sidecar died/timeout/nonsense →
`ocr_sidecar_failed` (502); `raster_failed` → 422, propagated by name;
any page `ocr_page_failed` → 422 `ocr_page_failed` naming the indices (the
document is refused rather than shipped with a hole).
