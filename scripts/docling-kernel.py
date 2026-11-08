#!/usr/bin/env python3
"""docling-kernel.py — the Tier-2 converter tool (IMPROVE06, candidate A).

One implementation of the kernel's language-agnostic tool contract
(docs/DOCLING_TOOL_PROTOCOL.md):

    docling-kernel.py <in.pdf> <out-dir>
      writes <out-dir>/doc.md     the converter's markdown, pages separated by
                                  the literal `<!-- page-break -->`
      writes <out-dir>/meta.json  {"tool":"docling","version":"…","pages":N}
      exit 0 ok · 3 conversion failed · 5 remote-config refused
                                  (4 timed out is RESERVED for a tool that
                                  self-times; this wrapper sets no tool-side
                                  timeout — the kernel kills at
                                  CONVERTER_TIMEOUT_SECS and answers
                                  converter_timeout itself, because a
                                  tool-side deadline risks emitting a PARTIAL
                                  document that looks like success)

Laws this file exists to keep (the plan's, restated where the code enforces):

- **No data leaves.** Conversion runs locally; the only inputs are the PDF
  path and the pre-fetched local artifacts (`BRAIN_DOCLING_ARTIFACTS`, the
  `docling-tools models download` tree). Remote/API configuration is refused
  BEFORE anything runs (exit 5): `DOCLING_SERVICE_URL` /
  `DOCLING_SERVICE_API_KEY` in the environment is a posture this tool does
  not accept, and no remote client is imported anywhere (`docling
  convert-remote` — the API subcommand — appears nowhere in this file).
- **No LLM anywhere.** The STANDARD pipeline (layout + TableFormer +
  OCR-model inference) is DocumentConverter's default and stays the default
  by omission: no VlmPipeline import, no `--pipeline vlm`, no generative
  `--enrich-*` surface exists here. The 20+ generative presets the CLI
  fronts are unreachable through this wrapper by construction.
- **The output is accepted AS PRODUCED.** This script never rewrites,
  smooths, or post-corrects converter bytes (arXiv:2610.30026's cue-loss
  class is a measured limitation, never smoothed): it writes the export
  verbatim and splits nothing itself — the KERNEL splits on the marker.
  `PARTIAL_SUCCESS` is refused named (exit 3): a partial document is not
  the document.

`-I`-unsafe BY DESIGN: this wrapper runs inside the operator's venv
(`python3 -m venv .venv-docling && .venv-docling/bin/pip install docling`)
— it needs the docling package, so it is run as
`.venv-docling/bin/python scripts/docling-kernel.py …`, never `python3 -I`.
Dependencies: stdlib + docling only (the `wer.py` precedent).
"""

import json
import os
import pathlib
import sys

MARKER = "<!-- page-break -->"

EXIT_OK = 0
EXIT_FAILED = 3
EXIT_TIMEOUT = 4  # reserved — see the module docstring
EXIT_REMOTE_REFUSED = 5


def die(code: int, message: str) -> "None":
    print(message, file=sys.stderr)
    sys.exit(code)


def main() -> None:
    # The no-remote law (DC-1), enforced before any import does work: a
    # remote-services posture in the environment is refused named, exit 5.
    for key in ("DOCLING_SERVICE_URL", "DOCLING_SERVICE_API_KEY"):
        if os.environ.get(key):
            die(
                EXIT_REMOTE_REFUSED,
                f"remote-services configuration is refused ({key} is set); "
                "the converter runs local-only — unset it or use the Tier-0 default",
            )
    if len(sys.argv) != 3:
        die(EXIT_FAILED, "usage: docling-kernel.py <in.pdf> <out-dir>")
    in_pdf = pathlib.Path(sys.argv[1])
    out_dir = pathlib.Path(sys.argv[2])
    if not in_pdf.is_file():
        die(EXIT_FAILED, f"input is not a file: {in_pdf}")
    out_dir.mkdir(parents=True, exist_ok=True)

    # The pre-fetched local artifacts. Unset falls back to docling's own
    # default cache — warn honestly: the no-network-at-conversion-time
    # guarantee only holds with the pre-fetch (`docling-tools models download`).
    artifacts = os.environ.get("BRAIN_DOCLING_ARTIFACTS") or None
    if artifacts is None:
        print(
            "warning: BRAIN_DOCLING_ARTIFACTS is unset — docling falls back to "
            "its default model cache; pre-fetch artifacts for the offline posture",
            file=sys.stderr,
        )

    # Imported HERE (after the refusal gates) and only here: the operator
    # venv's docling. Stdlib-only above this line.
    from docling.datamodel.base_models import ConversionStatus, InputFormat
    from docling.datamodel.pipeline_options import PdfPipelineOptions
    from docling.document_converter import DocumentConverter, PdfFormatOption
    from docling_core.types.doc import ImageRefMode
    import docling

    pipeline_options = PdfPipelineOptions(artifacts_path=artifacts)
    # The deterministic STANDARD pipeline, pinned by omission: DocumentConverter's
    # default for PDF is StandardPdfPipeline (layout + TableFormer + OCR models)
    # and nothing here overrides it — no VlmPipeline, no enrichments, no
    # generative surface exists in this file (the structural pin reads this).
    converter = DocumentConverter(
        format_options={
            InputFormat.PDF: PdfFormatOption(pipeline_options=pipeline_options)
        }
    )
    # convert() RAISES on a refused document (it does not return a failed
    # status) — the failure becomes exit 3 with a bounded one-line cause,
    # never a traceback and never a partial out-dir.
    try:
        result = converter.convert(in_pdf)
    except Exception as exc:  # noqa: BLE001 — the tool's failure surface is wide; the contract wants one line
        cause = str(exc).strip().splitlines()
        die(EXIT_FAILED, f"conversion failed: {cause[0] if cause else exc.__class__.__name__}")
    if result.status != ConversionStatus.SUCCESS:
        die(EXIT_FAILED, f"conversion failed: status {result.status.name}")

    doc = result.document
    markdown = doc.export_to_markdown(
        page_break_placeholder=MARKER,
        image_mode=ImageRefMode.PLACEHOLDER,
    )
    (out_dir / "doc.md").write_text(markdown, encoding="utf-8")
    meta = {"tool": "docling", "version": docling.__version__, "pages": len(doc.pages)}
    (out_dir / "meta.json").write_text(json.dumps(meta), encoding="utf-8")
    sys.exit(EXIT_OK)


if __name__ == "__main__":
    main()
