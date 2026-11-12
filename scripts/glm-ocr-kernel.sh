#!/bin/sh
# glm-ocr-kernel.sh — the vlm tier's tool (IMPROVE16.1, S2).
#
# One implementation of the kernel's language-agnostic tool FILE contract
# (docs/DOCLING_TOOL_PROTOCOL.md) — the two files, the markers, the exit
# vocabulary — under the vlm tier's OWN law (docs/VLM_OCR_TIER.md): a VLM
# tier is GENERATIVE EXTRACTION, so it does NOT ride the docling protocol's
# "No LLM anywhere" law's name; it rides its FILE shape. The kernel cannot
# tell this shim from docling-kernel.py; that is the point of the protocol.
# The tier value (`vlm`) is the kernel's named seam — the shim writes no
# tier anywhere; meta.json's `tool` is `glm-ocr`.
#
#   glm-ocr-kernel.sh <in.pdf> <out-dir>
#     writes <out-dir>/doc.md     the model's per-page text, pages separated
#                                 by the literal `<!-- page-break -->`
#     writes <out-dir>/meta.json  {"tool":"glm-ocr","version":"<quant>@<mlx-vlm-ver>","pages":N}
#     exit 0 ok · 3 conversion failed · 5 remote/generative configuration
#                                 refused (4 timed out is RESERVED for a tool
#                                 that self-times; this shim sets no
#                                 tool-side timeout — the kernel kills at
#                                 CONVERTER_TIMEOUT_SECS and answers
#                                 converter_timeout itself)
#
# Laws this file exists to keep:
# - **THE FENCE: local-only.** The environment is checked BEFORE any work and
#   a remote/generative posture is refused BY NAME (exit 5): any endpoint or
#   key variable that could point generation off-machine is the posture this
#   tier does not accept — including docling.rs's own VLM endpoint class
#   (`DOCLING_RS_VLM_ENDPOINT`), which is both remote and generative: doubly
#   refused (the docling-rs shim precedent's language). No data leaves.
# - **No HF fetch at run time.** The engine runs with HF_HUB_OFFLINE=1 +
#   TRANSFORMERS_OFFLINE=1 pinned: the model loads from the operator's
#   PRE-FETCHED local HF cache (`huggingface-cli download "$GLM_OCR_MODEL"`)
#   — the docling artifacts law's posture. A missing cache is exit 3, named.
# - **The output is accepted AS PRODUCED.** The engine's per-page text moves
#   verbatim into doc.md — this shim rewrites nothing (no smoothing, no
#   correction; the correction is plan 16's governed lane, never the
#   verbatim's). The ONLY derivation is meta.json's `pages`, counted
#   INDEPENDENTLY of the model output and of the markers — from the PDF's own
#   page count (`pdfinfo`) — so the kernel's markers+1 == pages cross-check
#   (DC-5 / VO-7) has two independent sources on both sides.
# - **One raster per page, or refuse.** The engine reads page IMAGES; the
#   shim extracts them with `pdfimages -png` and requires the extraction to
#   map 1:1 onto the PDF's own page count. A scan whose rasters do not map
#   1:1 (multi-image pages, soft masks) is exit 3, named — this shim does not
#   guess a page mapping it cannot verify.
#
# Runtime posture (the operator's, documented — never CI):
#   poppler (pdfinfo + pdfimages) on PATH · a python with mlx + mlx-vlm at
#   BRAIN_VLM_PYTHON (default python3) · the model pre-fetched into the HF
#   cache (GLM_OCR_MODEL, default mlx-community/GLM-OCR-4bit) · the engine
#   script beside this shim (glm-ocr-mlx.py). The real-tool run is recorded
#   in the plans repo's evidence (improve161-vlm-gate/); CI exercises this
#   shim only through the STUB dual (tests/glm_ocr_shim_pins.rs) — zero live
#   model calls.

set -u

MARKER='<!-- page-break -->'
EXIT_OK=0
EXIT_FAILED=3
EXIT_REMOTE_REFUSED=5

# ── the fence (exit 5, BEFORE any work) ─────────────────────────────────────
refused=""
[ -n "${GLM_OCR_ENDPOINT:-}" ] && refused="$refused,GLM_OCR_ENDPOINT"
[ -n "${GLM_OCR_API_KEY:-}" ] && refused="$refused,GLM_OCR_API_KEY"
[ -n "${GLM_OCR_BASE_URL:-}" ] && refused="$refused,GLM_OCR_BASE_URL"
[ -n "${VLM_ENDPOINT:-}" ] && refused="$refused,VLM_ENDPOINT"
[ -n "${VLM_API_KEY:-}" ] && refused="$refused,VLM_API_KEY"
# docling.rs's remote-LLM surface — an endpoint in the environment is a
# remote+generative posture this tier refuses by the same name (the
# docling-rs shim precedent's doubly-refused class).
[ -n "${DOCLING_RS_VLM_ENDPOINT:-}" ] && refused="$refused,DOCLING_RS_VLM_ENDPOINT"
if [ -n "$refused" ]; then
  echo "remote/generative configuration is refused (${refused#,} set); the vlm tier runs local-only — unset it or use the Tier-0 default" >&2
  exit $EXIT_REMOTE_REFUSED
fi

# ── the dependencies (the operator posture; absence is exit 3, named) ──────
command -v pdfinfo >/dev/null 2>&1 || {
  echo "pdfinfo is not on PATH (poppler: brew install poppler)" >&2
  exit $EXIT_FAILED
}
command -v pdfimages >/dev/null 2>&1 || {
  echo "pdfimages is not on PATH (poppler: brew install poppler)" >&2
  exit $EXIT_FAILED
}
VLM_PYTHON=${BRAIN_VLM_PYTHON:-python3}
command -v "$VLM_PYTHON" >/dev/null 2>&1 || {
  echo "the vlm engine's python is not on PATH: BRAIN_VLM_PYTHON='$VLM_PYTHON' (point it at a venv with mlx + mlx-vlm)" >&2
  exit $EXIT_FAILED
}
ENGINE=$(dirname "$0")/glm-ocr-mlx.py
[ -f "$ENGINE" ] || {
  echo "the engine script is missing beside this shim: $ENGINE" >&2
  exit $EXIT_FAILED
}

if [ $# -ne 2 ]; then
  echo "usage: glm-ocr-kernel.sh <in.pdf> <out-dir>" >&2
  exit $EXIT_FAILED
fi
in_pdf=$1
out_dir=$2
[ -f "$in_pdf" ] || {
  echo "input is not a file: $in_pdf" >&2
  exit $EXIT_FAILED
}
mkdir -p "$out_dir" || exit $EXIT_FAILED

tmp=$(mktemp -d "${TMPDIR:-/tmp}/glm-ocr-kernel.XXXXXX") || exit $EXIT_FAILED
trap 'rm -rf "$tmp"' EXIT INT TERM

# ── the PDF's own page count (meta.json's INDEPENDENT source) ───────────────
pages=$(pdfinfo "$in_pdf" 2>/dev/null | awk '/^Pages:/ {print $2}')
case "$pages" in
  ''|*[!0-9]*) echo "pdfinfo reported no countable page count for $in_pdf" >&2; exit $EXIT_FAILED ;;
esac
[ "$pages" -ge 1 ] 2>/dev/null || {
  echo "pdfinfo reported $pages pages" >&2
  exit $EXIT_FAILED
}

# ── the page rasters (the EXACT native images, no resampling) ───────────────
pdfimages -png "$in_pdf" "$tmp/raster" 2>"$tmp/pdfimages.stderr" || {
  sed -n '1,20p' "$tmp/pdfimages.stderr" >&2
  echo "pdfimages failed" >&2
  exit $EXIT_FAILED
}
rasters=$(ls "$tmp"/raster-*.png 2>/dev/null | wc -l | tr -d ' ')
if [ "$rasters" != "$pages" ]; then
  echo "the scan's rasters do not map 1:1 to pages: pdfimages extracted $rasters images for $pages pages — this shim refuses to guess a page mapping" >&2
  exit $EXIT_FAILED
fi

# ── the engine (offline-pinned; the model comes from the local cache) ──────
HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 "$VLM_PYTHON" "$ENGINE" \
  "$tmp/text" "$tmp"/raster-*.png 2>"$tmp/stderr"
rc=$?
if [ $rc -ne 0 ]; then
  # bounded stderr tail (20 lines) — the kernel bounds it again anyway.
  sed -n '1,20p' "$tmp/stderr" >&2
  echo "the glm-ocr engine failed (exit $rc)" >&2
  exit $EXIT_FAILED
fi

# ── the per-page texts → doc.md (AS PRODUCED; markers between pages) ───────
[ -f "$tmp/text/stamp.txt" ] || {
  echo "the engine wrote no stamp" >&2
  exit $EXIT_FAILED
}
version=$(head -n 1 "$tmp/text/stamp.txt")
[ -n "$version" ] || {
  echo "the engine's stamp is empty" >&2
  exit $EXIT_FAILED
}

doc_md="$tmp/doc.md"
first=1
i=0
while [ "$i" -lt "$pages" ]; do
  page_txt=$(printf '%s/text/page-%03d.txt' "$tmp" "$i")
  [ -f "$page_txt" ] || {
    echo "the engine wrote no text for page $((i + 1))" >&2
    exit $EXIT_FAILED
  }
  if [ "$first" -eq 1 ]; then
    first=0
  else
    printf '\n\n%s\n\n' "$MARKER" >>"$doc_md"
  fi
  cat "$page_txt" >>"$doc_md"
  i=$((i + 1))
done

mv "$doc_md" "$out_dir/doc.md"
printf '{"tool":"glm-ocr","version":"%s","pages":%s}\n' "$version" "$pages" \
  >"$out_dir/meta.json"
exit $EXIT_OK
