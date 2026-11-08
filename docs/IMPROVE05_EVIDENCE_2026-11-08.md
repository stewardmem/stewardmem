# IMPROVE05 EVIDENCE — the OCR sidecar (Apple Vision), 2026-11-08

The execution record for
`plans/improvements/05_OCR_SIDECAR_APPLE_VISION.md` (plans repo), whose §0.2
ground corrections (re-verified 2026-11-08 at `9de29c41`) are the ground this
round ran on. Four commits, one per step, red-first. The session machine:
macOS 27.2.0 arm64, build **26B123** (the engine build every number below was
measured on), Apple Swift 6.4.2, the Xcode 27-class toolchain.

## The four commits

| Step | Commit | What landed |
|---|---|---|
| S1 | the protocol doc + the fixture corpus + the WER helper, BEFORE code | `docs/OCR_SIDECAR_PROTOCOL.md`, `hybrid.pdf` (the plan-04 generator extended; the six pre-existing hashes byte-identical), `ocr-real/` (five REAL image classes, frozen at commit with SHA256SUMS + expected-text + the committed one-time Swift generator), `scripts/wer.py` |
| S2 | `shell-vision` | the SwiftPM executable (stdin request line + PDF bytes → CoreGraphics rasterization at the requested DPI → SEQUENTIAL Swift-native `RecognizeTextRequest` → per-page verbatim text + min-candidate confidence), eight framing tests over the REAL Vision API, the bundle/provenance list-edits (`sidecar.mjs` stage generalized to both Swift sidecars, `tauri.macos.conf.json` externalBin, `release-app-store.sh:187`, the shell wiring test extended in step) |
| S3 | the kernel seam + the ocr tier + the named fallbacks, red-first | `config::ocr_bridge_path` (the third probe sibling), `probe_ocr_bridge`'s three postures, the one-shot sidecar client (30 s bound, id echo, per-page verdicts), the route's ocr arm (probe-first, per-page emptiness routing, verbatim merge, the derived tier), the emitter's OCR arm (header `ocr-engine`/`pages-tier`, per-page confidence markers under the named floor `OCR_CONFIDENCE_FLOOR = 0.5`, `no-text` pages carry no claim), the row's `page_tiers`/`low_confidence_pages`/`ocr_engine` (migration 1.32.30), DI-3 extended, rederive re-runs the sidecar under the row's pinned engine, openapi.yaml + schema.d.ts regenerated in-commit, the contract doc's OCR vocabulary |
| S4 | the fidelity measurement + the e2e (this file) | the WER table below, the real-paper scan e2e |

## The red ledger (measured, per the improve04 pattern)

The pins (`tests/ocr_sidecar_pins.rs`) were written FIRST and RUN against the
pre-pin tree. Six reds, each naming its expectation:

- **OV-4 red**: `expected 503 ocr_unavailable, got 400 unsupported_tier` —
  the refusal did not exist.
- **OV-1 red**: `expected the ocr pipeline, got 400 unsupported_tier` — the
  route had no ocr arm; the hybrid's stub run never happened.
- **OV-2 red**: same run — no `pages-tier` header field, and the row read
  failed (no `page_tiers` column yet).
- **OV-3 red**: same run — no low-confidence marker, no row count.
- **OV-5 behavioral red**: same run — the pipeline never ran.
- **OV-4 independence red**: `the ocr arm probes its own seam, got
  unsupported_tier`.

Green-on-arrival with **mutation red-proofs** (each mutation applied, the pin
watched RED, reverted, watched GREEN):

- **OV-5 structural** (the closed import list): adding `import
  FoundationModels` to the sidecar source → `the sidecar's imports are
  exactly the rasterization+recognition set — a network or model import is a
  found bug: ["CoreGraphics", "Foundation", "Vision", "FoundationModels"]`.
- **OV-6** (sequential recognition): adding a `TaskGroup`-shaped construct →
  `the sidecar source carries TaskGroup — parallel recognition is the
  measured hang hazard`.
- **DI-3's map check** (the checker mutation): removing the
  `pages-tier`/`ocr-engine` comparison from `verify_document_header` → the
  dedicated tamper pin (`di3_row_check_fails_a_tampered_map_or_engine_by_name`)
  reds `expected Err, got Ok` — a tampered map would have verified.

Also measured during the green phase (fixed on the spot, recorded because the
wire caught them):

- `CGPDFDocument.page(at:)` is **1-based** — the S2 wire test caught the
  sidecar rasterizing the WRONG page (a text-layer page's content recognized
  through an image-only request).
- The stub's id echo double-quoted (the JSON template already quotes the
  token) — the client's id-echo check refused it, as designed.
- `stub_env`'s TempDir was dropped before the route probed the stub — the
  absence pin's 503 was actually a deleted-file PathMissing; the guard now
  holds the dir for the run's whole body.
- The absence posture has TWO arms and the pins use both deliberately:
  `missing_env()` (env → /nonexistent — the deterministic PathMissing posture
  that works on every tree, including a macOS tree that has the real sidecar)
  and `absent_env()` (env unset — resolves through the sibling/dev-tree
  probe, the Linux-CI posture, the presence path on macOS).

## The fixture provenance (per fixture)

Deterministic (`tests/fixtures/documents/`, `generate.py`, committed
SHA256SUMS; re-running the generator reproduces every byte):

- `hybrid.pdf` — 4 pages: text, image-only (the 4×4 XObject), text,
  image-only. The OV-1 vehicle: the routing signal is EMPTINESS, so the
  vehicle needs no readable pixels, and the sidecar's `no-text` reply on the
  image pages is the honest verdict.

Real once (`tests/fixtures/documents/ocr-real/`, `generate-real.swift`
committed beside the bytes; **frozen at commit** — SHA256SUMS is the
authority; a re-run's changed hash is the expected signal that the platform
moved, never an overwrite):

| Fixture | How it came to exist | sha256 (head) |
|---|---|---|
| `clean-scan.pdf` | Georgia 11pt, 300 DPI grayscale, JPEG q0.8, DCTDecode XObject in a hand-built %PDF-1.4 | `a407cb59…` |
| `photo-of-page.pdf` | same text engine + keystone (1.2% pull) + 1.1° tilt + 2% grain (¼-res, 4× upscaled, 0.8px settling blur) + 1.1px defocus + warm cast, ~200 DPI RGB q0.6 | `7f2bde76…` |
| `multi-column-scan.pdf` | Georgia 10.5pt, two 2.4-inch columns, greedy word-wrap per column | `be641172…` |
| `handwriting.pdf` | Bradley Hand 22pt, wide leading | `983a51df…` |
| `non-latin.pdf` | PingFang SC 14pt, Simplified Chinese prose | `dffdf03b…` |

Each carries `expected-text/<name>.txt` — the EXACT source text rendered.

Generation iterations measured and fixed BEFORE the freeze (the README
records them): full-resolution grain was incompressible (a 5.4 MB JPEG) and,
at 4% alpha with an 8× upscale, dense enough that Vision returned ZERO
observations; a missing EOL after the `stream` keyword rendered every page
black; the ImageIO color-model keys are not public API (the JPEG came out RGB
under a `/DeviceGray` XObject and CGPDF refused to draw it) — the gray path
goes through CIContext's true 1-channel L8 format.

## The WER table (S4, measured 2026-11-08)

Method: the REAL `shell-vision` binary (`.build/debug/shell-vision`), the
protocol wire (request line + PDF bytes on stdin, one reply), one document at
a time, sequential pages; `scripts/wer.py` (the committed stdlib helper —
levenshtein over casefolded word tokens) against the fixture's expected-text.
**Engine build: `apple-vision/recognize-text@26B123`** (macOS 27.2.0,
build 26B123). Same bytes + same OS build + same engine + same DPI (300) →
same output measured this run; nothing is promised across OS builds.

| Class | WER (word tokens) | S/D/I | page min-confidence | low-confidence flag |
|---|---:|---|---:|---|
| clean-scan | **0.00%** (139 words) | 0/0/0 | 1.000 | no |
| photo-of-page | **0.00%** (88 words) | 0/0/0 | 1.000 | no |
| multi-column-scan | **0.00%** (129 words) | 0/0/0 | 1.000 | no |
| handwriting | **2.27%** (44 words) | 1/0/0 | 1.000 | no |
| non-latin | **10.00%** (20 whitespace tokens) | 2/0/0 | 0.500 | **at the floor** |

Honest notes the table needs:

- **handwriting** S=1 is a real recognition error: `home.` → `nome.`
  (h→n). Case differences exist in the raw output and are correctly
  NOT counted (the helper casefolds).
- **multi-column-scan** was read in CORRECT left→right order this run — the
  0.00% includes the reading order. The class's known risk (interleaved
  columns) did not fire on this page; the caveat stands for the class.
- **non-latin** — two things, both recorded, neither smoothed:
  1. The whitespace-token WER overstates CJK errors (unsegmented scripts
     make whole lines single tokens; the 2 substitutions are two lines with
     one dropped character each: 有类型的→有类型, 它走过的→它走过).
     Character-level, the run is ~2 characters wrong of ~250.
  2. **The auto-detect gap is NONDETERMINISTIC on this build**: the same
     page returned ZERO observations in the S1 QA run and read at 10% in
     this run — same bytes, same engine build. Naming `zh-Hans` explicitly
     reads it reliably (measured during S1: 7 observations, conf 0.3, with a
     real character error 旳 for 的). The shipped posture is auto-detect
     (the plan's law: languages from the runtime, never hardcoded), so the
     class's honest flag is the confidence floor firing (0.5 = at-floor,
     `confidence < floor` is strictly below — the marker does NOT fire at
     exactly 0.5, and the page block records `confidence 0.5` for the
     reader). A language-hint field is a protocol extension, deliberately
     not invented this round — the plan-06 note.
- The floor fired NOWHERE this run (every min-confidence was 1.000 except
  non-latin's 0.500). The low-confidence machinery is pinned behaviorally by
  the stub pins (0.2 flags, 0.9 clean, the row count) — the REAL classes on
  this build simply do not dip below 0.5. That is the honest state of the
  measurement, not a vacuous pin.

## The e2e: one real scanned document, end to end

- Vehicle: **arXiv:2611.05539** (EditVeto — the plan's own no-correction
  citation) — `paper.pdf` sha256 `cd3f857306a59b…`, 702,525 bytes, 1.7.
- The scan: page 1 rasterized at 200 DPI (a dense two-column academic page;
  300 DPI blew the 1 MiB shared cap after base64 — 946 KB → refused by
  arithmetic, not by policy), grayscale q0.8, wrapped in the corpus's
  image-only PDF shape → `scanned-page.pdf` sha256
  `a71ba4fc56e9bea6…`, 541,118 bytes (base64 ≈ 721 KB — fits).
- The ingest: a REAL server process (`./target/debug/brain-server`, temp DB,
  token file, `BRAIN_VISION_BRIDGE` → the dev-built sidecar), `POST
  /ingest/document` over HTTP with `tier:"ocr"` → **200 created**, `tier:
  "ocr"`, `ocr_engine: apple-vision/recognize-text@26B123`, `pages: 1`,
  `page_tiers: ["ocr"]`, `low_confidence_pages: 0`, 5 chunks inserted. The
  document row, the chunks, the vec0 vectors, the sources linkage, and the
  audit row all landed through the existing write path.
- The fidelity review, markdown against the paper original (the original's
  page-1 text layer extracted through the SAME isolated child):
  - raw file WER 13.39% — inflated by the header comment and marker tokens
    plus the side-margin stamp Vision read (`arXiv:2611.05539v1 [cs.CL] 3
    Nov 2026`) — reported for completeness, not the fidelity number.
  - **content-only WER 4.39% (S=0, D=9, I=10)**: ZERO substitutions — every
    word Vision read matches the original; the 9/10 are line-boundary and
    reading-order artifacts between pdf-extract's content-stream order and
    Vision's visual reading of a two-column page. The header, authors,
    abstract, keywords, and the introduction's first paragraph all read
    faithfully (`layout: unverified` stays honest — the header never claims
    column order).
  - The page marker: `<!-- ocr: engine apple-vision/recognize-text@26B123 |
    confidence 0.5 -->` — at the floor, not below it, so no
    `low-confidence` flag; the confidence rides the block for the reader.

## The Linux/macOS split (recorded honestly)

- **Linux CI (the green half)**: the absence path (`ocr_unavailable`, 503 —
  the deterministic `missing_env()` posture), all stub-sidecar pins (the
  routing, the map, the floor, the verbatim law, the duplicate/engine pin),
  and the structural pins (OV-5's closed import list, OV-6's sequential
  source) — all run and green on any OS. The REAL sidecar cannot exist on
  Linux; `real_sidecar_full_path_when_present` takes the absence arm there
  and asserts the named refusal.
- **macOS (this run, the presence half)**: the full path through the REAL
  Vision API — the presence pin (the route, the real engine, the map), the
  eight S2 framing tests, the WER table, and the e2e above. `swift build` +
  `swift test` green on this machine; the `xcode-27` CI lane builds the
  package (shell-vision rides it by living in it).
- The dual is deliberate and pinned, not a skip: the same test asserts
  whichever half its tree can prove.

## The seal (stage, on the clean tree)

`node shell/scripts/sidecar.mjs stage` — **exit 0** on the clean tree at the
round's close: both Rust release targets compiled, the stale-copy guard ran
(`brain-server <version>` round-trip), the Swift universal build produced BOTH
sidecars, and the provenance record carries **nine artifacts** — the kernel's
three files (two thin slices + the universal), and the SAME three-file law for
`brain-afm` AND `shell-vision` (universal + `lipo -thin` per target, each
arch-checked before it earned a record entry):

```
brain-server-{aarch64,x86_64,universal}-apple-darwin
brain-afm-{universal,aarch64,x86_64}-apple-darwin
shell-vision-{universal,aarch64,x86_64}-apple-darwin   <- the IMPROVE05 addition
```

Every entry carries its sha256 + size + arch set (`SIDECAR_PROVENANCE.json`,
written LAST so a found record trusts every entry it names). The bundle-half
of verify (the codesign walk, hardened runtime, `--expect-identity`) runs at
the release cut over the real signed `.app` — and `release-app-store.sh`'s
bundle law now refuses a MAS submission whose `Contents/MacOS/` lacks
`shell-vision`, so a kernel-less or vision-less bundle cannot be submitted.
The verify LOGIC itself (including `stagedFailures` — every record artifact
checked against the staged dir by sha256 + size) is pinned by the shell
suite's `sidecar-bundle.test.ts` (8 tests, green).

## Lanes

- `cargo fmt --check`: green.
- `cargo clippy --all-targets -- -D warnings` (root): green, AND each of the
  six feature-matrix lanes measured green on this host (`compliance-pack`,
  `multivec`, `injection-classifier`, `neural-embed`, `loom`, `rerank-tier`)
  — two clippy findings fixed en route (a `find` fold in the new probe, a
  redundant closure in the row check).
- lipstyk (the diff watchdog): the round's diff carried ONE warn
  (memory.rs:2574, the verbose two-arm match) — folded to `.map()/
  unwrap_or`, re-run clean (0 diagnostics). The improve04 closing warn class,
  closed for this round too.
- Kernel battery: **the record stands and improves — 3,873 passed / 0
  failed** (the improve04 close-out was 3,847/0; the +26 are this round's
  kernel-side pins), derived exactly as the badge derives it (the
  platform-normalized skip list). The battery ALSO caught the schema-stamp
  drift-guards the 1.32.30
  bump moved (six pins across storage_layout/main_suite/clean_cycle/
  per_domain_axis/rbac_evaluation/version_axis, plus the error-taxonomy
  table gaining `OcrSidecarError` and the migration's both-arms/
  refuse-newer-probe pins) — each moved WITH the stamp, the same re-pin
  every recorded round made, all green after.
- `cargo deny check`: green — root (`advisories licenses bans sources`) AND
  the full 8-workspace policy loop (`crates`, `fuzz`, `client`,
  `tools/channel-bridge`, `tools/signal-gateway`, `tools/steward-harness`,
  `shell/src-tauri`). NO new dependencies this round; the lock is untouched.
- Shell: `pnpm check` (0 errors), `pnpm lint` clean, `pnpm test` 404 passed
  (the sidecar wiring test extended in step).
- `swift build` + `swift test` (shell-macos): green on this macOS host — 8
  framing tests over the REAL Vision API plus the AFM probe's suites.
- Census: re-derived — `test-census.py --check` green.
- Badge: re-derived from the real run — **3,865 → 3,873** (the
  `badges.sh` derivation), the README badge carries the new number, and
  `--selfcheck` is green.
- docs-truth: exit 0, identical to the baseline run (the LOW findings are
  the pre-existing static-asset routes, untouched).
- The schema-version lockstep triple (storage_layout's
  `LATEST_KNOWN_SCHEMA`, migration's stamp, delivery's atomic-unit pin) moved
  together to 1.32.30 and its pins were bumped with it — the same move every
  recorded round made.
- One PRE-EXISTING test-isolation flake fixed on the way (named, test-only):
  `rules_door_pins`' temp dir keyed on the process id was reused when a pid
  was reused across the day's repeated full runs — the stale db made the
  fixture replay refuse its own insert ("already present at revision 1").
  The dir is now cleared before use.

## The one sentence, kept

A scanned page that cannot cite itself is a rumor with a picture on it —
this round made Vision's on-device OCR a first-class tier whose every page
carries its own engine, confidence, and tier provenance, whose text crossed
the same gates as typed text, and whose failures are named rather than
smoothed.
