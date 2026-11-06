import Foundation
import Testing

@testable import MLXFMProbe

/// P-6 (CG-11): PFlash attention is ABSENT from the chat path — and the
/// refusal is PINNED as a source scan, not just stated. PFlash drops ~80%
/// of the prompt middle for speed; on a cite-or-abstain product the middle
/// IS the evidence, so a PFlash-equipped chat path would answer grounded
/// questions from mutilated evidence while the support gate labeled the
/// assertions — evidence mutilation wearing an honesty costume. The
/// lane's law: absent, and kept absent BY PIN.
///
/// The scan walks the three surfaces the chat path actually lives in —
/// the MLX package, the bridge child, and the kernel — for the PFlash
/// vocabulary. The needle is the class-ish identifier set, not prose (a
/// doc comment DISCUSSING the refusal is allowed to exist — the kernel's
/// own records carry one — so only code-shaped tokens count).
struct PFlashAbsencePins {

    /// The code-shaped needles. `PFlash`/`pflash` covers the Swift/Rust
    /// identifier families; `_flash_attn`-style C++ symbols ride the same
    /// stem.
    static let needles = ["PFlash", "pflash"]

    /// Where the chat path lives, relative to the repo root.
    static func chatPathSources() -> [String] {
        let here = URL(fileURLWithPath: #filePath, isDirectory: false)
        // …/Tests/MLXFMProbeTests/PFlashAbsencePins.swift → package dir → repo root
        let root = here.deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        return [
            root.appendingPathComponent("shell-macos-mlx/Sources").path,
            root.appendingPathComponent("shell-macos/Sources").path,
            root.appendingPathComponent("src").path,
        ]
    }

    @Test("PFlash is absent from every chat-path surface (source scan)")
    func pflashAbsentFromChatPath() throws {
        var scanned = 0
        for directory in Self.chatPathSources() {
            let files = try FileManager.default.subpathsOfDirectory(
                atPath: directory)
            for file in files where file.hasSuffix(".swift") || file.hasSuffix(".rs") {
                let url = URL(fileURLWithPath: directory).appendingPathComponent(file)
                guard let source = try? String(contentsOf: url, encoding: .utf8) else {
                    continue
                }
                scanned += 1
                // Comment-strip before scanning (the S1 lesson: a
                // commented needle still contains the needle — but note the
                // DIRECTION here is inverted: this pin wants ABSENCE, so
                // comment residue would FALSELY red. Strip lines whose
                // first non-space char is a comment marker, and block
                // comments, before looking.)
                var code = ""
                var inBlock = false
                for line in source.components(separatedBy: .newlines) {
                    var l = line
                    if inBlock {
                        if let end = l.range(of: "*/") {
                            l = String(l[end.upperBound...])
                            inBlock = false
                        } else {
                            continue
                        }
                    }
                    if let start = l.range(of: "/*") {
                        if let end = l.range(of: "*/", range: start.upperBound..<l.endIndex) {
                            l = l[start.lowerBound..<start.lowerBound]
                                + String(l[end.upperBound...])
                        } else {
                            l = String(l[..<start.lowerBound])
                            inBlock = true
                        }
                    }
                    let trimmed = l.trimmingCharacters(in: .whitespaces)
                    if trimmed.hasPrefix("//") { continue }
                    code += l + "\n"
                }
                for needle in Self.needles {
                    #expect(
                        !code.contains(needle),
                        "\(file) references \(needle) — PFlash entered the chat path; the evidence-mutilation refusal is broken")
                }
            }
        }
        #expect(scanned > 100, "the scan actually walked the tree: \(scanned) files")
    }
}
