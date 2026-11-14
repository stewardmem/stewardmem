// shell-vision — the OCR sidecar (IMPROVE05): on-device text recognition
// through Apple's Vision framework, behind the extract-document lifecycle.
//
// PROTOCOL (docs/OCR_SIDECAR_PROTOCOL.md, verbatim): ONE request per
// document — a single JSON line on stdin ({"id","op":"ocr","dpi":300,
// "pages":[…]?}) immediately followed by the raw PDF bytes to EOF — and ONE
// typed reply line on stdout, then exit. The JSONL FRAMING mirrors
// brain-afm's (one writer for stdout, sorted keys, named error codes, never
// prose a caller parses); the LIFETIME is extract-document's (one-shot per
// document — OCR is stateless, so a hung or crashed child poisons no later
// document, and the kernel's bounded wait applies cleanly).
//
// THE LAWS THIS PROCESS EXISTS TO KEEP:
//
// - On-device, no data leaves. Vision text recognition is on-device; this
//   process touches no network (no URLSession, no sockets), ships no bytes
//   anywhere, and requests no entitlements that could.
// - No LLM anywhere, no post-OCR correction (OV-5). The recognized text
//   enters the reply VERBATIM — no smoothing, no rewriting, no
//   "improvement" pass exists in this file (arXiv:2611.05539: LLM post-OCR
//   correction rewrites text that was already correct; the surface does
//   not exist here by construction). A page Vision refuses is a NAMED
//   failure — `ocr_page_failed`, or `ocr_assets_unavailable` when the
//   refusal IS the machine missing the on-demand recognition model — never
//   a fabricated empty, never a retry.
// - Sequential recognition (OV-6): ONE Vision request at a time, pages in
//   ascending order. Parallel RecognizeTextRequest tasks are the measured
//   hang hazard (Apple Developer Forums; the plan's §0.2.2) — the loop
//   below awaits each page before starting the next, and no task-group or
//   concurrent construct exists in this source (pinned structurally from
//   the kernel's suite).
// - Determinism, honestly bounded: same bytes + same OS build + same
//   engine + same DPI → same output, recorded per run (the engine string
//   carries the request type + kern.osversion). Nothing here promises
//   bitwise-identical output across OS versions — Apple does not, so the
//   protocol records what ran instead of promising what will run.
// - Languages come from the runtime (never hardcoded):
//   automaticallyDetectsLanguage stays on and the framework's own
//   supportedRecognitionLanguages() is the set — a hardcoded list in this
//   file is a found bug. (Measured 2026-11-08: 33 languages, including zh;
//   auto-detect nonetheless returns ZERO observations on a Chinese page —
//   the non-latin fixture class measures that gap honestly.)
//
// macOS 15.0+ is the floor: the Swift-native `RecognizeTextRequest` (not
// the legacy VN* classes) is exactly macOS 15+ and the package's
// deployment target is exactly macOS 15 — no availability fork on the base
// path. `RecognizeDocumentsRequest` (paragraph/table grouping) is macOS
// 26+ and is deliberately NOT used here (a plan-06 note, never a
// requirement); the engine string names the request type that ran.

import CoreGraphics
import Foundation
import Vision

// ── the named constants ──────────────────────────────────────────────────────

/// The DPI band the protocol accepts; the kernel sends its own named
/// constant (config::OCR_RASTER_DPI = 300), and the DPI is part of the
/// determinism tuple.
let DPI_BAND = 72...600

// ── the named error codes (the protocol's vocabulary) ────────────────────────

enum SidecarError: Error {
    /// The request line is not valid JSON, op is not "ocr", dpi is out of
    /// band, or a page index is out of range. Exit 2.
    case badRequest(String)
    /// The bytes are not a PDF CoreGraphics can open, or a requested page
    /// could not be rendered. Exit 3.
    case rasterFailed(String)
}

// ── the request ──────────────────────────────────────────────────────────────

struct OcrRequest {
    let id: Any?
    let dpi: Int
    /// The 1-based page indices to recognize, deduped and ascending —
    /// nil means ALL pages (the additive-optional `pages` field).
    let pages: [Int]?
}

/// Parse the request line. `id` is echoed verbatim (any JSON value); a
/// malformed request answers bad_request with `"id":null` when even the id
/// could not be read.
func parseRequest(_ line: Data) throws -> OcrRequest {
    guard line.count > 0,
        let obj = try? JSONSerialization.jsonObject(with: line),
        let dict = obj as? [String: Any]
    else {
        throw SidecarError.badRequest("the request line is not a JSON object")
    }
    guard let op = dict["op"] as? String, op == "ocr" else {
        throw SidecarError.badRequest("op must be exactly \"ocr\"")
    }
    let dpi: Int
    if let raw = dict["dpi"] {
        guard let n = raw as? Int, DPI_BAND.contains(n) else {
            throw SidecarError.badRequest("dpi must be an integer in \(DPI_BAND.lowerBound)...\(DPI_BAND.upperBound)")
        }
        dpi = n
    } else {
        dpi = 300
    }
    var pages: [Int]?
    if let raw = dict["pages"] {
        guard let arr = raw as? [Any] else {
            throw SidecarError.badRequest("pages must be an array of integers")
        }
        var seen = Set<Int>()
        var indices: [Int] = []
        for value in arr {
            guard let n = value as? Int else {
                throw SidecarError.badRequest("pages must be an array of integers")
            }
            guard n >= 1 else {
                throw SidecarError.badRequest("page indices are 1-based; got \(n)")
            }
            if seen.insert(n).inserted {
                indices.append(n)
            }
        }
        pages = indices.sorted()
    }
    return OcrRequest(id: dict["id"], dpi: dpi, pages: pages)
}

// ── the engine string ────────────────────────────────────────────────────────

/// The running OS build (kern.osversion, e.g. 25C101) — the engine string's
/// determinism anchor: apple-vision/recognize-text@<build>.
func osBuild() -> String {
    var size = 0
    sysctlbyname("kern.osversion", nil, &size, nil, 0)
    guard size > 0 else { return "unknown" }
    var buf = [CChar](repeating: 0, count: size)
    sysctlbyname("kern.osversion", &buf, &size, nil, 0)
    return String(cString: buf)
}

func engineString() -> String {
    "apple-vision/recognize-text@\(osBuild())"
}

// ── rasterization (CoreGraphics, at the requested DPI) ───────────────────────

/// Render page `index` (1-based) of the PDF bytes at `dpi`. White-backed
/// (a page with transparency draws over paper, never over black).
func rasterizePage(_ pdfData: Data, index: Int, dpi: Int) throws -> CGImage {
    guard let provider = CGDataProvider(data: pdfData as CFData),
        let doc = CGPDFDocument(provider)
    else {
        throw SidecarError.rasterFailed("the bytes are not a PDF CoreGraphics can open")
    }
    guard doc.numberOfPages > 0 else {
        throw SidecarError.rasterFailed("the PDF carries no pages")
    }
    guard index >= 1 && index <= doc.numberOfPages, let page = doc.page(at: index) else {
        throw SidecarError.badRequest("page \(index) does not exist (the PDF has \(doc.numberOfPages) pages)")
    }
    let box = page.getBoxRect(.mediaBox)
    let scale = CGFloat(dpi) / 72.0
    let width = max(1, Int(box.width * scale))
    let height = max(1, Int(box.height * scale))
    guard let ctx = CGContext(
        data: nil, width: width, height: height,
        bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
    ) else {
        throw SidecarError.rasterFailed("page \(index) could not be rasterized")
    }
    ctx.setFillColor(CGColor(gray: 1.0, alpha: 1.0))
    ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
    ctx.scaleBy(x: scale, y: scale)
    ctx.drawPDFPage(page)
    guard let image = ctx.makeImage() else {
        throw SidecarError.rasterFailed("page \(index) could not be rasterized")
    }
    return image
}

// ── recognition (SEQUENTIAL — OV-6) ─────────────────────────────────────────

/// One Vision request for one page image. The text is the observations'
/// top candidates joined line-by-line, VERBATIM; the confidence is the
/// page's MINIMUM top-candidate confidence (0..1, conservative honesty:
/// one weak region marks the page), nil when nothing was recognized.
/// Throws → the caller records ocr_page_failed for this index.
func recognizePage(_ image: CGImage) async throws -> (text: String, confidence: Double?) {
    // The Swift-native request (macOS 15+): .accurate for documents,
    // language correction on, languages auto-detected from the runtime's
    // own supported set — no hardcoded list (the module header's law).
    var request = RecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    request.automaticallyDetectsLanguage = true
    let observations = try await request.perform(on: image, orientation: .up)
    var lines: [String] = []
    var minConfidence: Double?
    for observation in observations {
        if let top = observation.topCandidates(1).first {
            lines.append(top.string)
            minConfidence = min(minConfidence ?? 2.0, Double(top.confidence))
        }
    }
    let text = lines.joined(separator: "\n")
    return (text, text.isEmpty ? nil : minConfidence)
}

/// The per-page failure code for a thrown Vision error. The ASSET class is
/// its own named verdict — `ocr_assets_unavailable`: the on-demand
/// recognition model data this page's language needs is not present on this
/// machine (fresh CI volumes; `dataUnavailable` / `unsupportedRevision` are
/// how the framework reports it) — an environment condition, not a
/// recognition failure. Everything else stays `ocr_page_failed`: a genuine
/// refusal this build must see, never smoothed into an empty. The
/// underlying message rides verbatim either way.
func visionErrorCode(_ error: any Error) -> String {
    switch error {
    case VisionError.dataUnavailable, VisionError.unsupportedRevision:
        "ocr_assets_unavailable"
    default:
        "ocr_page_failed"
    }
}

// ── the pipeline ─────────────────────────────────────────────────────────────

/// One document through the sidecar: parse → open/rasterize → recognize
/// (sequentially) → the reply dict + the process exit code. A page that
/// answered (success or ocr_page_failed) is a verdict; the reply is ok:true
/// when every requested page carries one. Raster failures refuse the whole
/// document (no page can be asked) — raster_failed, exit 3.
func process(requestLine: Data, pdfBytes: Data) async -> (reply: [String: Any], exitCode: Int32) {
    let request: OcrRequest
    do {
        request = try parseRequest(requestLine)
    } catch let error as SidecarError {
        return (errorReply(id: NSNull(), error: error), exitCode(for: error))
    } catch {
        return (errorReply(id: NSNull(), error: .badRequest("\(error)")), 2)
    }

    // Open the document FIRST (raster_failed / the page-count validation
    // happen before any recognition work).
    guard let provider = CGDataProvider(data: pdfBytes as CFData),
        let doc = CGPDFDocument(provider),
        doc.numberOfPages > 0
    else {
        let error = SidecarError.rasterFailed("the bytes are not a PDF CoreGraphics can open")
        return (errorReply(id: request.id ?? NSNull(), error: error), exitCode(for: error))
    }
    let pageCount = doc.numberOfPages
    if let marked = request.pages {
        let outOfRange = marked.filter { $0 > pageCount }
        if !outOfRange.isEmpty {
            let error = SidecarError.badRequest(
                "page(s) \(outOfRange.map(String.init).joined(separator: ", ")) do not exist (the PDF has \(pageCount) pages)"
            )
            return (errorReply(id: request.id ?? NSNull(), error: error), exitCode(for: error))
        }
    }
    let requested = request.pages ?? Array(1...pageCount)

    // THE SEQUENTIAL LOOP (OV-6): one page at a time, ascending order.
    // Each page either answers (text + confidence) or is a named per-page
    // failure — never a fabricated empty, never a retry, never a second
    // request for the same document (one-shot per document, the module
    // header's law).
    var pages: [[String: Any]] = []
    for index in requested {
        do {
            let image = try rasterizePage(pdfBytes, index: index, dpi: request.dpi)
            do {
                let (text, confidence) = try await recognizePage(image)
                var entry: [String: Any] = ["index": index, "text": text]
                // no recognized text → no confidence claim (a fabricated
                // number would be exactly the smoothing this protocol
                // forbids; the honest emptiness rule).
                if let confidence { entry["confidence"] = confidence }
                pages.append(entry)
            } catch {
                pages.append([
                    "index": index,
                    "ok": false,
                    "code": visionErrorCode(error),
                    "message": "\(error)",
                ])
            }
        } catch let error as SidecarError {
            // a page that cannot RENDER means no page can be asked — the
            // document-level raster refusal (the protocol's table).
            return (errorReply(id: request.id ?? NSNull(), error: error), exitCode(for: error))
        } catch {
            let raster = SidecarError.rasterFailed("page \(index) could not be rasterized")
            return (errorReply(id: request.id ?? NSNull(), error: raster), exitCode(for: raster))
        }
    }

    return (
        [
            "id": request.id ?? NSNull(),
            "ok": true,
            "engine": engineString(),
            "pages": pages,
        ],
        0
    )
}

func errorReply(id: Any, error: SidecarError) -> [String: Any] {
    [
        "id": id,
        "ok": false,
        "code": code(of: error),
        "message": message(of: error),
    ]
}

func code(of error: SidecarError) -> String {
    switch error {
    case .badRequest: "bad_request"
    case .rasterFailed: "raster_failed"
    }
}

func message(of error: SidecarError) -> String {
    switch error {
    case .badRequest(let m), .rasterFailed(let m): m
    }
}

func exitCode(for error: SidecarError) -> Int32 {
    switch error {
    case .badRequest: 2
    case .rasterFailed: 3
    }
}

// ── the one writer (the brain-afm LineOut shape) ─────────────────────────────

/// One writer for stdout: one JSON line, sorted keys, never interleaved.
final class ReplyWriter: @unchecked Sendable {
    private let handle = FileHandle.standardOutput

    func write(_ object: [String: Any]) {
        guard JSONSerialization.isValidJSONObject(object),
            let data = try? JSONSerialization.data(
                withJSONObject: object,
                options: [.sortedKeys, .withoutEscapingSlashes]
            )
        else {
            FileHandle.standardError.write(Data("shell-vision: the reply is not valid JSON\n".utf8))
            return
        }
        var line = data
        line.append(0x0A)
        try? handle.write(contentsOf: line)
    }
}

// ── main: the one-shot lifecycle (the brain-afm @main shape) ─────────────────

@main
struct ShellVisionSidecar {
    static func main() async {
        let all = FileHandle.standardInput.readDataToEndOfFile()
        // Two-body stdin: line one is the request; everything after it is
        // the PDF bytes (the protocol's shape — no base64, no second file).
        let separatorIndex = all.firstIndex(of: 0x0A)
        let requestLine = separatorIndex.map { all[all.startIndex..<$0] } ?? Data()
        let pdfBytes = separatorIndex.map { all[all.index(after: $0)...] } ?? Data()

        let (reply, code) = await process(
            requestLine: Data(requestLine),
            pdfBytes: Data(pdfBytes)
        )
        ReplyWriter().write(reply)
        exit(code)
    }
}
