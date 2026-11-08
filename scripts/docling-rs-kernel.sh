#!/bin/sh
# docling-rs-kernel.sh — the Tier-2 converter tool (IMPROVE06, candidate B).
#
# One implementation of the kernel's language-agnostic tool contract
# (docs/DOCLING_TOOL_PROTOCOL.md), speaking docling.rs (`docling-rs`, the
# official Rust port — the same models as ONNX, no Python) instead of the
# Python package. The kernel cannot tell this shim from docling-kernel.py;
# that is the point of the protocol.
#
#   docling-rs-kernel.sh <in.pdf> <out-dir>
#     writes <out-dir>/doc.md     the converter's markdown, pages separated by
#                                 the literal `<!-- page-break -->`
#     writes <out-dir>/meta.json  {"tool":"docling-rs","version":"…","pages":N}
#     exit 0 ok · 3 conversion failed · 5 remote/generative config refused
#                                 (4 timed out is RESERVED for a tool that
#                                 self-times; this shim sets no tool-side
#                                 timeout — the kernel kills at
#                                 CONVERTER_TIMEOUT_SECS, because docling.rs's
#                                 own --document-timeout emits the PARTIAL
#                                 pages done so far, which would smuggle a
#                                 half-document through as success)
#
# Laws this file exists to keep:
# - **No data leaves.** Everything runs locally. `--no-url-fetch` refuses
#   URL sources outright; image sources stay at docling.rs's default `none`
#   (no remote resolution) by omission; remote/LLM configuration in the
#   environment is refused BEFORE anything runs (exit 5) — including
#   docling.rs's own VLM endpoint (`DOCLING_RS_VLM_ENDPOINT`), which is both
#   a remote call and an LLM: doubly refused (DC-1 + the no-LLM law).
# - **No LLM anywhere.** The STANDARD pipeline is docling-rs's default and
#   stays the default by omission: no `--pipeline vlm`, no `vlm_*` options,
#   no picture-description surface appears in this file (the structural pin
#   reads it).
# - **The output is accepted AS PRODUCED.** The markdown moves verbatim from
#   the tool to doc.md — this shim rewrites nothing (no smoothing, no
#   correction; arXiv:2610.30026's cue-loss class is measured, never fixed).
#   The only derivation is meta.json's `pages`, counted INDEPENDENTLY of the
#   markers from the converter's own document JSON (the brace-depth count of
#   the `pages` object), so the kernel's markers+1 == pages cross-check has
#   two independent sources on both sides.
#
# Runtime posture: a POSIX sh + the `docling-rs` binary on PATH (prebuilt for
# macOS arm64 / Linux x64+arm64, or `cargo install docling-cli`). The binary
# resolves its `.models/` relative to its own (symlink-resolved) location —
# install per docs/DOCLING_TOOL_PROTOCOL.md and nothing else is needed.

set -u

MARKER='<!-- page-break -->'
EXIT_OK=0
EXIT_FAILED=3
EXIT_REMOTE_REFUSED=5

refused=""
[ -n "${DOCLING_SERVICE_URL:-}" ] && refused="$refused,DOCLING_SERVICE_URL"
[ -n "${DOCLING_SERVICE_API_KEY:-}" ] && refused="$refused,DOCLING_SERVICE_API_KEY"
# docling.rs's own remote-LLM surface (OPTIONS.md): an endpoint in the
# environment would silently turn the standard pipeline generative+remote.
[ -n "${DOCLING_RS_VLM_ENDPOINT:-}" ] && refused="$refused,DOCLING_RS_VLM_ENDPOINT"
if [ -n "$refused" ]; then
  echo "remote/generative configuration is refused (${refused#,} set); the converter runs local-only — unset it or use the Tier-0 default" >&2
  exit $EXIT_REMOTE_REFUSED
fi

command -v docling-rs >/dev/null 2>&1 || {
  echo "docling-rs is not on PATH (the operator installs it beside its .models/)" >&2
  exit $EXIT_FAILED
}

if [ $# -ne 2 ]; then
  echo "usage: docling-rs-kernel.sh <in.pdf> <out-dir>" >&2
  exit $EXIT_FAILED
fi
in_pdf=$1
out_dir=$2
[ -f "$in_pdf" ] || {
  echo "input is not a file: $in_pdf" >&2
  exit $EXIT_FAILED
}
mkdir -p "$out_dir" || exit $EXIT_FAILED

tmp=$(mktemp -d "${TMPDIR:-/tmp}/docling-rs-kernel.XXXXXX") || exit $EXIT_FAILED
trap 'rm -rf "$tmp"' EXIT INT TERM

# The deterministic STANDARD pipeline, pinned by omission (no --pipeline
# flag exists on this invocation line); --no-url-fetch is the no-data-leaves
# belt-and-braces; --images placeholder is docling.rs's default, stated.
docling-rs \
  --no-url-fetch \
  --to md --to json \
  --page-break-placeholder "$MARKER" \
  --images placeholder \
  "$in_pdf" --output "$tmp" 2>"$tmp/stderr"
rc=$?
if [ $rc -ne 0 ]; then
  # bounded stderr tail (20 lines) — the kernel bounds it again anyway.
  sed -n '1,20p' "$tmp/stderr" >&2
  echo "docling-rs failed (exit $rc)" >&2
  exit $EXIT_FAILED
fi

stem=$(basename "$in_pdf")
stem=${stem%.*}
[ -f "$tmp/$stem.md" ] || {
  echo "the tool wrote no markdown for $stem" >&2
  exit $EXIT_FAILED
}
[ -f "$tmp/$stem.json" ] || {
  echo "the tool wrote no document JSON for $stem" >&2
  exit $EXIT_FAILED
}

# meta.json's page count, counted from the converter's own document model
# (the JSON's `pages` object), NOT from the markdown markers — independence
# is what gives the kernel's DC-5 cross-check two sides. awk tracks brace
# depth: a `{` opened while depth==1 inside `pages` is one page object.
pages=$(awk '
  /"pages" *: *\{/ { inpages = 1; depth = 1; next }
  inpages {
    opens = gsub(/\{/, "{")
    closes = gsub(/\}/, "}")
    if (opens > 0 && depth == 1) count++
    depth += opens - closes
    if (depth <= 0) { print count; exit }
  }
' "$tmp/$stem.json")
if [ -z "$pages" ] || [ "$pages" -lt 1 ] 2>/dev/null; then
  echo "the document JSON carries no countable pages object" >&2
  exit $EXIT_FAILED
fi

mv "$tmp/$stem.md" "$out_dir/doc.md"
version=$(docling-rs --version | sed 's/^docling-rs //; s/ .*//')
printf '{"tool":"docling-rs","version":"%s","pages":%s}\n' "$version" "$pages" \
  >"$out_dir/meta.json"
exit $EXIT_OK
