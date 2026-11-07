#!/usr/bin/env python3
"""Regenerate the deterministic text-layer PDF fixtures (IMPROVE04).

The corpus is committed WITH a SHA256SUMS manifest; this script is the
provenance of the committed bytes, not a build step. It must stay stdlib-only
(no pip install) and byte-deterministic:

  - hand-written %PDF-1.4 with computed xref offsets (the tree's
    `src/audit/decision.rs::render_pdf` is the precedent that hand-written
    PDFs are fine — and that they are the honest way to keep bytes stable),
  - Helvetica base-14 (no font embedding, no font-file drift),
  - no timestamps, no /ID, no metadata that could move between runs.

  python3 -I tests/fixtures/documents/generate.py
      # rewrites the five fixtures; SHA256SUMS must be re-committed with them
      # if (and only if) the generator itself changed.

Fixtures (the plan's S1 set):
  single-column.pdf   3 pages of prose, one column
  two-column.pdf      2 pages, left column drawn then right (Tier 0 does NOT
                      detect columns — the emitted order is content order)
  table.pdf           1 page, column-aligned tabular text
  footnotes.pdf       2 pages, body text + footnote block
  scanned.pdf         2 pages, image-only (uncompressed 4x4 RGB XObject, no
                      text operators) — the no_text_layer named-refusal test
"""

import hashlib
import pathlib
import zlib

HERE = pathlib.Path(__file__).resolve().parent

PAGE_W, PAGE_H = 612, 792  # US Letter portrait
LEFT, TOP = 54, PAGE_H - 72
LINE = 14  # leading


def esc(text: str) -> str:
    """PDF string escaping: backslash, parens. ASCII-only by construction."""
    out = text.replace("\\", r"\\").replace("(", r"\(").replace(")", r"\)")
    return out.encode("ascii", "strict").decode("ascii")


def text_stream(ops: list[str]) -> str:
    return "BT\n" + "\n".join(ops) + "\nET"


def draw_lines(x: float, y: float, font: str, size: float, lines: list[str]) -> list[str]:
    ops = [f"/{font} {size} Tf"]
    for i, line in enumerate(lines):
        yy = y - i * LINE
        ops.append(f"1 0 0 1 {x:.2f} {yy:.2f} Tm")
        if line:
            ops.append(f"({esc(line)}) Tj")
    return ops


def page_stream(ops: list[str]) -> str:
    return text_stream(ops)


def image_page_stream(x: float, y: float, w: float, h: float) -> str:
    return f"q {w:.0f} 0 0 {h:.0f} {x:.0f} {y:.0f} cm /Im0 Do Q"


def build_pdf(pages: list[tuple[str, dict]]) -> bytes:
    """pages: list of (content_stream, resources_dict). Hand-built objects,
    computed xref — the render_pdf shape.

    Object numbering (the layout the /Kids refs and /Contents refs below
    both assume): 1 = catalog, 2 = page-tree root, then per page i
    (0-based) 3 + 2i = page, 4 + 2i = content; after those, one image
    XObject per scanned document (3 + 2n), then the font last."""
    n = len(pages)
    has_image = any(res.get("image") for _, res in pages)
    image_no = 3 + 2 * n
    font_no = image_no + 1 if has_image else 3 + 2 * n

    out = bytearray(b"%PDF-1.4\n")
    offsets: list[int] = []

    def emit_obj(no: int, body: str, payload: bytes | None = None) -> None:
        offsets.append(len(out))
        out.extend(f"{no} 0 obj\n{body}\n".encode("ascii"))
        if payload is not None:
            # a stream payload: data, EOL, endstream (the EOL is not counted
            # in /Length), then endobj.
            out.extend(payload)
            out.extend(b"\nendstream\nendobj\n")
        else:
            out.extend(b"endobj\n")

    kids = " ".join(f"{3 + 2 * j} 0 R" for j in range(n))
    emit_obj(1, "<< /Type /Catalog /Pages 2 0 R >>")
    emit_obj(2, f"<< /Type /Pages /Kids [{kids}] /Count {n} >>")
    for i, (stream, res) in enumerate(pages):
        page_no = 3 + 2 * i
        content_no = page_no + 1
        if res.get("image"):
            res_str = f"<< /XObject << /Im0 {image_no} 0 R >> /Font << /F1 {font_no} 0 R >> >>"
        else:
            res_str = f"<< /Font << /F1 {font_no} 0 R >> >>"
        emit_obj(
            page_no,
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] "
            f"/Resources {res_str} /Contents {content_no} 0 R >>",
        )
        emit_obj(
            content_no,
            f"<< /Length {len(stream.encode('ascii'))} >>\nstream\n{stream}\nendstream",
        )
    if has_image:
        rgb = bytes(
            b
            for row in range(4)
            for col in range(4)
            for b in ((row * 40 + col * 10) % 256, (col * 50) % 256, (row * 60) % 256)
        )
        # body ends at "stream"; emit_obj's own newline is the single EOL
        # the spec requires before the data (a second one would shift the
        # parse of every byte that follows).
        emit_obj(
            image_no,
            "<< /Type /XObject /Subtype /Image /Width 4 /Height 4 "
            f"/ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {len(rgb)} >>\nstream",
            rgb,
        )
    emit_obj(font_no, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")

    xref_at = len(out)
    out.extend(f"xref\n0 {font_no + 1}\n".encode("ascii"))
    out.extend(b"0000000000 65535 f \n")
    for off in offsets:
        out.extend(f"{off:010} 00000 n \n".encode("ascii"))
    out.extend(
        f"trailer\n<< /Size {font_no + 1} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n".encode(
            "ascii"
        )
    )
    return bytes(out)


def prose_page(lines: list[str]) -> tuple[str, dict]:
    return page_stream(draw_lines(LEFT, TOP, "F1", 10, lines)), {}


# ── fixture content (deterministic; ASCII; real prose, not lorem) ────────────

SINGLE_COL_PAGES = [
    [
        "Local-First Memory for Agents",
        "",
        "The kernel stores what an agent learns. Every write crosses the same",
        "gate: the injection screen reads the text, the trust label bounds its",
        "provenance, and the audit chain records the write without the bytes.",
        "",
        "Recall is a fusion of three legs. The lexical leg matches exact terms",
        "through a full-text index. The semantic leg embeds the query and ranks",
        "vectors by cosine distance. The graph leg walks typed relationships so",
        "a two-hop answer can cite the path it walked.",
        "",
        "Nothing in the hot path calls a model provider. Embeddings are local,",
        "deterministic, and zero-token; the ranking is a pure function of the",
        "stored rows. A recall that cannot run offline is a recall that has",
        "already failed its own posture.",
    ],
    [
        "The Write Gate",
        "",
        "An agent write is a proposal until a human approves it. The proposal",
        "queue is the only promotion path: no insert may bypass it, and the",
        "behavioral pins prove the negative by counting rows after a write.",
        "",
        "Provenance rides every row. The source label is a bounded vocabulary,",
        "the owner stamp records the principal, and the origin field derives",
        "from the label so an agent cannot forge human authorship.",
        "",
        "Quarantine is the middle verdict. A flagged chunk is stored but never",
        "retrieved by default: it carries no vector, so the ANN top-k cannot",
        "surface it. The reviewer sees the verdict without a round trip.",
    ],
    [
        "Determinism as a Product Law",
        "",
        "Same input bytes, same output bytes. The law applies to extraction,",
        "to embedding, to ranking, and to every golden the tests pin. A path",
        "that cannot be re-run into the same bytes is a path that cannot be",
        "audited, and an unaudited path does not ship.",
        "",
        "The fence is the advisory layer around untrusted context. Storage",
        "stays verbatim; the strips compose at the boundary, and the sentinel",
        "vocabulary has exactly one definition in the tree.",
    ],
]

TWO_COL_PAGES = [
    [
        # left column first (content order = reading order in this fixture)
        *draw_lines(LEFT, TOP, "F1", 10, [
            "Left Column: Architecture",
            "",
            "The server is a single binary. The HTTP layer owns the routes, the",
            "handlers own the semantics, and the storage layer owns the rows.",
            "",
            "Each layer is replaceable behind its seam. The router merges",
            "sub-routers; the handlers call services; the services touch SQLite",
            "through one pool. Nothing reaches across two layers at once.",
            "",
            "The design keeps the memory envelope small. The hot path allocates",
            "bounded buffers and the pools cap their connections.",
        ]),
        # right column
        *draw_lines(312, TOP, "F1", 10, [
            "Right Column: Operations",
            "",
            "Operands are verbs, not daemons. The operator runs a command,",
            "reads the exit code, and closes the terminal.",
            "",
            "Backups are file copies. Restore is the copy run backward. The",
            "migration rehearsal replays a snapshot into a scratch database",
            "before the live schema moves, so the upgrade path is measured,",
            "not hoped for.",
            "",
            "Every alert names its remedy. A page without a runbook is a",
            "page that will page again.",
        ]),
    ],
    [
        *draw_lines(LEFT, TOP, "F1", 10, [
            "Left Column: Verification",
            "",
            "The suite is the largest artifact. Its shape is measured, its",
            "classes are derived by a rule, and the census is re-runnable.",
            "",
            "A pin without a control arm is a check that cannot fail. The",
            "negative side ships with every positive claim.",
        ]),
        *draw_lines(312, TOP, "F1", 10, [
            "Right Column: Economics",
            "",
            "Latency is a budget. The recall budget is single-digit",
            "milliseconds on the edge profile, and every added stage must",
            "state its cost before it lands.",
            "",
            "Storage is the scarcest resource on the small boxes. The vector",
            "store quantizes to int8 plus a binary sidecar and keeps the",
            "float32 rows only for the backfill window.",
        ]),
    ],
]

TABLE_PAGE = [
    # column-aligned rows; Tier 0 emits content order (row by row)
    *draw_lines(LEFT, TOP, "F1", 10, ["Table: Retrieval Legs"]),
    *draw_lines(LEFT, TOP - 2 * LINE, "F1", 10, [
        "leg          source            cost        notes",
        "lexical      FTS5 index        O(terms)    exact + phrase matches",
        "semantic     vec0 int8         O(k)        cosine over quantized rows",
        "graph        typed edges       O(hops)     two-hop paths, labelled",
        "fusion       RRF               O(legs)     reciprocal rank merge",
    ]),
    *draw_lines(LEFT, TOP - 8 * LINE, "F1", 10, [
        "",
        "Table: Lifecycle Verdicts",
        "verdict      stored            retrieved   operator surface",
        "clean        yes               yes         none",
        "quarantine   yes               no          review queue",
        "reject       no                no          refusal code",
        "pending      proposal only     no          HITL gate",
    ]),
]

FOOTNOTE_PAGES = [
    [
        *draw_lines(LEFT, TOP, "F1", 10, [
            "Footnotes and Provenance",
            "",
            "Scholarly pages bury their citations at the bottom of the page. A",
            "text-layer extractor sees the same reading order as any other",
            "line: body first, rule, then the note block in note order.[1]",
            "",
            "The anchor contract exists so a claim can cite the page without",
            "claiming the layout. Page-level provenance is honest; column and",
            "region provenance is a later tier.[2]",
        ]),
        *draw_lines(LEFT, 120, "F1", 8, [
            "1. The rule is drawn as text here; a real rule is a path op.",
            "2. Plans 05/06 own scan-grade and layout-aware extraction.",
        ]),
    ],
    [
        *draw_lines(LEFT, TOP, "F1", 10, [
            "Second Page, Same Shape",
            "",
            "The body continues. The footnote block anchors to the bottom of",
            "each page, so a page break between body and notes is a found",
            "artifact of the source, not of the extractor.[3]",
        ]),
        *draw_lines(LEFT, 120, "F1", 8, [
            "3. The fixture pins the behavior, not the typography.",
        ]),
    ],
]

SCAN_PAGES = [
    (image_page_stream(54, 200, 504, 500), {"image": True}),
    (image_page_stream(54, 200, 504, 500), {"image": True}),
]

# ── hostile-font-panic.pdf ──────────────────────────────────────────────────
# The isolation-boundary fixture (IMPROVE04, measured 2026-11-07): a font
# whose ToUnicode CMap covers only 'A', with NO /Encoding fallback, over text
# that uses 'B'. pdf-extract's decode_char hits its `expect("missing unicode
# map and encoding")` and PANICS — the exact RUSTSEC-2026-0187 class
# (uncatchable parser death on untrusted bytes; a legitimate arXiv paper,
# 2610.28425, trips the same expect). Under the release profile's
# panic = "abort" an in-process parse would kill the server; the child
# (extract-document) dies alone and the route refuses NAMED. The pin: this
# fixture must produce a named refusal, never a crash.
HOSTILE_CMAP = (
    "/CIDInit /ProcSet findresource begin\n"
    "12 dict begin\n"
    "begincmap\n"
    "1 begincodespacerange\n<00> <FF>\nendcodespacerange\n"
    "1 beginbfchar\n<41> <0041>\nendbfchar\n"
    "endcmap\n"
    "CMapName currentdict /CMap defineresource end\n"
    "end\n"
    "end\n"
)


def hostile_stream() -> str:
    content = "BT /F1 10 Tf 54 720 Td (B) Tj ET"
    return content, HOSTILE_CMAP


def build_hostile_pdf() -> bytes:
    content, cmap = hostile_stream()
    out = bytearray(b"%PDF-1.4\n")
    offsets: list[int] = []

    def emit(no: int, body: str, payload: bytes | None = None) -> None:
        offsets.append(len(out))
        out.extend(f"{no} 0 obj\n{body}\n".encode("ascii"))
        if payload is not None:
            out.extend(payload)
            out.extend(b"\nendstream\nendobj\n")
        else:
            out.extend(b"endobj\n")

    emit(1, "<< /Type /Catalog /Pages 2 0 R >>")
    emit(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    emit(
        3,
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        "/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
    )
    emit(4, "<< /Type /Font /Subtype /Type1 /BaseFont /MyCustomFont /ToUnicode 6 0 R >>")
    emit(5, f"<< /Length {len(content.encode('ascii'))} >>\nstream", content.encode("ascii"))
    emit(6, f"<< /Length {len(cmap.encode('ascii'))} >>\nstream", cmap.encode("ascii"))
    emit(7, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    xref_at = len(out)
    out.extend(f"xref\n0 8\n".encode("ascii"))
    out.extend(b"0000000000 65535 f \n")
    for off in offsets:
        out.extend(f"{off:010} 00000 n \n".encode("ascii"))
    out.extend(
        f"trailer\n<< /Size 8 /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n".encode("ascii")
    )
    return bytes(out)


def main() -> None:
    fixtures = {
        "single-column.pdf": build_pdf([prose_page(p) for p in SINGLE_COL_PAGES]),
        "two-column.pdf": build_pdf([(page_stream(p), {}) for p in TWO_COL_PAGES]),
        "table.pdf": build_pdf([(page_stream(TABLE_PAGE), {})]),
        "footnotes.pdf": build_pdf([(page_stream(p), {}) for p in FOOTNOTE_PAGES]),
        "scanned.pdf": build_pdf(SCAN_PAGES),
        "hostile-font-panic.pdf": build_hostile_pdf(),
    }
    manifest = []
    for name in sorted(fixtures):
        data = fixtures[name]
        (HERE / name).write_bytes(data)
        digest = hashlib.sha256(data).hexdigest()
        manifest.append(f"{digest}  {name}")
        print(f"{digest}  {name}  ({len(data)} bytes)")
    (HERE / "SHA256SUMS").write_text("\n".join(manifest) + "\n", encoding="ascii")
    print("SHA256SUMS written")


if __name__ == "__main__":
    main()
