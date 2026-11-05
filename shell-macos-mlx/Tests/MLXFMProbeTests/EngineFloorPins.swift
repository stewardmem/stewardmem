import Foundation
import Testing

/// PINS THE ENGINE FLOOR (CG-10 E1a/E1c, amended lane M-2): the resolved
/// swift-jinja must be ≥ 3.0.0. On 1.3.0 — what `swift-transformers 0.1.x`
/// resolved until the CG-10 bump — the slice evaluator returns EMPTY for
/// any implicit-range slice with a step (`messages[::-1]` on a 2+ element
/// list), and every Qwen3.5 chat template reverse-scans the conversation
/// through exactly that construct: EVERY multi-turn after turn 1 raises
/// `No user query found in messages.`, regardless of factory path or
/// config (measured by the `template-debug` bisect, outside the model
/// stack). Upstream fixed it in #99 + #101 + the Jinja2 conformance suite
/// #95, released as swift-jinja 3.0.0 (2026-10-27) — reached by OUR
/// swift-transformers 1.4.0 bump. A future resolution regression to a
/// pre-3.0.0 engine silently re-breaks every Qwen3.5 tier; this pin makes
/// it loud instead.
@Suite("The swift-jinja engine floor")
struct EngineFloorPins {

    @Test("swift-jinja resolves ≥ 3.0.0 — the Qwen3.5 multi-turn floor")
    func jinjaEngineMeetsTheFloor() throws {
        // Package.resolved sits at the package root, three levels up from
        // this file (file → MLXFMProbeTests → Tests → package root).
        let here = URL(fileURLWithPath: #filePath, isDirectory: false)
        let root = here
            .deletingLastPathComponent()  // Tests/MLXFMProbeTests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // package root
        let data = try Data(contentsOf: root.appendingPathComponent("Package.resolved"))
        let resolved = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let pins = try #require(
            resolved?["pins"] as? [[String: Any]],
            "Package.resolved carries pins"
        )

        var jinjaVersion: String?
        for pin in pins {
            guard (pin["identity"] as? String) == "swift-jinja" else { continue }
            let state = pin["state"] as? [String: Any]
            jinjaVersion = state?["version"] as? String
        }
        let version = try #require(jinjaVersion, "swift-jinja must appear in Package.resolved")

        let parts = version.split(separator: ".").map { Int($0) ?? 0 }
        let major = parts.count > 0 ? parts[0] : 0
        let minor = parts.count > 1 ? parts[1] : 0
        let patch = parts.count > 2 ? parts[2] : 0
        #expect(
            (major, minor, patch) >= (3, 0, 0),
            "swift-jinja resolved \(version) — below the 3.0.0 floor, every Qwen3.5 multi-turn raises `No user query found in messages.` after turn 1 (the 1.3.0 slice bug); bump swift-transformers to ≥ 1.4.0"
        )
    }
}
