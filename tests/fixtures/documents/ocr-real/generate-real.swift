#!/usr/bin/env swift
// generate-real.swift — the ONE-TIME generator for the five REAL image
// classes (IMPROVE05 S1). macOS-only (CoreText/CoreImage/ImageIO).
//
//   swift tests/fixtures/documents/ocr-real/generate-real.swift
//
// The committed bytes are FROZEN AT COMMIT; SHA256SUMS (written once, next
// to this file) is the authority. Re-running this script is provenance, not
// a build step: font rasterization and the noise seed may differ across
// macOS versions, so the committed bytes are never regenerated over — a
// changed hash after a re-run is the EXPECTED signal that the platform
// moved, and the corpus stays as committed.
//
// The five classes (each a single-page image-only PDF — JPEG-wrapped, no
// text operators — the scanned.pdf shape with real pixels):
//
//   clean-scan.pdf        300 DPI grayscale, serif book page
//   photo-of-page.pdf     a photographed page: keystone + rotation + noise
//                         + slight blur + warm cast, RGB
//   multi-column-scan.pdf 300 DPI grayscale, two columns
//   handwriting.pdf       a script-font page (Bradley Hand)
//   non-latin.pdf         a Simplified Chinese page (PingFang SC)
//
// Each class carries an expected-text/<name>.txt file: the EXACT source
// text rendered into the image. The WER helper (scripts/wer.py) measures
// the sidecar's transcription against it (S4).

import CoreGraphics
import CoreImage
import CoreText
import CryptoKit
import Foundation
import ImageIO

// ── the fixed geometry (recorded per fixture: DPI 300, US Letter) ───────────

let DPI = 300.0
let PAGE_W = 8.5 * DPI  // 2550 px
let PAGE_H = 11.0 * DPI  // 3300 px
let MARGIN = 1.0 * DPI  // 1 inch
let JPEG_QUALITY = 0.8

let here = URL(fileURLWithPath: (#filePath as NSString).deletingLastPathComponent)

func sha256Hex(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

// ── bitmap rendering (CoreText, one line at a time) ─────────────────────────

func makeContext(rgb: Bool) -> CGContext {
    // CGBitmapContext has no grayscale pixel format, so EVERY page renders
    // sRGB; the gray classes convert at JPEG-encode time (ImageIO's
    // kCGImageDestinationColorModel, see jpegData).
    let ctx = CGContext(
        data: nil,
        width: Int(PAGE_W), height: Int(PAGE_H),
        bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
    )!
    // paper
    ctx.setFillColor(rgb ? CGColor(red: 1, green: 1, blue: 1, alpha: 1) : CGColor(gray: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: PAGE_W, height: PAGE_H))
    ctx.setFillColor(rgb ? CGColor(red: 0, green: 0, blue: 0, alpha: 1) : CGColor(gray: 0, alpha: 1))
    return ctx
}

/// Draw one line of text with its baseline at `y` (bottom-left origin).
/// `sizePt` is in TYPE POINTS; the render scales it to the fixed DPI.
func drawLine(_ ctx: CGContext, _ text: String, x: CGFloat, y: CGFloat, fontName: String, sizePt: CGFloat) {
    guard !text.isEmpty else { return }
    let font = CTFontCreateWithName(fontName as CFString, sizePt * DPI / 72.0, nil)
    let attr = NSAttributedString(string: text, attributes: [kCTFontAttributeName as NSAttributedString.Key: font])
    let line = CTLineCreateWithAttributedString(attr)
    ctx.textPosition = CGPoint(x: x, y: y)
    CTLineDraw(line, ctx)
}

/// A simple left-aligned block: returns the y of the next baseline.
func drawBlock(_ ctx: CGContext, _ lines: [String], x: CGFloat, topY: CGFloat, fontName: String, sizePt: CGFloat, leading: CGFloat) -> CGFloat {
    var y = topY
    for line in lines {
        drawLine(ctx, line, x: x, y: y, fontName: fontName, sizePt: sizePt)
        y -= sizePt * DPI / 72.0 * leading
    }
    return y
}

func cgImage(_ ctx: CGContext) -> CGImage { ctx.makeImage()! }

// ── photo-of-page: the CoreImage chain (keystone + rotate + noise + blur) ────

func photoOfPage(_ base: CGImage) -> CGImage {
    var ci = CIImage(cgImage: base)
    let w = ci.extent.width, h = ci.extent.height
    // a photographed page: slight keystone (top edge narrower) + 1.1° tilt
    let keystone = CIFilter(name: "CIPerspectiveTransform")!
    keystone.setValue(ci, forKey: kCIInputImageKey)
    let pull: CGFloat = w * 0.012
    let tilt = CGFloat(1.1) * .pi / 180
    func rot(_ p: CGPoint) -> CGPoint {
        let s = sin(tilt), c = cos(tilt)
        return CGPoint(x: p.x * c - p.y * s, y: p.x * s + p.y * c)
    }
    keystone.setValue(CIVector(cgPoint: rot(CGPoint(x: 0, y: h))), forKey: "inputTopLeft")
    keystone.setValue(CIVector(cgPoint: rot(CGPoint(x: w, y: h + pull))), forKey: "inputTopRight")
    keystone.setValue(CIVector(cgPoint: rot(CGPoint(x: 0, y: -pull))), forKey: "inputBottomLeft")
    keystone.setValue(CIVector(cgPoint: rot(CGPoint(x: w, y: 0))), forKey: "inputBottomRight")
    ci = keystone.outputImage!.transformed(by: CGAffineTransform(translationX: pull, y: 0))

    // sensor noise at ~2% opacity, generated at 1/8 resolution and upscaled:
    // full-resolution random grain is incompressible (measured: a 5.4 MB
    // JPEG) and, at 4% alpha with an 8x upscale, dense enough that Vision
    // returned ZERO observations (measured) — heavier than any real phone
    // camera. 2% at 4x upscale + a settling blur reads as grain while the
    // text stays machine-readable (the class measures camera conditions,
    // not the impossibility of reading).
    let noise = CIFilter(name: "CIRandomGenerator")!.outputImage!
        .cropped(to: CGRect(x: 0, y: 0, width: Int(w / 4), height: Int(h / 4)))
        .transformed(by: CGAffineTransform(scaleX: 4, y: 4))
    let faint = noise.applyingFilter("CIColorMatrix", parameters: [
        "inputRVector": CIVector(x: 0, y: 0, z: 0, w: 0),
        "inputGVector": CIVector(x: 0, y: 0, z: 0, w: 0),
        "inputBVector": CIVector(x: 0, y: 0, z: 0, w: 0),
        "inputAVector": CIVector(x: 0.02, y: 0.02, z: 0.02, w: 0),
        "inputBiasVector": CIVector(x: 0, y: 0, z: 0, w: 0),
    ])
    ci = faint.applyingFilter("CISourceOverCompositing", parameters: [kCIInputBackgroundImageKey: ci])
    ci = ci.applyingFilter("CIGaussianBlur", parameters: [kCIInputRadiusKey: 0.8])

    // hand-held focus + a warm indoor cast
    ci = ci.applyingFilter("CIGaussianBlur", parameters: [kCIInputRadiusKey: 1.1])
    ci = ci.applyingFilter("CIColorControls", parameters: [
        kCIInputSaturationKey: 0.9, kCIInputContrastKey: 0.95, kCIInputBrightnessKey: 0.01,
    ])
    ci = ci.applyingFilter("CIColorMatrix", parameters: [
        "inputRVector": CIVector(x: 1.02, y: 0, z: 0, w: 0),
        "inputGVector": CIVector(x: 0, y: 1.0, z: 0, w: 0),
        "inputBVector": CIVector(x: 0, y: 0, z: 0.95, w: 0),
    ])

    // crop to the interior so no filter edge shows; a phone photo of a page
    // is realistically ~200 DPI, not the scanner's 300 (the class measures
    // camera conditions, and ⅔ scale also keeps the committed bytes sane).
    let scale: CGFloat = 2.0 / 3.0
    ci = ci.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
    let ew = ci.extent.width, eh = ci.extent.height
    let inset: CGFloat = ew * 0.02
    ci = ci.cropped(to: CGRect(x: inset, y: inset, width: ew - 2 * inset, height: eh - 2 * inset))
    return CIContext().createCGImage(ci, from: ci.extent)!
}

// ── JPEG + the hand-built image-only PDF (the scanned.pdf shape) ─────────────

func jpegData(_ image: CGImage, quality: Double, gray: Bool) -> (Data, Int, Int) {
    // Gray classes become TRUE 1-channel images through CIContext's L8
    // format (CGBitmapContext has no gray pixel format, and the encode-time
    // color-model keys are not public API — measured: the JPEG came out RGB
    // while the PDF XObject declared /DeviceGray, and CGPDF refused to draw
    // it). A 1-channel CGImage encodes as a grayscale JPEG with no property.
    let target: CGImage
    if gray {
        let ci = CIImage(cgImage: image)
        target = CIContext().createCGImage(
            ci, from: ci.extent, format: .L8,
            colorSpace: CGColorSpace(name: CGColorSpace.genericGrayGamma2_2)!
        )!
    } else {
        target = image
    }
    let out = NSMutableData()
    let dest = CGImageDestinationCreateWithData(out, "public.jpeg" as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, target, [
        kCGImageDestinationLossyCompressionQuality: quality,
    ] as CFDictionary)
    CGImageDestinationFinalize(dest)
    return (out as Data, target.width, target.height)
}

/// Hand-built %PDF-1.4 wrapping one DCTDecode image (computed xref — the
/// generate.py/render_pdf shape; no timestamps, no /ID).
func imageOnlyPDF(jpeg: Data, width: Int, height: Int, rgb: Bool) -> Data {
    var out = Data("%PDF-1.4\n".utf8)
    var offsets: [Int] = []

    func emit(_ no: Int, _ body: String, payload: Data? = nil) {
        offsets.append(out.count)
        out.append(Data("\(no) 0 obj\n\(body)\n".utf8))
        if let payload {
            out.append(payload)
            out.append(Data("\nendstream\nendobj\n".utf8))
        } else {
            out.append(Data("endobj\n".utf8))
        }
    }

    let cs = rgb ? "/DeviceRGB" : "/DeviceGray"
    let content = "q 612 0 0 792 0 0 cm /Im0 Do Q"
    emit(1, "<< /Type /Catalog /Pages 2 0 R >>")
    emit(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    emit(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        + "/Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>")
    // the single EOL after `stream` is REQUIRED (the spec's stream keyword
    // is followed by CRLF or LF before the data; a missing one is the
    // black-page failure measured here).
    emit(4, "<< /Type /XObject /Subtype /Image /Width \(width) /Height \(height) "
        + "/ColorSpace \(cs) /BitsPerComponent 8 /Filter /DCTDecode "
        + "/Length \(jpeg.count) >>\nstream", payload: jpeg)
    emit(5, "<< /Length \(content.utf8.count) >>\nstream\n\(content)\nendstream")

    let xrefAt = out.count
    out.append(Data("xref\n0 6\n0000000000 65535 f \n".utf8))
    for off in offsets {
        out.append(Data(String(format: "%010d 00000 n \n", off).utf8))
    }
    out.append(Data("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n\(xrefAt)\n%%EOF\n".utf8))
    return out
}

// ── the five classes' content (the expected-text files carry these VERBATIM) ─

let cleanScanText = """
    On-Device Transcription

    The scanner delivers a picture of a page, and a picture cites nothing.
    The recognition engine reads it on this machine, and the reading crosses
    the same doors that typed text crosses: the strip, the screen, the
    dedupe, and the fence. Nothing about the path is privileged by its
    medium.

    Every recognized page carries its own provenance. The engine stamps the
    request type and the build of the operating system it ran under, the
    page carries the lowest confidence among its candidates, and a weak
    region marks the whole page rather than hiding inside it.

    A page the engine refuses is a named refusal, never an invented blank.
    The record shows the hole and the reason for the hole, because a
    transcript that hides its gaps is a transcript that manufactures
    confidence it did not earn.
    """

let photoText = """
    A Note Left on the Desk

    This page was photographed rather than scanned. The camera tilted, the
    room light was warm, and the sensor added its own grain to the paper.
    Recognition still owes the same honesty here: the words it reads are
    recorded verbatim, and the confidence it reports is the confidence it
    actually holds.

    The point of this fixture is the class of failure, not the failure
    itself. A clean scan flatters the engine; a photograph asks the
    question every desk in the world actually asks.
    """

let multiColumnLeft = """
    Left Column: The Two Doors

    Every page of this document was laid out in two columns. The reading
    order a human follows is column by column, but a recognition pass sees
    geometry first and semantics second, so the transcript may interleave
    what the layout separated.

    The transcript is still faithful to the page: fidelity to the source
    order is a layout claim, and the header never makes one. The words are
    the words; their order is the engine's reading order.
    """

let multiColumnRight = """
    Right Column: What Is Measured

    The error rate for this class counts substitutions, deletions, and
    insertions against the column-ordered reference. A gap here is a real
    signal: either a word was misread, or the reading order moved, and the
    evidence records which class the gap belongs to.
    """

let multiColumnText = multiColumnLeft + "\n\n" + multiColumnRight

let handwritingText = """
    Shopping list and reminders

    Pick up oats, olive oil, and oranges on the way home.
    The dentist moved the appointment to Thursday at four.
    Call the plumber about the kitchen tap before the weekend.
    Send the signed form back to the office by Friday.
    """

let nonLatinText = """
    本地优先的智能体记忆

    智能体把学到的东西存进内核。每次写入都经过同一道门：注入检查器阅读文本，
    信任标签约束来源，审计链在不保存原文的前提下记录这次写入。

    召回由三条腿融合。词法腿通过全文索引匹配精确词项；语义腿嵌入查询并按余弦
    距离排序向量；图谱腿沿有类型的关系行走，让两跳答案能引用它走过的路径。

    热路径上没有任何模型调用。嵌入是本地、确定、零成本的；排序是存储行的纯
    函数。一次不能离线运行的召回，是已经辜负自身立场的召回。
    """

// ── the five builds ──────────────────────────────────────────────────────────

func build(name: String, rgb: Bool = false, text: String, expectedName: String? = nil,
           font: String = "Georgia", sizePt: CGFloat = 11, leading: CGFloat = 1.55,
           columns: (left: String, right: String)? = nil, quality: Double = JPEG_QUALITY,
           transform: ((CGImage) -> CGImage)? = nil)
{
    let ctx = makeContext(rgb: rgb)
    if let columns {
        // two columns, each wrapped to its own measure
        let colW: CGFloat = (PAGE_W - 3 * MARGIN) / 2
        let wrappedL = wrap(columns.left, width: colW, fontName: font, sizePt: sizePt)
        let wrappedR = wrap(columns.right, width: colW, fontName: font, sizePt: sizePt)
        drawBlock(ctx, wrappedL, x: MARGIN, topY: PAGE_H - MARGIN, fontName: font, sizePt: sizePt, leading: leading)
        drawBlock(ctx, wrappedR, x: 2 * MARGIN + colW, topY: PAGE_H - MARGIN, fontName: font, sizePt: sizePt, leading: leading)
    } else {
        let measure = PAGE_W - 2 * MARGIN
        let wrapped = wrap(text, width: measure, fontName: font, sizePt: sizePt)
        drawBlock(ctx, wrapped, x: MARGIN, topY: PAGE_H - MARGIN, fontName: font, sizePt: sizePt, leading: leading)
    }
    var image = cgImage(ctx)
    if let transform { image = transform(image) }
    let (jpeg, w, h) = jpegData(image, quality: quality, gray: !rgb)
    let pdf = imageOnlyPDF(jpeg: jpeg, width: w, height: h, rgb: rgb)
    try! pdf.write(to: here.appendingPathComponent("\(name).pdf"))
    let expected = (expectedName ?? name)
    try! text.write(to: here.appendingPathComponent("expected-text/\(expected).txt"), atomically: true, encoding: .utf8)
    print("\(name).pdf  \(jpeg.count) bytes jpeg, \(w)x\(h), \(rgb ? "rgb" : "gray")")
}

/// Naive word wrap so lines fit the column measure (the corpus is prose, the
/// wrap is deterministic: greedy, break on spaces).
func wrap(_ text: String, width: CGFloat, fontName: String, sizePt: CGFloat) -> [String] {
    let font = CTFontCreateWithName(fontName as CFString, sizePt * DPI / 72.0, nil)
    let pxWidth = { (s: String) -> CGFloat in
        let attr = NSAttributedString(string: s, attributes: [kCTFontAttributeName as NSAttributedString.Key: font])
        let line = CTLineCreateWithAttributedString(attr)
        return CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
    }
    var out: [String] = []
    for para in text.split(separator: "\n", omittingEmptySubsequences: false) {
        if para.isEmpty { out.append(""); continue }
        var line = ""
        for word in para.split(separator: " ", omittingEmptySubsequences: false) {
            let candidate = line.isEmpty ? String(word) : line + " " + word
            if pxWidth(candidate) > width, !line.isEmpty {
                out.append(line)
                line = String(word)
            } else {
                line = candidate
            }
        }
        out.append(line)
    }
    return out
}

build(
    name: "clean-scan", text: cleanScanText,
    expectedName: "clean-scan",
    font: "Georgia", sizePt: 11, leading: 1.6,
)

build(
    name: "photo-of-page", rgb: true, text: photoText,
    expectedName: "photo-of-page",
    font: "Georgia", sizePt: 11, leading: 1.6,
    quality: 0.6,
    transform: photoOfPage(_:),
)

build(
    name: "multi-column-scan", text: multiColumnText,
    expectedName: "multi-column-scan",
    font: "Georgia", sizePt: 10.5, leading: 1.55,
    columns: (multiColumnLeft, multiColumnRight),
)

build(
    name: "handwriting", text: handwritingText,
    expectedName: "handwriting",
    font: "Bradley Hand", sizePt: 22, leading: 2.0,
)

build(
    name: "non-latin", text: nonLatinText,
    expectedName: "non-latin",
    font: "PingFang SC", sizePt: 14, leading: 1.9,
)

// ── the manifest (written ONCE; the committed file is the authority) ─────────

let dir = here.appendingPathComponent("expected-text").path
let names = ["clean-scan", "photo-of-page", "multi-column-scan", "handwriting", "non-latin"]
var manifest = ""
for n in names {
    for f in ["\(n).pdf", "expected-text/\(n).txt"] {
        let data = try! Data(contentsOf: here.appendingPathComponent(f))
        manifest += "\(sha256Hex(data))  \(f)\n"
    }
}
try! manifest.write(to: here.appendingPathComponent("SHA256SUMS"), atomically: true, encoding: .utf8)
print("SHA256SUMS written (\(try! FileManager.default.contentsOfDirectory(atPath: dir).count) expected-text files)")
