//! Document ingestion core (IMPROVE04): the Tier-0 PDF → annotated-markdown
//! contract. A PDF's text layer becomes page-anchored markdown whose every
//! byte is attributable — a claim that cites a page cites a page that exists.
//! The contract this module implements is `docs/DOCUMENT_INGESTION_CONTRACT.md`;
//! the plan is `plans/improvements/04_DOCUMENT_INGESTION_CORE.md` (plans repo).
//!
//! ## The three laws this module exists to keep
//!
//! **1 · Deterministic zero-token hot path.** [`extract_text_layer`] +
//! [`emit_annotated_markdown`] are pure functions of the PDF bytes (the
//! emitted-utc rides in as a parameter, never `Utc::now()` — the route owns
//! the clock, the emitter owns determinism). No LLM anywhere in extraction;
//! Tier 0 needs no converter and no configuration.
//!
//! **2 · Untrusted bytes, bounded.** The Tier-0 parse consumes UNTRUSTED
//! input behind the shared 1 MiB body cap. RUSTSEC-2026-0187 is the standing
//! threat model for this class (nested PDF objects once SIGABRT'd the parser —
//! stack overflow, uncatchable; patched in the `lopdf ^0.42` band this crate
//! resolves). The parse must NEVER run in the recall hot path; the route runs
//! it inside `spawn_blocking` and treats a panicked parse as a named refusal
//! (`invalid_document`), not a 500.
//!
//! **3 · No bypass.** The extracted markdown crosses the EXISTING doors only
//! (the route composes them: screen, trace strip, dedupe, trust-label bounds,
//! the HITL gate). This module adds none of those doors and removes none —
//! the fence vocabulary stays in `fence.rs` (one definition, every surface;
//! the series' drift-guard), the screen stays `screen::screen`, the strip
//! stays `hygiene::strip_reasoning_blocks`.
//!
//! ## Why the page count is cross-checked
//!
//! `pdf_extract::extract_text_from_mem_by_pages` walks pages with a
//! `while let Ok(...)` loop: the FIRST page that fails to render ends the
//! walk, and a hostile PDF could silently lose every page after it. The
//! emitted `pages:` header would then understate the document — a fidelity
//! lie this module refuses. [`extract_text_layer`] cross-checks the observed
//! page count against the parsed page tree (`lopdf`) and refuses named
//! (`document_parse_incomplete`) when they disagree.
//!
//! ## Tier 0 never claims layout verified
//!
//! Multi-column reading order is NOT attempted at Tier 0: the emitted order
//! is the content-stream order. Every header carries `layout: unverified`,
//! every document row carries `layout_unverified = 1`, and
//! [`verify_document_header`] fails a header that claims otherwise. Scan-grade
//! and layout-aware extraction are plans 05/06.

use sha2::{Digest, Sha256};

/// The Tier-0 tier label. `ocr` is IMPROVE05's vocabulary; `docling` is
/// IMPROVE06's — the Tier-2 converter tier (the whole-document layout read).
pub const TIER_TEXT_LAYER: &str = "text-layer";
/// The all-OCR derived tier: every page came from the sidecar.
pub const TIER_OCR: &str = "ocr";
/// The mixed derived tier: at least one page from each source.
pub const TIER_MIXED: &str = "mixed";
/// The Tier-2 request/tier vocabulary: the whole document went through the
/// operator-configured converter (docs/DOCLING_TOOL_PROTOCOL.md). The
/// converter reads the WHOLE document (the layout model is whole-document by
/// design — no per-page mixing this round), so a docling row's map is
/// all-`docling` and the derived-tier function never produces this value.
pub const TIER_DOCLING: &str = "docling";
/// The corrected variant's tier suffix (IMPROVE16): the verbatim tier +
/// `-corrected` (e.g. `docling-corrected`), so search filters can scope
/// and the two-artifact law reads in the row itself.
pub const TIER_CORRECTED_SUFFIX: &str = "-corrected";
/// The corrected variant's row extractor (IMPROVE16): the row's `extractor`
/// column names what produced THIS artifact's text — for a corrected row,
/// the corrector. The version half carries the `<model>@<served>` stamp,
/// and DI-3 (via [`verify_document_header`]) requires the header's
/// `corrector:` field to agree with it exactly.
pub const DOC_CORRECTOR_EXTRACTOR: &str = "doc-corrector";

/// The corrected variant's tier: the verbatim tier + the corrected suffix.
pub fn corrected_tier(verbatim_tier: &str) -> String {
    format!("{verbatim_tier}{TIER_CORRECTED_SUFFIX}")
}
/// The extractor name the header and the row both carry.
pub const EXTRACTOR_NAME: &str = "pdf-extract";
/// The resolved pdf-extract version. Pinned against Cargo.lock by a test
/// (the drift-guard: a lock bump without this const is a found bug).
pub const EXTRACTOR_VERSION: &str = "0.12.1";
/// Tier 0's layout posture — the ONLY value it may emit.
pub const LAYOUT_UNVERIFIED: &str = "unverified";

/// The confidence floor under which an OCR page carries the
/// `low-confidence` marker (OV-3). Conservative honesty: page confidence is
/// the MINIMUM top-candidate confidence, so one weak region marks the page.
pub const OCR_CONFIDENCE_FLOOR: f64 = 0.5;
/// The DPI the kernel asks the sidecar to rasterize at — the request's
/// named constant (the protocol's determinism tuple: bytes + OS build +
/// engine + DPI).
pub const OCR_RASTER_DPI: u32 = 300;
/// The one-shot sidecar's wall-clock bound (the extract-document bound —
/// a hostile or hung document cannot pin a child forever).
pub const OCR_SIDECAR_TIMEOUT_SECS: u64 = 30;
/// The Tier-2 converter's wall-clock bound (IMPROVE06). Its own named
/// constant — the tool cold-loads GB-scale model artifacts, so the 30 s
/// extract-document/sidecar bound would refuse honest work. The kernel
/// kills at the bound and answers `converter_timeout` itself: a tool-side
/// deadline risks emitting a PARTIAL document that looks like success (the
/// protocol doc's exit-4 note).
pub const CONVERTER_TIMEOUT_SECS: u64 = 300;
/// The literal marker the tool protocol pins between pages in `doc.md`.
pub const CONVERTER_PAGE_MARKER: &str = "<!-- page-break -->";

// ── extraction ───────────────────────────────────────────────────────────────

/// A Tier-0 extraction failure. Named, never swallowed.
#[derive(Debug)]
pub enum DocumentExtractError {
    /// The bytes do not parse as a PDF (or the parser panicked on them).
    InvalidPdf(String),
    /// The PDF parses and has pages, but no text layer anywhere — the
    /// image-only scan. Tier 0 refuses; it never OCRs, never invents.
    NoTextLayer,
    /// The page tree has more pages than the text walk returned — the
    /// silent-truncation hazard (see the module header).
    ParseIncomplete { tree_pages: usize, got: usize },
}

impl std::fmt::Display for DocumentExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPdf(e) => write!(f, "invalid PDF: {e}"),
            Self::NoTextLayer => write!(f, "no text layer (image-only scan)"),
            Self::ParseIncomplete { tree_pages, got } => {
                write!(f, "text walk covered {got} of {tree_pages} pages")
            }
        }
    }
}

/// One document's text layer, page by page.
#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    /// Per-page text (content-stream order — Tier 0 does NOT reorder columns).
    pub pages: Vec<String>,
}

/// Extract the text layer of a PDF, page by page. Panics of the underlying
/// parser (expect-style, on malformed objects) are the CALLER's to convert —
/// the route wraps this in `spawn_blocking` and maps a join panic to
/// [`DocumentExtractError::InvalidPdf`], keeping the panic off the async
/// runtime and the refusal named.
pub fn extract_text_layer(bytes: &[u8]) -> Result<ExtractedDocument, DocumentExtractError> {
    // The page-count cross-check needs the parsed tree; load once here.
    let tree_pages = lopdf::Document::load_mem(bytes)
        .map(|doc| doc.get_pages().len())
        .map_err(|e| DocumentExtractError::InvalidPdf(e.to_string()))?;
    if tree_pages == 0 {
        return Err(DocumentExtractError::InvalidPdf(
            "parsed tree holds no pages".into(),
        ));
    }
    let pages = pdf_extract::extract_text_from_mem_by_pages(bytes)
        .map_err(|e| DocumentExtractError::InvalidPdf(e.to_string()))?;
    if pages.len() != tree_pages {
        return Err(DocumentExtractError::ParseIncomplete {
            tree_pages,
            got: pages.len(),
        });
    }
    if pages.iter().all(|p| p.trim().is_empty()) {
        return Err(DocumentExtractError::NoTextLayer);
    }
    Ok(ExtractedDocument { pages })
}

/// Lowercase hex sha256 of the source bytes — the `doc:` header field and the
/// document row's integrity anchor.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

// ── the annotated-markdown contract ─────────────────────────────────────────

/// One page's OCR provenance: its 1-based index and the page's MINIMUM
/// top-candidate confidence — `None` when Vision recognized nothing (an
/// honest no-text page carries no confidence claim, never a fabricated
/// number).
#[derive(Debug, Clone, PartialEq)]
pub struct OcrPageMark {
    pub index: usize,
    pub confidence: Option<f64>,
}

/// What the OCR sidecar produced for a document: the engine stamp that ran
/// (recorded, never promised bitwise across OS builds) and the per-page
/// marks. This is the assembly the emitter, the header, and the row all
/// read — one source of truth for the tier map, the confidence markers,
/// and the low-confidence count.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrAssembly {
    pub engine: String,
    pub pages: Vec<OcrPageMark>,
}

impl OcrAssembly {
    /// The per-page tier map over `page_count` pages: `ocr` where this
    /// assembly marked a page, `text-layer` everywhere else (OV-2's map —
    /// built from what actually ran, never from the request).
    pub fn page_tier_map(&self, page_count: usize) -> Vec<&'static str> {
        (1..=page_count)
            .map(|i| {
                if self.pages.iter().any(|p| p.index == i) {
                    "ocr"
                } else {
                    "text-layer"
                }
            })
            .collect()
    }

    /// The pages whose confidence is below the named floor (OV-3's count).
    pub fn low_confidence_count(&self) -> usize {
        self.pages
            .iter()
            .filter(|p| matches!(p.confidence, Some(c) if c < OCR_CONFIDENCE_FLOOR))
            .count()
    }
}

/// The OCR marker line for one page block: the engine, the confidence (or
/// the honest no-text absence of one), and the below-floor flag. A marker
/// rides INSIDE the block, before the anchor — the anchor still closes the
/// block, and the marker crosses the doors with the page's text.
fn ocr_marker(engine: &str, confidence: Option<f64>) -> String {
    match confidence {
        Some(c) if c < OCR_CONFIDENCE_FLOOR => {
            format!("<!-- ocr: engine {engine} | confidence {c} | low-confidence -->")
        }
        Some(c) => format!("<!-- ocr: engine {engine} | confidence {c} -->"),
        None => format!("<!-- ocr: engine {engine} | no-text -->"),
    }
}

/// What produced the pages of an extracted document — the single source the
/// emitter, the header, and the row all read (IMPROVE06's generalization of
/// the improve05 assembly: one enum, three postures, no parallel Option
/// fields to disagree).
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentProvenance {
    /// Tier 0: every page from the PDF's own text layer. No engine stamp,
    /// no per-page map — the pre-improve05 shape, byte-identical.
    TextLayer,
    /// The IMPROVE05 hybrid: the sidecar's engine stamp + per-page marks.
    Ocr(OcrAssembly),
    /// The IMPROVE06 Tier-2 converter: the whole document went through the
    /// operator-configured tool. The extractor name/version are the TOOL's
    /// (meta.json's `tool`/`version` — recorded per run, never
    /// lockfile-pinned); the header carries NO ocr-engine field and the
    /// per-page map is all-`docling` (the whole-document read).
    Converter { tool: String, version: String },
}

impl DocumentProvenance {
    /// The extractor `<name>/<version>` the header renders for this posture.
    pub fn extractor_stamp(&self) -> String {
        match self {
            Self::TextLayer => format!("{EXTRACTOR_NAME}/{EXTRACTOR_VERSION}"),
            Self::Ocr(_) => format!("{EXTRACTOR_NAME}/{EXTRACTOR_VERSION}"),
            Self::Converter { tool, version } => format!("{tool}/{version}"),
        }
    }

    /// The per-page tier map over `page_count` pages, or `None` for the
    /// postures that carry no map (Tier 0). The OCR map is what ran; the
    /// converter map is all-`docling` (the whole-document read).
    pub fn page_tier_map(&self, page_count: usize) -> Option<Vec<&'static str>> {
        match self {
            Self::TextLayer => None,
            Self::Ocr(a) => Some(a.page_tier_map(page_count)),
            Self::Converter { .. } => Some(vec![TIER_DOCLING; page_count]),
        }
    }
}

/// Emit the annotated markdown for an extracted document. Pure: the same
/// arguments always produce the same bytes. `extracted_utc` rides in (the
/// route's clock or a test's fixed stamp) — DI-1's volatile-field rule makes
/// it the ONLY input that may move the output. `provenance` is what produced
/// the pages: `TextLayer` leaves the output byte-identical to the
/// pre-improve05 shape; `Ocr` adds the engine + per-page map to the header
/// and one marker line per OCR page block; `Converter` stamps the tool's
/// extractor name/version and the all-`docling` map — and NEVER an
/// ocr-engine field or a confidence marker.
pub fn emit_annotated_markdown(
    extracted: &ExtractedDocument,
    sha256: &str,
    source: &str,
    tier: &str,
    extracted_utc: &str,
    layout: &str,
    provenance: &DocumentProvenance,
) -> String {
    let mut out =
        String::with_capacity(128 + extracted.pages.iter().map(|p| p.len() + 40).sum::<usize>());
    let ocr_engine_field = match provenance {
        DocumentProvenance::Ocr(a) => format!(" | ocr-engine: {}", a.engine),
        _ => String::new(),
    };
    let pages_tier_field = provenance
        .page_tier_map(extracted.pages.len())
        .map(|map| format!(" | pages-tier: {}", map.join(",")))
        .unwrap_or_default();
    out.push_str(&format!(
        "<!-- doc: {sha256} | source: {source} | pages: {n} | tier: {tier}{ocr_engine_field}\n     extracted: {extracted_utc} | extractor: {stamp} | layout: {layout}{pages_tier_field} -->\n",
        n = extracted.pages.len(),
        stamp = provenance.extractor_stamp(),
    ));
    for (i, page) in extracted.pages.iter().enumerate() {
        let n = i + 1;
        out.push_str(&format!("\n## Page {n}\n"));
        // The page text is already door-processed (the route strips traces
        // per page BEFORE this call, so an anchor can never be swallowed by
        // a `<think>` span that closes after it). Trim the trailing newline
        // the extractor appends so the anchor lands deterministically.
        out.push_str(page.trim_end_matches('\n'));
        if let DocumentProvenance::Ocr(assembly) = provenance
            && let Some(mark) = assembly.pages.iter().find(|p| p.index == n)
        {
            out.push('\n');
            out.push_str(&ocr_marker(&assembly.engine, mark.confidence));
        }
        out.push_str(&format!("\n<!-- anchor: p{n} -->\n"));
    }
    out
}

// ── the corrected variant's emission (IMPROVE16 S2) ─────────────────────────

/// Emit the CORRECTED variant's header. The route owns this emission
/// wholesale — the corrector never authors the header (a hostile or
/// careless endpoint cannot forge the doc sha, the source, or the stamp);
/// it edits the page TEXT, the route re-headers. The lineage fields come
/// from the VERBATIM header (source, page count, layout posture, the OCR
/// provenance); the correction vocabulary is the route's: the corrected
/// tier, the doc-corrector extractor stamp, and the `corrector:` field
/// beside it. `doc:` carries `doc_sha256` — the route passes sha(M_v), the
/// corrector's INPUT (the lineage anchor; the corrected row's sha256 IS
/// this value, and DI-3 checks the header against the row). Pure.
pub fn emit_corrected_header(
    verbatim: &DocumentHeader,
    doc_sha256: &str,
    extracted_utc: &str,
    corrector_stamp: &str,
) -> String {
    let ocr_engine_field = match &verbatim.ocr_engine {
        Some(e) => format!(" | ocr-engine: {e}"),
        None => String::new(),
    };
    let pages_tier_field = verbatim
        .pages_tier
        .as_ref()
        .map(|map| format!(" | pages-tier: {}", map.join(",")))
        .unwrap_or_default();
    format!(
        "<!-- doc: {sha} | source: {source} | pages: {n} | tier: {tier}{ocr_engine_field}\n     extracted: {utc} | extractor: {extractor}/{stamp} | layout: {layout}{pages_tier_field} | corrector: {stamp} -->\n",
        sha = doc_sha256,
        source = verbatim.source,
        n = verbatim.pages,
        tier = corrected_tier(&verbatim.tier),
        utc = extracted_utc,
        extractor = DOC_CORRECTOR_EXTRACTOR,
        stamp = corrector_stamp,
        layout = verbatim.layout,
    )
}

/// Assemble the corrected markdown: the route's own header + the
/// corrector's BODY — everything from the first `## Page ` heading on. The
/// corrector's reasoning blocks are stripped FIRST (a local model's
/// `<think>` span never survives ingest — the same hygiene the verbatim
/// pages cross). An output with NO page structure refuses NAMED (the
/// contract error the route surfaces as `correction_contract_violation`).
/// The STRUCTURE check itself is [`verify_anchor_contract`], which the
/// route runs on the assembled markdown — this function is the assembly,
/// the contract is the cage.
pub fn assemble_corrected_markdown(
    corrector_output: &str,
    header: String,
) -> Result<String, String> {
    let stripped = crate::hygiene::strip_reasoning_blocks(corrector_output);
    let body = match stripped.find("## Page ") {
        Some(at) => &stripped[at..],
        None => return Err("the corrector output carries no page structure".to_string()),
    };
    let mut out = header;
    out.push_str(body);
    // The body must end deterministically (a trailing newline after the
    // final anchor, the emitter's shape) — normalize a missing final one.
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

/// Verify the structural anchor contract (DI-2): one `## Page N` heading per
/// page, monotonic 1..=pages, every block closed by `<!-- anchor: pN -->`
/// before the next heading or EOF. Pure — the route runs it against its OWN
/// output before returning (fail-closed: the emitter refusing to ship a
/// markdown that violates the contract it exists to provide).
pub fn verify_anchor_contract(markdown: &str, expected_pages: usize) -> Result<(), String> {
    let mut seen = 0usize;
    let mut pending: Option<usize> = None;
    for line in markdown.lines() {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("## Page ") {
            // a new heading closes the previous block — which must have
            // carried its anchor by now.
            if let Some(prev) = pending {
                return Err(format!("page {prev} block carries no anchor"));
            }
            let n: usize = rest
                .trim()
                .parse()
                .map_err(|_| format!("page heading is not a number: {line:?}"))?;
            if n != seen + 1 {
                return Err(format!(
                    "page headings not monotonic from 1: saw Page {n} after {seen} pages"
                ));
            }
            pending = Some(n);
            seen += 1;
        } else if let Some(n) = pending
            && line == format!("<!-- anchor: p{n} -->")
        {
            pending = None;
        }
    }
    if let Some(n) = pending {
        return Err(format!("page {n} block carries no anchor"));
    }
    if seen != expected_pages {
        return Err(format!(
            "page count mismatch: {seen} blocks for {expected_pages} declared pages"
        ));
    }
    Ok(())
}

// ── the machine-checkable header (DI-3) ─────────────────────────────────────

/// The parsed header fields of an annotated markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHeader {
    pub doc: String,
    pub source: String,
    pub pages: usize,
    pub tier: String,
    pub extracted: String,
    pub extractor: String,
    pub layout: String,
    /// IMPROVE05: the engine stamp, present exactly when OCR ran.
    pub ocr_engine: Option<String>,
    /// IMPROVE05: the per-page tier map (`text-layer`/`ocr` per page),
    /// present exactly when OCR ran.
    pub pages_tier: Option<Vec<String>>,
    /// IMPROVE16: the corrector stamp (`<model>@<served>`), present exactly
    /// when the markdown is a CORRECTED variant — beside (never instead of)
    /// the extractor field. DI-3 requires it to agree with the row's stamp.
    pub corrector: Option<String>,
}

/// Parse the header comment out of an annotated markdown. The header is the
/// FIRST line's `<!-- doc: ...` through the closing `-->` (the contract's
/// two-physical-line shape). Each PHYSICAL line splits on `|` — folding the
/// newline into one string would merge the last field of line one into the
/// first of line two (measured: `tier:` swallowed `extracted:`), so the field
/// set is the union across lines, order-checked by key, not by column.
pub fn parse_document_header(markdown: &str) -> Result<DocumentHeader, String> {
    let start = markdown
        .find("<!-- doc: ")
        .ok_or("no document header comment found")?;
    let end = markdown[start..]
        .find("-->")
        .ok_or("document header comment is never closed")?;
    let inner = &markdown[start + 4..start + end];
    let mut fields = DocumentHeaderFields::default();
    for line in inner.lines() {
        for part in line.split('|') {
            let part = part.trim();
            let Some((k, v)) = part.split_once(':') else {
                continue;
            };
            let v = v.trim();
            match k.trim() {
                "doc" => fields.doc = Some(v.to_string()),
                "source" => fields.source = Some(v.to_string()),
                "pages" => {
                    fields.pages = Some(
                        v.parse()
                            .map_err(|_| format!("header pages is not a number: {v:?}"))?,
                    )
                }
                "tier" => fields.tier = Some(v.to_string()),
                "extracted" => fields.extracted = Some(v.to_string()),
                "extractor" => fields.extractor = Some(v.to_string()),
                "layout" => fields.layout = Some(v.to_string()),
                // IMPROVE05: the OCR extension fields — present exactly when
                // OCR ran; unknown fields stay ignored (the union parse).
                "ocr-engine" => fields.ocr_engine = Some(v.to_string()),
                "pages-tier" => {
                    fields.pages_tier = Some(
                        v.split(',')
                            .map(|t| t.trim().to_string())
                            .filter(|t| !t.is_empty())
                            .collect(),
                    )
                }
                // IMPROVE16: the corrected variant's stamp — beside (never
                // instead of) `extractor`; present exactly when the markdown
                // is a corrected variant.
                "corrector" => fields.corrector = Some(v.to_string()),
                _ => {}
            }
        }
    }
    let missing = |f: &Option<String>, name: &str| {
        f.clone()
            .ok_or_else(|| format!("header field missing: {name}"))
    };
    Ok(DocumentHeader {
        doc: missing(&fields.doc, "doc")?,
        source: missing(&fields.source, "source")?,
        pages: fields.pages.ok_or("header field missing: pages")?,
        tier: missing(&fields.tier, "tier")?,
        extracted: missing(&fields.extracted, "extracted")?,
        extractor: missing(&fields.extractor, "extractor")?,
        layout: missing(&fields.layout, "layout")?,
        ocr_engine: fields.ocr_engine,
        pages_tier: fields.pages_tier,
        corrector: fields.corrector,
    })
}

#[derive(Default)]
struct DocumentHeaderFields {
    doc: Option<String>,
    source: Option<String>,
    pages: Option<usize>,
    tier: Option<String>,
    extracted: Option<String>,
    extractor: Option<String>,
    layout: Option<String>,
    ocr_engine: Option<String>,
    pages_tier: Option<Vec<String>>,
    corrector: Option<String>,
}

// ── the tool-detector seam (S4, the Tier-2 plumbing and nothing else) ────────

/// What the converter-path probe observed. The seam DETECTS; the conversion
/// itself is plans 05/06 — this type exists so the route's refusal is a
/// posture the operator can act on, never a bare error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConverterProbe {
    /// `BRAIN_DOCLING_PATH` unset/empty — the operator has not configured a
    /// converter. Tier 0 needs none.
    Unconfigured,
    /// Configured, but the path does not exist (or is not a file).
    PathMissing,
    /// Configured and present. The route still refuses (named) — the
    /// conversion is plans 05/06's work — but the posture differs: the
    /// operator's configuration is GOOD.
    Present(std::path::PathBuf),
}

/// What the OCR-sidecar probe observed (IMPROVE05 — the third probe
/// sibling, beside `ConverterProbe`). `Absent`/`PathMissing` both answer
/// the route's `ocr_unavailable` (503, OV-4's exact name — NOT the docling
/// arm's `converter_unavailable`); `Present` runs the hybrid pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrProbe {
    /// Nothing resolvable: `BRAIN_VISION_BRIDGE` unset, no shell-vision
    /// beside the running kernel, no dev-tree build. The Linux-CI posture.
    Absent,
    /// The env named a path that does not exist (or is not a file).
    PathMissing(std::path::PathBuf),
    /// The sidecar is there. The hybrid pipeline runs against it.
    Present(std::path::PathBuf),
}

/// Probe the resolved OCR sidecar path. Pure: the resolution happens in
/// `config::ocr_bridge_path()`; this takes the resolved option so the three
/// arms are testable without env manipulation (the converter probe's shape).
pub fn probe_ocr_bridge(resolved: Option<&std::path::Path>) -> OcrProbe {
    match resolved {
        None => OcrProbe::Absent,
        Some(p) if p.is_file() => OcrProbe::Present(p.to_path_buf()),
        Some(p) => OcrProbe::PathMissing(p.to_path_buf()),
    }
}

/// Probe the configured converter path. Pure: the env read happens in
/// `config::document_converter_path()`; this takes the resolved option so the
/// three arms are testable without env manipulation.
pub fn probe_document_converter(configured: Option<&std::path::Path>) -> ConverterProbe {
    match configured {
        None => ConverterProbe::Unconfigured,
        Some(p) if p.is_file() => ConverterProbe::Present(p.to_path_buf()),
        Some(_) => ConverterProbe::PathMissing,
    }
}

// ── the isolation boundary (the child parse) ─────────────────────────────────

/// How the child extraction failed, from the PARENT's seat. The child's
/// internal cause is unknowable by design (its panic/abort never reaches
/// this process); only the observable outcome is.
#[derive(Debug)]
pub enum ChildExtractError {
    /// The child binary is not beside the server (or not built in the dev
    /// tree). Tier 0 is unavailable rather than process-lethal: the parse
    /// never runs in-process as a fallback.
    ChildMissing,
    /// The child died, timed out, or answered nonsense. Hostile bytes are
    /// the expected cause (the parser's abort surface); the route answers
    /// `invalid_document`.
    ChildFailed,
    /// The child answered the deterministic no-text-layer verdict (exit 9).
    NoTextLayer,
}

impl std::fmt::Display for ChildExtractError {
    /// Fixed strings only — the child's internals are unknowable by design,
    /// so there is nothing raw to format.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChildMissing => write!(f, "the extraction child is not installed"),
            Self::ChildFailed => write!(f, "the extraction child failed"),
            Self::NoTextLayer => write!(f, "no text layer"),
        }
    }
}

/// Locate the extraction child: the sibling of the running binary in
/// production (`<install>/brain-server` + `<install>/extract-document`), or
/// the cargo target dir in dev/test (`cargo test` builds `[[bin]]` targets,
/// so `target/debug/extract-document` exists under the test profile).
pub fn resolve_child_exe() -> Option<std::path::PathBuf> {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("extract-document");
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    // dev/test: the cargo target dir. The profile is the build profile of
    // THIS binary (debug_assertions ⇒ debug).
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let manifest = env!("CARGO_MANIFEST_DIR");
    let target = std::path::Path::new(manifest)
        .join("target")
        .join(profile)
        .join("extract-document");
    if target.is_file() {
        return Some(target);
    }
    None
}

/// Run the Tier-0 parse in the isolated child. THE ONLY extraction entry the
/// route uses: a parser death is the child's problem, the parent's answer is
/// a named refusal. Bounded (30 s, the /add-closure precedent) — a hostile
/// document cannot pin a child forever.
pub fn extract_pages_via_child(
    exe: &std::path::Path,
    bytes: &[u8],
) -> Result<ExtractedDocument, ChildExtractError> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ChildExtractError::ChildMissing)?;
    // write the bytes; a child that dies mid-write surfaces at wait().
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(bytes);
        // drop closes stdin so the child's read_to_end completes.
    }
    // bounded wait: read stdout to completion with a wall-clock cap.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let output = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break child
                    .stdout
                    .take()
                    .map(|mut o| {
                        let mut buf = Vec::new();
                        let _ = std::io::Read::read_to_end(&mut o, &mut buf);
                        (status, buf)
                    })
                    .unwrap_or((status, Vec::new()));
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ChildExtractError::ChildFailed);
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
            Err(_) => return Err(ChildExtractError::ChildFailed),
        }
    };
    let (status, stdout) = output;
    if status.code() == Some(9) {
        return Err(ChildExtractError::NoTextLayer);
    }
    if !status.success() {
        return Err(ChildExtractError::ChildFailed);
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&stdout).map_err(|_| ChildExtractError::ChildFailed)?;
    let pages: Vec<String> = parsed
        .get("pages")
        .and_then(|p| p.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .ok_or(ChildExtractError::ChildFailed)?;
    if pages.is_empty() {
        return Err(ChildExtractError::ChildFailed);
    }
    Ok(ExtractedDocument { pages })
}

// ── the OCR sidecar client (IMPROVE05) ──────────────────────────────────────

/// The sidecar lifecycle is extract-document's (docs/OCR_SIDECAR_PROTOCOL.md):
/// one-shot per document — the request line + the PDF bytes on stdin, ONE
/// typed reply line on stdout, exit codes as verdicts, the 30 s bound. A
/// hung or crashed child poisons no later document.
#[derive(Debug)]
pub enum OcrSidecarError {
    /// The sidecar could not be spawned, died, timed out, answered
    /// nonsense, or broke the framing (id mismatch, unknown code). The
    /// route answers `ocr_sidecar_failed` (502).
    SidecarFailed,
    /// The sidecar's own top-level `raster_failed` — the bytes it was
    /// given do not rasterize (propagated by name, 422).
    RasterFailed,
    /// Per-page `ocr_page_failed` verdicts, ascending — a page Vision
    /// refused. Never a fabricated empty; the route refuses the document
    /// naming the indices.
    PageFailed(Vec<usize>),
    /// The reply carried a page index outside the document (or a malformed
    /// page entry). A contract violation, not a document property — 502.
    BadReply(String),
}

impl std::fmt::Display for OcrSidecarError {
    /// Fixed strings only — the child's internals are unknowable by design
    /// (the ChildExtractError precedent), so there is nothing raw to format.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SidecarFailed => {
                write!(f, "the OCR sidecar died, timed out, or answered nonsense")
            }
            Self::RasterFailed => write!(f, "the OCR sidecar could not rasterize the document"),
            Self::PageFailed(indices) => {
                write!(f, "the OCR sidecar refused pages {indices:?}")
            }
            Self::BadReply(what) => write!(f, "the OCR sidecar's reply broke the framing: {what}"),
        }
    }
}

/// One recognized page from the sidecar's reply.
#[derive(Debug, Clone)]
pub struct OcrSidecarPage {
    pub index: usize,
    pub text: String,
    pub confidence: Option<f64>,
}

/// A complete sidecar reply: the engine stamp (recorded per run) + the
/// per-page verdicts for exactly the requested pages.
#[derive(Debug, Clone)]
pub struct OcrSidecarReply {
    pub engine: String,
    pub pages: Vec<OcrSidecarPage>,
}

/// Run one document through the OCR sidecar. `pages` marks the 1-based
/// indices to recognize (OV-1: the kernel names ONLY the pages the text
/// walk found empty; `None` = all pages, the exit-9 all-scan case).
/// `doc_pages` is the page count the extraction observed (0 when the child
/// answered no_text_layer and the count is unknown — the all-pages reply
/// then defines it). Bounded like [`extract_pages_via_child`]; the reply's
/// id must echo the request's.
pub fn ocr_pages_via_sidecar(
    exe: &std::path::Path,
    pdf_bytes: &[u8],
    pages: Option<&[usize]>,
    doc_pages: usize,
) -> Result<OcrSidecarReply, OcrSidecarError> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    static OCR_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = format!(
        "ocr-{}",
        OCR_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );

    // the request line: the protocol's shape, the kernel's named DPI
    // constant, the additive-optional pages field only when the kernel is
    // marking pages.
    let mut request = serde_json::json!({ "id": id, "op": "ocr", "dpi": OCR_RASTER_DPI });
    if let Some(marked) = pages {
        request["pages"] = serde_json::to_value(marked).expect("usize array serializes");
    }
    let mut request_line =
        serde_json::to_vec(&request).map_err(|_| OcrSidecarError::SidecarFailed)?;
    request_line.push(b'\n');

    let mut child = Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| OcrSidecarError::SidecarFailed)?;
    // write the request line then the raw PDF bytes; drop stdin so the
    // child's read-to-EOF completes.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&request_line);
        let _ = stdin.write_all(pdf_bytes);
    }
    // bounded wait: the extract-document law — a hostile or hung document
    // cannot pin the child forever.
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(OCR_SIDECAR_TIMEOUT_SECS);
    let output = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break child
                    .stdout
                    .take()
                    .map(|mut o| {
                        let mut buf = Vec::new();
                        let _ = std::io::Read::read_to_end(&mut o, &mut buf);
                        (status, buf)
                    })
                    .unwrap_or((status, Vec::new()));
            }
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(OcrSidecarError::SidecarFailed);
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
            Err(_) => return Err(OcrSidecarError::SidecarFailed),
        }
    };
    let (status, stdout) = output;
    // ONE reply line (the framing); a reply-less child is a dead child.
    let line = stdout
        .split(|b| *b == b'\n')
        .find(|l| !l.is_empty())
        .ok_or(OcrSidecarError::SidecarFailed)?;
    let parsed: serde_json::Value =
        serde_json::from_slice(line).map_err(|_| OcrSidecarError::SidecarFailed)?;

    // the id echoes verbatim (the framing contract); a mismatching reply is
    // nonsense from the caller's seat.
    if parsed.get("id").and_then(|v| v.as_str()) != Some(id.as_str()) {
        return Err(OcrSidecarError::SidecarFailed);
    }

    // Top-level failures: the sidecar writes the named reply THEN exits
    // nonzero (bad_request 2 / raster_failed 3). A nonzero exit without a
    // parseable named reply is a dead child.
    if parsed.get("ok") == Some(&serde_json::Value::Bool(false)) {
        return match parsed.get("code").and_then(|v| v.as_str()) {
            Some("raster_failed") => Err(OcrSidecarError::RasterFailed),
            _ => Err(OcrSidecarError::SidecarFailed),
        };
    }
    if !status.success() {
        return Err(OcrSidecarError::SidecarFailed);
    }

    let engine = parsed
        .get("engine")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| OcrSidecarError::BadReply("the reply carries no engine stamp".into()))?
        .to_string();
    let entries = parsed
        .get("pages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| OcrSidecarError::BadReply("the reply carries no pages".into()))?;
    if entries.is_empty() {
        return Err(OcrSidecarError::BadReply(
            "the reply carries no pages".into(),
        ));
    }

    let mut out: Vec<OcrSidecarPage> = Vec::with_capacity(entries.len());
    let mut failed: Vec<usize> = Vec::new();
    for entry in entries {
        let index = entry
            .get("index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| OcrSidecarError::BadReply("a page entry carries no index".into()))?
            as usize;
        if doc_pages > 0 && (index == 0 || index > doc_pages) {
            return Err(OcrSidecarError::BadReply(format!(
                "page index {index} is outside the document ({doc_pages} pages)"
            )));
        }
        if entry.get("ok") == Some(&serde_json::Value::Bool(false)) {
            // the per-page named refusal (OV-5: never a fabricated empty)
            if entry.get("code").and_then(|v| v.as_str()) != Some("ocr_page_failed") {
                return Err(OcrSidecarError::BadReply(
                    "a failed page carries a code the protocol does not define".into(),
                ));
            }
            failed.push(index);
            continue;
        }
        let text = entry
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OcrSidecarError::BadReply("a page entry carries no text".into()))?
            .to_string();
        let confidence = entry.get("confidence").and_then(|v| v.as_f64());
        if text.is_empty() && confidence.is_some() {
            return Err(OcrSidecarError::BadReply(
                "an empty page carries a confidence claim — the smoothing the protocol forbids"
                    .into(),
            ));
        }
        if !text.is_empty()
            && confidence
                .map(|c| !(0.0..=1.0).contains(&c))
                .unwrap_or(true)
        {
            return Err(OcrSidecarError::BadReply(
                "a non-empty page needs a 0..1 confidence".into(),
            ));
        }
        out.push(OcrSidecarPage {
            index,
            text,
            confidence,
        });
    }
    if !failed.is_empty() {
        return Err(OcrSidecarError::PageFailed(failed));
    }
    // the reply is ascending and, for the all-pages request, complete.
    if out.windows(2).any(|w| w[0].index >= w[1].index) {
        return Err(OcrSidecarError::BadReply(
            "the reply's pages are not ascending".into(),
        ));
    }
    if doc_pages == 0 && out.len() != out.last().map(|p| p.index).unwrap_or(0) {
        return Err(OcrSidecarError::BadReply(
            "the all-pages reply is not complete from 1".into(),
        ));
    }
    Ok(OcrSidecarReply { engine, pages: out })
}

// ── the Tier-2 converter (IMPROVE06, the tool protocol's kernel side) ────────

/// How the converter run failed, from the kernel's seat (the route maps each
/// arm to its named refusal; the protocol's exit codes are the vocabulary).
#[derive(Debug)]
pub enum ConverterError {
    /// Exit 3 (or a crash / a non-protocol exit): the conversion failed.
    /// The payload is the tool's bounded stderr tail — the refusal MESSAGE
    /// carries it, never a row.
    Failed(String),
    /// The kernel killed the tool at `CONVERTER_TIMEOUT_SECS`.
    Timeout,
    /// Exit 5: the tool refused a remote/generative configuration posture.
    RemoteRefused,
    /// The tool "succeeded" but violated the contract: a file missing,
    /// meta.json unparseable, or the marker count disagreeing with the
    /// declared page count (DC-5's by-name failure). A half-answer is a
    /// failed answer — the document is refused, never shipped with a hole.
    ContractViolation(String),
}

impl std::fmt::Display for ConverterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(tail) => write!(f, "the converter failed: {tail}"),
            Self::Timeout => write!(f, "the converter exceeded its wall-clock bound"),
            Self::RemoteRefused => write!(f, "the converter refused a remote-services posture"),
            Self::ContractViolation(what) => write!(f, "converter contract violation: {what}"),
        }
    }
}

/// What a successful converter run produced: the tool's own identity from
/// meta.json and the per-page blocks split VERBATIM from `doc.md` (DC-2:
/// the kernel never rewrites converter bytes — the only transformation is
/// the split on the protocol's marker).
#[derive(Debug, Clone)]
pub struct ConverterOutput {
    /// meta.json's `tool` — the implementation name (`docling`, `docling-rs`);
    /// the row's `extractor` column and the header's extractor name.
    pub tool: String,
    /// meta.json's `version` — the operator-installed converter version,
    /// recorded per run (the improve05 engine-pin law: a re-derive under a
    /// moved version refuses).
    pub version: String,
    /// The per-page blocks, in order, split on `CONVERTER_PAGE_MARKER`.
    pub pages: Vec<String>,
}

/// The bounded stderr tail a `Failed` refusal carries in its MESSAGE: the
/// last 512 bytes, whitespace-collapsed to one line. Never a row — a failed
/// conversion lands nothing.
fn bounded_stderr_tail(stderr: &[u8]) -> String {
    let raw = String::from_utf8_lossy(stderr);
    let tail: String = raw
        .chars()
        .rev()
        .take(512)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    tail.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Run the operator-configured converter tool against `pdf_bytes` per
/// docs/DOCLING_TOOL_PROTOCOL.md: write the bytes to a process-private input
/// file, spawn `<tool> <in.pdf> <out-dir>` ONE-SHOT (the extract-document
/// lifecycle — a hung tool poisons no later document), kill at
/// `CONVERTER_TIMEOUT_SECS`, and read the two output files back. The
/// kernel's temp handling is std-only (no new dependency — the deny law):
/// a unique directory under the OS temp root, removed on every exit path.
///
/// No temp dir is left behind on any path: the caller's guard removes the
/// directory even when the tool crashed it full of output.
pub fn run_converter(
    tool: &std::path::Path,
    pdf_bytes: &[u8],
) -> Result<ConverterOutput, ConverterError> {
    let unique = format!(
        "brain-converter-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let work = std::env::temp_dir().join(unique);
    if std::fs::create_dir_all(&work).is_err() {
        return Err(ConverterError::Failed(
            "the kernel could not create the converter's working directory".into(),
        ));
    }
    let result = run_converter_in(tool, pdf_bytes, &work);
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// The testable core of [`run_converter`]: the same lifecycle against a
/// caller-provided directory (the pins use it to keep the stub's outputs
/// inspectable; the pure `run_converter` is this plus the private temp dir).
pub fn run_converter_in(
    tool: &std::path::Path,
    pdf_bytes: &[u8],
    work: &std::path::Path,
) -> Result<ConverterOutput, ConverterError> {
    use std::io::Read as _;
    use std::process::{Command, Stdio};

    let in_pdf = work.join("input.pdf");
    let out_dir = work.join("out");
    std::fs::write(&in_pdf, pdf_bytes)
        .map_err(|_| ConverterError::Failed("the kernel could not stage the input bytes".into()))?;
    std::fs::create_dir_all(&out_dir).map_err(|_| {
        ConverterError::Failed("the kernel could not stage the output directory".into())
    })?;

    // ONE-SHOT: spawn, bound, reap. stderr is captured (the bounded tail);
    // stdout is silenced (the tools print progress there; the protocol's
    // answers ride the FILES, never the stream).
    let mut child = Command::new(tool)
        .arg(&in_pdf)
        .arg(&out_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ConverterError::Failed(format!("the tool could not be spawned: {e}")))?;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(CONVERTER_TIMEOUT_SECS);
    let mut timed_out = false;
    let mut exited: Option<std::process::ExitStatus> = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exited = Some(status);
                break;
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return Err(ConverterError::Failed(format!(
                    "the tool's exit status could not be read: {e}"
                )));
            }
        }
    }
    if timed_out {
        return Err(ConverterError::Timeout);
    }
    let status = exited.expect("the loop exits only with a status or the timeout");

    // The protocol's exit vocabulary. A kill-by-signal or any other
    // non-protocol exit is the Failed posture (the tool's internals are its
    // own); the stderr tail rides the error.
    match status.code() {
        // The remote/generative refusal is the tool's own NAMED posture —
        // its stderr said so; the kernel's answer names the posture.
        Some(5) => return Err(ConverterError::RemoteRefused),
        // A self-timed tool that refuses to emit a partial document (exit 4,
        // the protocol's reserved code) is the honest-timeout posture: the
        // kernel answers converter_timeout for it too. The kernel's own kill
        // at CONVERTER_TIMEOUT_SECS lands in the same arm via `timed_out`.
        Some(4) => return Err(ConverterError::Timeout),
        Some(0) => {}
        _ => {
            let tail = child
                .stderr
                .take()
                .map(|mut s| {
                    let mut buf = Vec::new();
                    let _ = s.read_to_end(&mut buf);
                    bounded_stderr_tail(&buf)
                })
                .unwrap_or_else(|| "<no stderr>".into());
            return Err(ConverterError::Failed(tail));
        }
    }

    // The two output files. A missing one is a contract violation BY NAME.
    let doc_md = std::fs::read_to_string(out_dir.join("doc.md"))
        .map_err(|_| ConverterError::ContractViolation("the tool wrote no doc.md".to_string()))?;
    let meta_raw = std::fs::read_to_string(out_dir.join("meta.json")).map_err(|_| {
        ConverterError::ContractViolation("the tool wrote no meta.json".to_string())
    })?;
    #[derive(serde::Deserialize)]
    struct ConverterMeta {
        tool: String,
        version: String,
        pages: usize,
    }
    let meta: ConverterMeta = serde_json::from_str(&meta_raw).map_err(|e| {
        ConverterError::ContractViolation(format!("meta.json is not the contract's object: {e}"))
    })?;
    if meta.tool.is_empty() || meta.version.is_empty() {
        return Err(ConverterError::ContractViolation(
            "meta.json carries an empty tool or version".to_string(),
        ));
    }

    // DC-5: the markers, the declared pages, and the split blocks must all
    // agree — pages == markers+1 == blocks. A mismatch fails BY NAME; the
    // document is never ingested with a hole or a phantom page.
    let blocks = split_converter_markdown(&doc_md);
    let markers = doc_md.matches(CONVERTER_PAGE_MARKER).count();
    if blocks.len() != meta.pages {
        return Err(ConverterError::ContractViolation(format!(
            "the tool's marker split yields {} blocks for {} declared pages",
            blocks.len(),
            meta.pages
        )));
    }
    if markers != meta.pages - 1 {
        return Err(ConverterError::ContractViolation(format!(
            "the tool wrote {markers} page-break markers for {} declared pages",
            meta.pages
        )));
    }
    if blocks.iter().any(|p| p.contains(CONVERTER_PAGE_MARKER)) {
        return Err(ConverterError::ContractViolation(
            "a page block still carries a page-break marker after the split".to_string(),
        ));
    }

    Ok(ConverterOutput {
        tool: meta.tool,
        version: meta.version,
        pages: blocks,
    })
}

/// Split the converter's markdown on the protocol's marker, VERBATIM (DC-2:
/// the only transformation the kernel applies to converter bytes). The
/// marker line itself never survives into a block — the block boundaries
/// are the pages the tool meant.
pub fn split_converter_markdown(doc_md: &str) -> Vec<String> {
    doc_md
        .split(CONVERTER_PAGE_MARKER)
        .map(|block| block.trim().to_string())
        .collect()
}

// ── the document row ─────────────────────────────────────────────────────────

/// One row of the `documents` table — the provenance anchor for every chunk
/// the document produced. DI-3's authority: the header must AGREE with this
/// row, never the reverse.
#[derive(Debug, Clone)]
pub struct DocumentRow {
    pub id: i64,
    pub sha256: String,
    pub title: String,
    pub tier: String,
    pub extractor: String,
    pub extractor_version: String,
    pub pages: i64,
    pub layout_unverified: bool,
    pub source: String,
    pub domain: String,
    pub owner: Option<String>,
    pub extracted_utc: String,
    pub markdown_sha256: String,
    /// IMPROVE05: the per-page tier map as a JSON array (`["text-layer",
    /// "ocr", …]`), present exactly when OCR ran. DI-3's row check extends
    /// to it — a tampered map fails BY NAME.
    pub page_tiers: Option<String>,
    /// IMPROVE05: the count of pages flagged below the named floor.
    pub low_confidence_pages: i64,
    /// IMPROVE05: the engine stamp that produced the OCR pages (recorded
    /// per run; a re-serve under a moved engine refuses named).
    pub ocr_engine: Option<String>,
}

/// Verify an annotated markdown's header against the document row (DI-3).
/// Every field must agree; a tampered header — any one field — fails, and
/// the check never "fixes" the markdown.
pub fn verify_document_header(markdown: &str, row: &DocumentRow) -> Result<(), String> {
    let h = parse_document_header(markdown)?;
    let expect = |cond: bool, what: String| if cond { Ok(()) } else { Err(what) };
    expect(
        h.doc == row.sha256,
        format!("doc: header {} != row {}", h.doc, row.sha256),
    )?;
    expect(
        h.source == row.source,
        format!("source: header {:?} != row {:?}", h.source, row.source),
    )?;
    expect(
        h.pages == row.pages as usize,
        format!("pages: header {} != row {}", h.pages, row.pages),
    )?;
    expect(
        h.tier == row.tier,
        format!("tier: header {:?} != row {:?}", h.tier, row.tier),
    )?;
    expect(
        h.extracted == row.extracted_utc,
        format!(
            "extracted: header {:?} != row {:?}",
            h.extracted, row.extracted_utc
        ),
    )?;
    let want_extractor = format!("{}/{}", row.extractor, row.extractor_version);
    expect(
        h.extractor == want_extractor,
        format!(
            "extractor: header {:?} != row {:?}",
            h.extractor, want_extractor
        ),
    )?;
    let want_layout = if row.layout_unverified {
        LAYOUT_UNVERIFIED.to_string()
    } else {
        "verified".to_string()
    };
    expect(
        h.layout == want_layout,
        format!("layout: header {:?} != row {:?}", h.layout, want_layout),
    )?;
    // IMPROVE05: the OCR extension of DI-3. The engine and the map appear
    // in the header EXACTLY when the row carries them — a text-layer row's
    // header carries neither (the None==None arm), and a tampered map or a
    // moved engine fails BY NAME.
    expect(
        h.ocr_engine == row.ocr_engine,
        format!(
            "ocr-engine: header {:?} != row {:?}",
            h.ocr_engine, row.ocr_engine
        ),
    )?;
    let row_map = row
        .page_tiers
        .as_deref()
        .map(serde_json::from_str::<Vec<String>>)
        .transpose()
        .map_err(|e| format!("page_tiers: the row's map is not a JSON array: {e}"))?;
    expect(
        h.pages_tier == row_map,
        format!("pages-tier: header {:?} != row {:?}", h.pages_tier, row_map),
    )?;
    // IMPROVE16: the corrected variant's stamp agreement (the DI-3
    // extension). A row with the doc-corrector extractor IS a corrected
    // row: its tier carries the corrected suffix, and the header MUST
    // claim the exact same stamp the row's version half carries. Every
    // tamper shape fails BY NAME: a stripped `corrector:` field (the row
    // says corrected, the header claims nothing), a swapped stamp, a
    // forged `corrector:` on a non-corrected row, or a corrected row
    // whose tier lost the suffix.
    let row_is_corrected = row.extractor == DOC_CORRECTOR_EXTRACTOR;
    let want_corrector = if row_is_corrected {
        Some(row.extractor_version.clone())
    } else {
        None
    };
    expect(
        h.corrector == want_corrector,
        format!(
            "corrector: header {:?} != row {:?}",
            h.corrector, want_corrector
        ),
    )?;
    expect(
        row_is_corrected == row.tier.ends_with(TIER_CORRECTED_SUFFIX),
        format!(
            "tier: row {:?} disagrees with its corrected posture (extractor {:?})",
            row.tier, row.extractor
        ),
    )
}

/// Insert the document row (idempotent on sha256 — the UNIQUE index makes a
/// re-ingest a lookup, handled by the route before it gets here). Returns the
/// row id.
pub fn insert_document_row(
    tx: &rusqlite::Transaction<'_>,
    row: &DocumentRow,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO documents
           (sha256, title, tier, extractor, extractor_version, pages,
            layout_unverified, source, domain, owner, extracted_utc, markdown_sha256,
            page_tiers, low_confidence_pages, ocr_engine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            row.sha256,
            row.title,
            row.tier,
            row.extractor,
            row.extractor_version,
            row.pages,
            row.layout_unverified as i64,
            row.source,
            row.domain,
            row.owner,
            row.extracted_utc,
            row.markdown_sha256,
            row.page_tiers,
            row.low_confidence_pages,
            row.ocr_engine,
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Load a document row by content sha256 — the duplicate-path lookup.
pub fn find_document_by_sha(
    conn: &rusqlite::Connection,
    sha256: &str,
) -> rusqlite::Result<Option<DocumentRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, sha256, title, tier, extractor, extractor_version, pages,
                layout_unverified, source, domain, owner, extracted_utc, markdown_sha256,
                page_tiers, low_confidence_pages, ocr_engine
         FROM documents WHERE sha256 = ?1",
    )?;
    let mut rows = stmt.query_map([sha256], |r| {
        Ok(DocumentRow {
            id: r.get(0)?,
            sha256: r.get(1)?,
            title: r.get(2)?,
            tier: r.get(3)?,
            extractor: r.get(4)?,
            extractor_version: r.get(5)?,
            pages: r.get(6)?,
            layout_unverified: r.get::<_, i64>(7)? != 0,
            source: r.get(8)?,
            domain: r.get(9)?,
            owner: r.get(10)?,
            extracted_utc: r.get(11)?,
            markdown_sha256: r.get(12)?,
            page_tiers: r.get(13)?,
            low_confidence_pages: r.get(14)?,
            ocr_engine: r.get(15)?,
        })
    })?;
    rows.next().transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE_COL: &[u8] = include_bytes!("../tests/fixtures/documents/single-column.pdf");
    const SCANNED: &[u8] = include_bytes!("../tests/fixtures/documents/scanned.pdf");

    // ── IMPROVE16: the corrected variant's vocabulary ──

    const VERBATIM_MD: &str = "<!-- doc: pdf-sha | source: document | pages: 1 | tier: docling\n     extracted: 2026-11-12T00:00:00+00:00 | extractor: docling-rs/0.9 | layout: unverified -->\n## Page 1\nThe cat satt on the mat.\n<!-- anchor: p1 -->\n";

    fn verbatim_row() -> DocumentRow {
        DocumentRow {
            id: 1,
            sha256: "pdf-sha".to_string(),
            title: "t".to_string(),
            tier: "docling".to_string(),
            extractor: "docling-rs".to_string(),
            extractor_version: "0.9".to_string(),
            pages: 1,
            layout_unverified: true,
            source: "document".to_string(),
            domain: "global".to_string(),
            owner: None,
            extracted_utc: "2026-11-12T00:00:00+00:00".to_string(),
            markdown_sha256: "m-sha".to_string(),
            page_tiers: None,
            low_confidence_pages: 0,
            ocr_engine: None,
        }
    }

    fn corrected_row(stamp: &str) -> DocumentRow {
        let mut row = verbatim_row();
        row.sha256 = "sha-of-verbatim-markdown".to_string();
        row.tier = corrected_tier("docling");
        row.extractor = DOC_CORRECTOR_EXTRACTOR.to_string();
        row.extractor_version = stamp.to_string();
        // the correction is its OWN event: the corrected row's clock is
        // the correction moment (the header carries the same value).
        row.extracted_utc = "2026-11-12T01:00:00+00:00".to_string();
        row
    }

    fn corrected_markdown(stamp: &str) -> String {
        let verbatim_header = parse_document_header(VERBATIM_MD).expect("verbatim header");
        let header = emit_corrected_header(
            &verbatim_header,
            "sha-of-verbatim-markdown",
            "2026-11-12T01:00:00+00:00",
            stamp,
        );
        let corrected_body = "## Page 1\nThe cat sat on the mat.\n<!-- anchor: p1 -->\n";
        assemble_corrected_markdown(corrected_body, header).expect("the body has structure")
    }

    /// The assembled corrected markdown parses, carries the corrected
    /// vocabulary end-to-end, and re-passes the SAME contract the verbatim
    /// pipeline's output passes (the cage, on the happy path).
    #[test]
    fn the_corrected_variant_round_trips_through_the_cage() {
        let md = corrected_markdown("qwen@qwen");
        verify_anchor_contract(&md, 1).expect("the corrected markdown passes the cage");
        let h = parse_document_header(&md).expect("the corrected header parses");
        assert_eq!(h.tier, "docling-corrected");
        assert_eq!(h.corrector.as_deref(), Some("qwen@qwen"));
        assert_eq!(h.extractor, format!("{DOC_CORRECTOR_EXTRACTOR}/qwen@qwen"));
        assert_eq!(h.doc, "sha-of-verbatim-markdown");
        assert_eq!(h.pages, 1);
        // DI-3: the header agrees with the corrected row exactly.
        verify_document_header(&md, &corrected_row("qwen@qwen"))
            .expect("the corrected header and row agree");
    }

    /// The DI-3 extension's tamper shapes, each BY NAME: a stripped stamp,
    /// a swapped stamp, a forged stamp on a non-corrected row, and a
    /// corrected row whose tier lost the suffix.
    #[test]
    fn the_corrector_stamp_agreement_fails_every_tamper_by_name() {
        let md = corrected_markdown("qwen@qwen");
        // a stripped `corrector:` field
        let stripped = md.replace(" | corrector: qwen@qwen", "");
        let err = verify_document_header(&stripped, &corrected_row("qwen@qwen"))
            .expect_err("a stripped stamp reds");
        assert!(err.contains("corrector:"), "named: {err}");
        // a swapped stamp — the stamp rides BOTH header fields (extractor:
        // and corrector:), so the swap breaks the extractor agreement
        // first; either way the disagreement is NAMED.
        let swapped = corrected_markdown("other@other");
        let err = verify_document_header(&swapped, &corrected_row("qwen@qwen"))
            .expect_err("a swapped stamp reds");
        assert!(
            err.contains("corrector:") || err.contains("extractor:"),
            "named: {err}"
        );
        // a forged stamp on a NON-corrected row: the plain markdown with a
        // `corrector:` field injected into its header (the doc/sha fields
        // still agree with the plain row — the corrector check is what
        // fires).
        let forged = VERBATIM_MD.replace(
            "| layout: unverified",
            "| layout: unverified | corrector: forged@v1",
        );
        let err = verify_document_header(&forged, &verbatim_row())
            .expect_err("a forged stamp on a plain row reds");
        assert!(err.contains("corrector:"), "named: {err}");
        // a row whose corrected posture disagrees INTERNALLY: extractor
        // doc-corrector but the tier lost the suffix — with every header
        // agreement holding (both tiers demoted to plain so the tier check
        // passes), the row-internal posture check is what fires.
        let mut bad = corrected_row("qwen@qwen");
        bad.tier = "docling".to_string();
        let demoted = md.replace("tier: docling-corrected", "tier: docling");
        let err = verify_document_header(&demoted, &bad).expect_err("a mixed-posture row reds");
        assert!(
            err.contains("disagrees with its corrected posture"),
            "named: {err}"
        );
        // the plain pipeline's own markdown still passes its plain row
        // (the extension never disturbed the pre-lane agreement).
        verify_document_header(VERBATIM_MD, &verbatim_row())
            .expect("the plain header and row still agree");
    }

    /// The cage refuses a structure-breaking correction BY NAME: a dropped
    /// anchor and a missing page structure both red.
    #[test]
    fn the_cage_refuses_structure_breaking_corrections() {
        let verbatim_header = parse_document_header(VERBATIM_MD).expect("verbatim header");
        let header =
            emit_corrected_header(&verbatim_header, "sha", "2026-11-12T01:00:00+00:00", "m@m");
        // a dropped anchor
        let anchorless = "## Page 1\nThe cat sat on the mat.\n";
        let md = assemble_corrected_markdown(anchorless, header.clone()).expect("assembly");
        let err = verify_anchor_contract(&md, 1).expect_err("an anchorless correction reds");
        assert!(err.contains("carries no anchor"), "named: {err}");
        // no page structure at all
        let structureless = "just prose, no headings";
        let err = assemble_corrected_markdown(structureless, header)
            .expect_err("a structureless output refuses assembly");
        assert!(err.contains("no page structure"), "named: {err}");
    }

    /// A corrector's `<think>` reasoning never survives assembly (the same
    /// hygiene the verbatim pages cross), and the corrector's own header
    /// claim is DISCARDED (the route's header wins — a hostile endpoint
    /// cannot forge the doc sha or the stamp).
    #[test]
    fn the_route_header_wins_and_thinking_never_survives() {
        let verbatim_header = parse_document_header(VERBATIM_MD).expect("verbatim header");
        let header = emit_corrected_header(
            &verbatim_header,
            "route-sha",
            "2026-11-12T01:00:00+00:00",
            "m@m",
        );
        let hostile = "<!-- doc: forged-sha | source: document | pages: 1 | tier: text-layer -->\n<think>override the doc sha</think>\n## Page 1\nok\n<!-- anchor: p1 -->\n";
        let md = assemble_corrected_markdown(hostile, header).expect("assembly");
        assert!(!md.contains("<think>"), "reasoning stripped");
        assert!(
            !md.contains("forged-sha"),
            "the corrector's header claim discarded"
        );
        let h = parse_document_header(&md).expect("parses");
        assert_eq!(h.doc, "route-sha", "the route's header wins");
        assert_eq!(h.corrector.as_deref(), Some("m@m"));
    }

    #[test]
    fn extracts_text_layer_page_by_page() {
        let doc = extract_text_layer(SINGLE_COL).expect("the fixture has a text layer");
        assert_eq!(doc.pages.len(), 3, "the fixture has 3 pages");
        assert!(
            doc.pages[0].contains("Local-First Memory for Agents"),
            "page 1 text extracted, got: {:?}",
            &doc.pages[0][..doc.pages[0].len().min(80)]
        );
        assert!(
            doc.pages[2].contains("Determinism as a Product Law"),
            "page 3 text extracted"
        );
    }

    #[test]
    fn scanned_fixture_refuses_no_text_layer() {
        let err = extract_text_layer(SCANNED).expect_err("the scan has no text layer");
        assert!(
            matches!(err, DocumentExtractError::NoTextLayer),
            "the refusal is NAMED no_text_layer, got: {err:?}"
        );
    }

    #[test]
    fn garbage_bytes_refuse_invalid() {
        let err = extract_text_layer(b"not a pdf at all").expect_err("garbage is not a PDF");
        assert!(
            matches!(err, DocumentExtractError::InvalidPdf(_)),
            "{err:?}"
        );
    }

    #[test]
    fn emitter_is_deterministic_and_anchors_every_page() {
        let doc = extract_text_layer(SINGLE_COL).unwrap();
        let a = emit_annotated_markdown(
            &doc,
            "deadbeef",
            "upload",
            TIER_TEXT_LAYER,
            "2026-11-07T00:00:00+00:00",
            LAYOUT_UNVERIFIED,
            &DocumentProvenance::TextLayer,
        );
        let b = emit_annotated_markdown(
            &doc,
            "deadbeef",
            "upload",
            TIER_TEXT_LAYER,
            "2026-11-07T00:00:00+00:00",
            LAYOUT_UNVERIFIED,
            &DocumentProvenance::TextLayer,
        );
        assert_eq!(a, b, "same arguments → byte-identical markdown");
        verify_anchor_contract(&a, 3).expect("the emitted markdown carries its own contract");
        for n in 1..=3 {
            assert!(a.contains(&format!("<!-- anchor: p{n} -->")), "anchor p{n}");
        }
        assert!(a.contains("layout: unverified"), "Tier 0 layout posture");
        assert!(
            a.contains("extractor: pdf-extract/0.12.1"),
            "extractor stamp"
        );
    }

    #[test]
    fn anchor_contract_rejects_structural_violations() {
        // the control arm: each violation is rejected BY NAME.
        let missing_anchor = "## Page 1\ntext\n";
        assert_eq!(
            verify_anchor_contract(missing_anchor, 1),
            Err("page 1 block carries no anchor".to_string())
        );
        let non_monotonic =
            "## Page 1\nt\n<!-- anchor: p1 -->\n## Page 3\nt\n<!-- anchor: p3 -->\n";
        assert!(
            verify_anchor_contract(non_monotonic, 2)
                .expect_err("non-monotonic")
                .contains("not monotonic")
        );
        let count_mismatch = "## Page 1\nt\n<!-- anchor: p1 -->\n";
        assert_eq!(
            verify_anchor_contract(count_mismatch, 2),
            Err("page count mismatch: 1 blocks for 2 declared pages".to_string())
        );
    }

    #[test]
    fn header_roundtrip_and_row_verification() {
        let doc = extract_text_layer(SINGLE_COL).unwrap();
        let utc = "2026-11-07T12:00:00+00:00";
        let md = emit_annotated_markdown(
            &doc,
            "cafe123",
            "upload",
            TIER_TEXT_LAYER,
            utc,
            LAYOUT_UNVERIFIED,
            &DocumentProvenance::TextLayer,
        );
        let h = parse_document_header(&md).expect("the emitted header parses");
        assert_eq!(h.doc, "cafe123");
        assert_eq!(h.pages, 3);
        assert_eq!(h.tier, TIER_TEXT_LAYER);
        assert_eq!(h.extracted, utc);
        assert_eq!(h.layout, LAYOUT_UNVERIFIED);

        let row = DocumentRow {
            id: 1,
            sha256: "cafe123".into(),
            title: "t".into(),
            tier: TIER_TEXT_LAYER.into(),
            extractor: EXTRACTOR_NAME.into(),
            extractor_version: EXTRACTOR_VERSION.into(),
            pages: 3,
            layout_unverified: true,
            source: "upload".into(),
            domain: "global".into(),
            owner: None,
            extracted_utc: utc.into(),
            markdown_sha256: String::new(),
            page_tiers: None,
            low_confidence_pages: 0,
            ocr_engine: None,
        };
        verify_document_header(&md, &row).expect("an honest header verifies");

        // DI-3's control: EVERY field, tampered one at a time, fails.
        for (field, from, to) in [
            ("doc", "cafe123", "dead999"),
            ("source", "upload", "memory"),
            ("tier", TIER_TEXT_LAYER, "ocr"),
            ("extracted", utc, "2027-01-01T00:00:00+00:00"),
            ("layout", LAYOUT_UNVERIFIED, "verified"),
        ] {
            let tampered = md.replacen(from, to, 1);
            let err = verify_document_header(&tampered, &row)
                .expect_err(&format!("tampered {field} must fail the row check"));
            assert!(err.starts_with(field), "the failure names the field: {err}");
        }
        // pages tampering: the number is a standalone token
        let tampered = md.replacen("| pages: 3 |", "| pages: 9 |", 1);
        assert!(
            verify_document_header(&tampered, &row)
                .expect_err("tampered pages")
                .starts_with("pages")
        );
        // extractor tampering: any version bump fails
        let tampered = md.replacen("pdf-extract/0.12.1", "pdf-extract/9.9.9", 1);
        assert!(
            verify_document_header(&tampered, &row)
                .expect_err("tampered extractor")
                .starts_with("extractor")
        );
    }

    #[test]
    fn converter_probe_names_all_three_postures() {
        // the S4 seam is tested by absence (CI runs with no converter
        // configured) — and the other two arms are pinned pure, no env.
        assert_eq!(
            probe_document_converter(None),
            ConverterProbe::Unconfigured,
            "unset = Unconfigured, the CI posture"
        );
        let missing = std::path::Path::new("/nonexistent/converter/path");
        assert_eq!(
            probe_document_converter(Some(missing)),
            ConverterProbe::PathMissing,
            "configured-but-absent = PathMissing"
        );
        let present = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        assert_eq!(
            probe_document_converter(Some(&present)),
            ConverterProbe::Present(present),
            "configured-and-present = Present (the route still refuses, named)"
        );
    }

    #[test]
    fn extractor_version_const_matches_the_lock() {
        // The drift-guard: EXTRACTOR_VERSION must equal the resolved lock
        // version of pdf-extract. A lock bump without the const is a found
        // bug (the header would name an extractor that did not run).
        let lock = include_str!("../Cargo.lock");
        let mut in_block = false;
        for line in lock.lines() {
            if line.trim() == "[[package]]" {
                in_block = false;
            }
            if line.trim() == "name = \"pdf-extract\"" {
                in_block = true;
            } else if in_block && line.trim().starts_with("version = ") {
                let v = line
                    .trim()
                    .trim_start_matches("version = \"")
                    .trim_end_matches('"');
                assert_eq!(
                    EXTRACTOR_VERSION, v,
                    "EXTRACTOR_VERSION drifted from Cargo.lock"
                );
                return;
            }
        }
        panic!("pdf-extract not found in Cargo.lock");
    }
}
