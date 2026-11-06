import Foundation
import MLXLMCommon
import Testing

@testable import MLXFMProbe

/// P-3 (CG-11): spec decode is acceptance-gated and OFF by default.
///
/// Three halves:
/// 1. THE GATE — `specDecodeEvidenced` is the pinned threshold (≥ 40%):
///    spec decode nobody accepts is pure overhead, so the tier posture may
///    move ON only above the gate, measured off the draft-model path's
///    native `SpeculativeDecodingTelemetry.acceptanceRate` (C-1: the
///    MTP-only `acceptedDraftTokens` field stays nil on this path).
/// 2. THE POSTURE — every roster tier is OFF until the acceptance table
///    moves it: a hardcoded-on tier cannot hide.
/// 3. THE OFF POSTURE NEVER PAYS — `resolvedSpecDecode` returns nil for a
///    disabled tier WITHOUT touching the drafter loader: an OFF posture
///    must not load 0.6B weights beside the target.
@Suite("Spec decode pins (P-3)")
struct SpecDecodePins {

    @Test("the acceptance gate holds the 40% threshold")
    func acceptanceGateHolds() {
        guard #available(macOS 27.0, *) else { return }
        #expect(!MLXBridge.specDecodeEvidenced(acceptanceRate: 0.39),
                "just below the gate must NOT evidence spec decode")
        #expect(MLXBridge.specDecodeEvidenced(acceptanceRate: 0.40),
                "at the gate it evidences")
        #expect(MLXBridge.specDecodeEvidenced(acceptanceRate: 0.85),
                "well above the gate it evidences")
    }

    @Test("every roster tier's spec-decode posture is OFF by default")
    func tierPosturesOffByDefault() {
        // The FULL roster (the CG-11 amendments brought it to seven MLX
        // tiers).
        for modelId in ["qwen3-8b-4bit", "qwen3.5-2b-4bit", "qwen3.5-9b-heretic-5bit",
                        "qwen3.5-9b-4bit", "qwen3.5-4b-uncensored-4bit",
                        "qwen3.5-9b-heretic-4bit", "qwen3.5-4b-huihui-4bit"] {
            guard let tier = MLXChatTier.tier(modelId: modelId) else {
                Issue.record("roster id \(modelId) does not map to a tier")
                continue
            }
            #expect(!tier.specDecode,
                    "\(modelId) must be OFF until the acceptance table moves it")
        }
    }

    @Test("the OFF posture never touches the drafter loader")
    func offPostureNeverLoads() async throws {
        guard #available(macOS 27.0, *) else { return }
        guard let tier = MLXChatTier.tier(modelId: "qwen3-8b-4bit") else {
            Issue.record("the 8B id does not map to a tier")
            return
        }
        let resolved = try await MLXBridge.resolvedSpecDecode(tier: tier) {
            // The loader TRAPS: an OFF posture must never reach it. (The
            // measurement override could turn this pin into a false red on
            // a machine exported with BRAIN_MLX_SPEC_DECODE=1 — that env is
            // the instrument's, and this pin documents the dependence.)
            Issue.record("the drafter loader ran for an OFF posture")
            throw CocoaError(.featureUnsupported)
        }
        #expect(resolved == nil, "an OFF posture resolves to nil")
    }
}
