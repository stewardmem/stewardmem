# ocr-real — the five REAL image classes (IMPROVE05 S1)

One-time generated fixtures: real rendered pixels wrapped in image-only PDFs
(no text operators — the `scanned.pdf` shape with real content). The
committed bytes are **FROZEN AT COMMIT**; `SHA256SUMS` (this directory) is
the authority. Re-running `generate-real.swift` is provenance, not a build
step — font rasterization and the noise seed may shift across macOS
versions, and a changed hash after a re-run is the EXPECTED signal that the
platform moved. The corpus stays as committed.

## Provenance (how each fixture came to exist)

Generated **once** on 2026-11-08 by `generate-real.swift` (committed beside
these bytes), run with `swift tests/fixtures/documents/ocr-real/generate-real.swift`
on the author's machine: macOS 27.2.0 arm64 (build 25C101 class), Apple
Swift 6.4.2, Xcode 27-class toolchain. Every class: US Letter, **300 DPI**
render (photo-of-page downsamples to ~200 DPI — a phone photo, not a
scanner), text drawn with CoreText, JPEG-encoded (grayscale classes through
a true 1-channel CIContext L8 render; quality 0.8, photo 0.6), wrapped in a
hand-built `%PDF-1.4` with `/Filter /DCTDecode` and computed xref (the
`generate.py` shape; no timestamps, no `/ID`).

| Fixture | Class | Font | Conditions | Expected text |
|---|---|---|---|---|
| `clean-scan.pdf` | clean scanner page | Georgia 11pt | none — the flattering case | `expected-text/clean-scan.txt` |
| `photo-of-page.pdf` | photographed page | Georgia 11pt | keystone + 1.1° tilt + 2% grain (⅛-res, upscaled) + 1.1px defocus + warm cast; ~200 DPI | `expected-text/photo-of-page.txt` |
| `multi-column-scan.pdf` | two-column scan | Georgia 10.5pt | two 2.4in columns | `expected-text/multi-column-scan.txt` |
| `handwriting.pdf` | handwriting class | Bradley Hand 22pt | wide leading | `expected-text/handwriting.txt` |
| `non-latin.pdf` | non-Latin class | PingFang SC 14pt | Simplified Chinese prose | `expected-text/non-latin.txt` |

Each `expected-text/<name>.txt` is the EXACT source text rendered into the
image — the reference the WER helper (`scripts/wer.py`) measures against.

## The one-time QA (recorded 2026-11-08, before commit)

A throwaway Vision harness (the same shape S2's sidecar uses:
`RecognizeTextRequest`, `.accurate`, language correction, auto-detected
languages, sequential pages) ran the corpus through the REAL API on the
generation machine:

| Fixture | Recognized | Page min-confidence | WER vs expected |
|---|---|---|---|
| clean-scan | 805 chars | 1.000 | **0.00%** (S=0 D=0 I=0, 139 words) |
| photo-of-page | 498 chars | 1.000 | **0.00%** (S=0 D=0 I=0, 88 words) |
| multi-column-scan | 745 chars, correct left→right order | 1.000 | **0.00%** (S=0 D=0 I=0, 129 words) |
| handwriting | 246 chars | 1.000 | **0.00%** (S=0 D=0 I=0, 44 words) |
| non-latin | **0 chars** | none | **100%** (0 of 71 words) |

**The non-latin finding (measured, kept honestly):** with auto-detected
languages — the sidecar's law — `RecognizeTextRequest` returns ZERO
observations for this Chinese page on this build, while
`supportedRecognitionLanguages()` reports 33 languages including `zh`.
Naming `zh-Hans` explicitly reads the page (7 observations, conf 0.3, with
a real character error: 旳 for 的). The gap the fixture measures is therefore
the AUTO-DETECT gap, not the script gap — the shipped posture cannot ingest
this class of scan, and the S4 table records it as the honest flag it is.
(A language-hint field is a protocol extension, deliberately not invented
here — a plan-06 note.)

Earlier generation iterations (also measured, fixed before commit): the
first photo grain (4% alpha, 8x upscale) was dense enough that Vision
returned ZERO observations — the class must measure camera conditions, not
unreadability; and a missing EOL after the `stream` keyword rendered every
page black.
