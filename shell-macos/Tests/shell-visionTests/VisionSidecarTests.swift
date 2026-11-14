// shell-vision framing tests (IMPROVE05 S2): the protocol doc and the WIRE
// agree. These spawn the BUILT BINARY — the honest wire (stdin bytes → one
// reply line → exit code), not an import: a hyphenated executable has no
// importable module, and the wire is the contract under test. The corpus
// comes from the source tree at #filePath (the apple-verified-quotes
// precedent); the clean-scan round-trip runs the REAL Vision API, so these
// tests are macOS-only by nature — the lane that runs them is the macOS
// lane, and the kernel-side absence path carries the Linux half.

import XCTest

final class VisionSidecarTests: XCTestCase {
    // ── locating the binary + the corpus ────────────────────────────────────

    private var packageDir: URL {
        // Tests/shell-visionTests/<this file> → the shell-macos package dir.
        URL(fileURLWithPath: (#filePath as NSString).deletingLastPathComponent)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }

    private var repoRoot: URL {
        packageDir.deletingLastPathComponent()
    }

    /// The built executable, probed across the SwiftPM layouts (the same
    /// probing discipline sidecar.mjs uses: a layout change fails loudly
    /// here rather than silently skipping the tests).
    private var sidecarURL: URL {
        let candidates = [
            packageDir.appendingPathComponent(".build/debug/shell-vision"),
            packageDir.appendingPathComponent(".build/arm64-apple-macosx/debug/shell-vision"),
            packageDir.appendingPathComponent(".build/release/shell-vision"),
            packageDir.appendingPathComponent(".build/apple/Products/Debug/shell-vision"),
        ]
        for candidate in candidates where FileManager.default.fileExists(atPath: candidate.path) {
            return candidate
        }
        fatalError("shell-vision binary not found — probed:\n  \(candidates.map(\.path).joined(separator: "\n  "))")
    }

    private func fixture(_ name: String) -> Data {
        let url = repoRoot
            .appendingPathComponent("tests/fixtures/documents/ocr-real/\(name)")
        do {
            return try Data(contentsOf: url)
        } catch {
            fatalError("fixture missing: \(url.path) — \(error)")
        }
    }

    private var protocolDoc: String {
        try! String(contentsOf: repoRoot.appendingPathComponent("docs/OCR_SIDECAR_PROTOCOL.md"), encoding: .utf8)
    }

    // ── the harness ─────────────────────────────────────────────────────────

    struct Reply {
        let exitCode: Int32
        let object: [String: Any]
    }

    /// Spawn the sidecar with the request line + PDF bytes on stdin; parse
    /// the ONE reply line on stdout.
    private func run(request: [String: Any], pdf: Data, environment: [String: String] = [:]) throws -> Reply {
        let requestLine = try JSONSerialization.data(withJSONObject: request, options: [.sortedKeys])
        var stdin = requestLine
        stdin.append(0x0A)
        stdin.append(pdf)
        return try runRaw(stdin, environment: environment)
    }

    private func runRaw(_ stdin: Data, environment: [String: String] = [:]) throws -> Reply {
        let process = Process()
        process.executableURL = sidecarURL
        process.standardInput = Pipe()
        let stdout = Pipe()
        process.standardOutput = stdout
        process.standardError = Pipe()
        if !environment.isEmpty {
            process.environment = environment
        }
        try process.run()
        // write ALL of stdin then close so the child's read-to-EOF completes
        let inputPipe = process.standardInput as! Pipe
        inputPipe.fileHandleForWriting.write(stdin)
        inputPipe.fileHandleForWriting.closeFile()
        process.waitUntilExit()
        let out = stdout.fileHandleForReading.readDataToEndOfFile()

        let lines = out.split(separator: 0x0A, omittingEmptySubsequences: true)
        XCTAssertEqual(lines.count, 1, "exactly one reply line, got \(lines.count)")
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: lines.first.map(Data.init) ?? Data())
                as? [String: Any],
            "the reply line is a JSON object"
        )
        return Reply(exitCode: process.terminationStatus, object: object)
    }

    /// The honest boundary for a page entry that carries a CODE instead of
    /// text: `ocr_assets_unavailable` is the machine missing the on-demand
    /// recognition model (an environment condition — skip with the GREPPABLE
    /// notice the workflow's report step looks for, the AppleFMProbeTests
    /// precedent); any other code is a genuine recognition failure — the
    /// message surfaces in the failure and the lane reds. Returns nil when
    /// the test should stop (skip or fail).
    private func assetBoundary(_ page: [String: Any]) throws -> Bool {
        guard let code = page["code"] as? String else { return true }
        let message = (page["message"] as? String) ?? ""
        if code == "ocr_assets_unavailable" {
            throw XCTSkip("on-device vision recognition assets unavailable: \(message)")
        }
        XCTFail("the page genuinely failed recognition: \(code) — \(message)")
        return false
    }

    // ── the framing pins ────────────────────────────────────────────────────

    /// The REAL Vision round-trip on the committed corpus: per-page text +
    /// confidence present, the engine string names the request type and the
    /// OS build, exit 0. (The one test that exercises the actual model.)
    func testCleanScanRoundTripRealVision() throws {
        let reply = try run(
            request: ["id": "t1", "op": "ocr", "dpi": 300],
            pdf: fixture("clean-scan.pdf")
        )
        XCTAssertEqual(reply.exitCode, 0, "exit 0 when every page answered")
        XCTAssertEqual(reply.object["ok"] as? Bool, true)
        XCTAssertEqual(reply.object["id"] as? String, "t1", "the id echoes verbatim")
        let engine = try XCTUnwrap(reply.object["engine"] as? String, "engine present when ok")
        XCTAssertTrue(
            engine.hasPrefix("apple-vision/recognize-text@"),
            "the engine string names the request type + OS build, got: \(engine)"
        )
        let pages = try XCTUnwrap(reply.object["pages"] as? [[String: Any]])
        XCTAssertEqual(pages.count, 1)
        guard try assetBoundary(pages[0]) else { return }
        let text = try XCTUnwrap(pages[0]["text"] as? String)
        XCTAssertFalse(text.isEmpty, "the clean scan recognizes text")
        XCTAssertTrue(
            text.contains("On-Device Transcription"),
            "the recognized text is the expected prose, got: \(text.prefix(80))"
        )
        let confidence = try XCTUnwrap(pages[0]["confidence"] as? Double)
        XCTAssertTrue((0.0...1.0).contains(confidence), "confidence is 0..1, got \(confidence)")
    }

    /// The additive-optional `pages` field: the reply carries EXACTLY the
    /// requested pages (OV-1's wire half — the kernel names only the empty
    /// pages). The hybrid's page 2 is image-only with no readable pixels,
    /// so the honest answer is an empty text with NO confidence claim.
    func testPagesFieldCarriesOnlyRequestedPages() throws {
        let reply = try run(
            request: ["id": "t2", "op": "ocr", "dpi": 300, "pages": [2]],
            pdf: try! Data(contentsOf: repoRoot.appendingPathComponent("tests/fixtures/documents/hybrid.pdf"))
        )
        XCTAssertEqual(reply.exitCode, 0)
        XCTAssertEqual(reply.object["ok"] as? Bool, true)
        let pages = try XCTUnwrap(reply.object["pages"] as? [[String: Any]])
        XCTAssertEqual(pages.count, 1, "exactly the requested page, got \(pages.count)")
        XCTAssertEqual(pages[0]["index"] as? Int, 2)
        guard try assetBoundary(pages[0]) else { return }
        XCTAssertEqual(pages[0]["text"] as? String, "", "the 4x4 XObject recognizes nothing — honest emptiness")
        XCTAssertNil(pages[0]["confidence"], "no text → no confidence claim")
    }

    /// All-pages form (no `pages` field): the hybrid answers once per page,
    /// ascending.
    func testAbsentPagesMeansAllPagesAscending() throws {
        let reply = try run(
            request: ["id": "t3", "op": "ocr"],
            pdf: try! Data(contentsOf: repoRoot.appendingPathComponent("tests/fixtures/documents/hybrid.pdf"))
        )
        XCTAssertEqual(reply.exitCode, 0)
        let pages = try XCTUnwrap(reply.object["pages"] as? [[String: Any]])
        XCTAssertEqual(pages.map { $0["index"] as? Int }, [1, 2, 3, 4], "ascending, complete")
    }

    func testWrongOpIsBadRequestExit2() throws {
        let reply = try run(
            request: ["id": "t4", "op": "chat"],
            pdf: fixture("clean-scan.pdf")
        )
        XCTAssertEqual(reply.exitCode, 2, "bad_request exits 2")
        XCTAssertEqual(reply.object["ok"] as? Bool, false)
        XCTAssertEqual(reply.object["code"] as? String, "bad_request")
        XCTAssertNil(reply.object["pages"], "a top-level failure carries no pages")
    }

    func testDpiOutOfBandIsBadRequest() throws {
        let reply = try run(
            request: ["id": "t5", "op": "ocr", "dpi": 10],
            pdf: fixture("clean-scan.pdf")
        )
        XCTAssertEqual(reply.exitCode, 2)
        XCTAssertEqual(reply.object["code"] as? String, "bad_request")
    }

    func testNonPdfBytesAreRasterFailedExit3() throws {
        let reply = try run(
            request: ["id": "t6", "op": "ocr"],
            pdf: Data("not a pdf at all".utf8)
        )
        XCTAssertEqual(reply.exitCode, 3, "raster_failed exits 3")
        XCTAssertEqual(reply.object["ok"] as? Bool, false)
        XCTAssertEqual(reply.object["code"] as? String, "raster_failed")
    }

    func testTrulyEmptyStdinIsBadRequestWithNullId() throws {
        // NO request line at all: bad_request with "id":null (the protocol's
        // rule — the id cannot be read, so it cannot be echoed).
        let reply = try runRaw(Data())
        XCTAssertEqual(reply.exitCode, 2, "bad_request exits 2")
        XCTAssertEqual(reply.object["code"] as? String, "bad_request")
        XCTAssertTrue(reply.object["id"] is NSNull, "an unreadable id crosses as null")
    }

    /// The structural half: every named code this binary can emit appears in
    /// the protocol doc — the doc and the wire cannot drift silently.
    func testProtocolDocNamesEveryCode() {
        let doc = protocolDoc
        for code in ["bad_request", "raster_failed", "ocr_page_failed", "ocr_assets_unavailable"] {
            XCTAssertTrue(doc.contains("`\(code)`"), "the protocol doc names \(code)")
        }
        XCTAssertTrue(
            doc.contains("apple-vision/recognize-text@"),
            "the protocol doc carries the engine-string shape"
        )
    }

    /// The environment verdict's WIRE SHAPE, deterministically: the probe's
    /// forced refusal (SHELL_VISION_PROBE_FAIL — the sidecar's env-gated
    /// test surface) makes every requested page carry
    /// `ocr_assets_unavailable` with the probe's evidence. This is the
    /// shape the real-Vision pins skip on when a machine cannot OCR.
    func testForcedProbeFailureCarriesTheEnvironmentVerdict() throws {
        let reply = try run(
            request: ["id": "t7", "op": "ocr", "dpi": 300],
            pdf: fixture("clean-scan.pdf"),
            environment: ["SHELL_VISION_PROBE_FAIL": "1"]
        )
        XCTAssertEqual(reply.exitCode, 0, "the probe verdict is a per-page verdict; the reply completes")
        XCTAssertEqual(reply.object["ok"] as? Bool, true)
        let pages = try XCTUnwrap(reply.object["pages"] as? [[String: Any]])
        XCTAssertEqual(pages.count, 1)
        XCTAssertEqual(pages[0]["code"] as? String, "ocr_assets_unavailable")
        XCTAssertEqual(pages[0]["ok"] as? Bool, false)
        let message = try XCTUnwrap(pages[0]["message"] as? String)
        XCTAssertTrue(
            message.contains("capability probe"),
            "the probe's evidence rides verbatim, got: \(message)"
        )
        XCTAssertNil(pages[0]["text"], "a refused page carries no text")
        XCTAssertNil(pages[0]["confidence"], "a refused page carries no confidence claim")
    }
}