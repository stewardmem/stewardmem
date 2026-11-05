import Foundation
// Both macro expansions name modules directly (`HuggingFace.HubClient` in
// #hubDownloader, `Tokenizers.AutoTokenizer` in #huggingFaceTokenizerLoader)
// — the consumer imports both, the same pair the multi-turn-probe target
// carries for the identical reason (see Package.swift).
import HuggingFace
import MLXHuggingFace
import MLXLMCommon
import MLXLLM
import Testing
import Tokenizers

@testable import MLXFMProbe

/// P-1 (CG-11): the KV-scheme gate is FAIL-CLOSED at generation start —
/// an unrecognized `kvScheme` string is refused BY NAME, never silently
/// run at full precision. The refusal fires in the package's plan
/// resolution (`kvCachePlan()` → `unsupportedLegacyScheme`), the same
/// resolution every generation start runs (`ChatSession.streamMap` →
/// `generateParameters.kvCachePlan()`), so pinning it here pins the gate
/// the bridge's whole ratchet stands on: if a future package bump made
/// unknown schemes silently fall back to full precision, the roster's
/// evidenced scheme would rot into a no-op and every measured claim in
/// the lane would quietly stop meaning anything.
///
/// The paired arm pins the ratchet's OWN schemes: `turbo0v4`, `turbo8v3`
/// and `turbo8v4` resolve through the same gate — a typo in
/// `MLXChatTier.kvScheme`'s evidenced posture or an upstream scheme rename
/// reds here before a tier ever serves a degraded cache.
///
/// LIVE (the gate runs inside a loaded model's plan resolution): gated on
/// `BRAIN_FM_MEASURE=1`, the same switch as the other live pins; the CI
/// model (Qwen3-0.6B-4bit, cached locally, the CG-10 probe's CI model)
/// keeps the load at ~334 MB. Without the env the suite stays hermetic.
@Suite("KV scheme pins (P-1)", .serialized)
struct KVSchemePins {

    /// The 0.6B CI model through the SAME LLM-factory text-tower path the
    /// bridge's `MLXBridge.loadContainer` uses — not a separate load shape.
    @available(macOS 27.0, *)
    static func container() async throws -> ModelContainer {
        try await LLMModelFactory.shared.loadContainer(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: ModelConfiguration(id: "mlx-community/Qwen3-0.6B-4bit"))
    }

    static let live = ProcessInfo.processInfo.environment["BRAIN_FM_MEASURE"] == "1"

    /// P-1, the refusal: an unknown scheme throws at the plan gate. The
    /// error names the scheme (`unsupportedLegacyScheme("…")`) — the
    /// fail-closed contract the bridge relies on.
    @Test("unknown kvScheme refused at the plan gate", .enabled(if: live))
    func unknownSchemeRefused() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        let bogus = "definitely-not-a-scheme"
        do {
            // cacheStatus resolves the plan exactly as generation start
            // does — the refusal must fire BEFORE any decode.
            _ = try await container.cacheStatus(
                parameters: GenerateParameters(maxTokens: 8, kvScheme: bogus, temperature: 0))
            Issue.record(
                "an unknown kvScheme was ACCEPTED at the plan gate — the fail-closed law is gone upstream: \(bogus)")
        } catch let error as KVCacheConfigurationError {
            guard case .unsupportedLegacyScheme(let named) = error else {
                Issue.record("refused, but by the wrong error: \(error)")
                return
            }
            #expect(named == bogus, "the refusal must name the scheme: \(named)")
        }
    }

    /// P-1, the paired arm: the ratchet's schemes RESOLVE — they are real
    /// schemes at this pinned revision, so a typo in the roster or an
    /// upstream rename cannot hide behind the refusal arm above.
    @Test("the ratchet's schemes resolve at the plan gate", .enabled(if: live))
    func ratchetSchemesResolve() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        for scheme in ["turbo0v4", "turbo8v3", "turbo8v4"] {
            do {
                _ = try await container.cacheStatus(
                    parameters: GenerateParameters(maxTokens: 8, kvScheme: scheme, temperature: 0))
            } catch {
                Issue.record("the ratchet scheme \(scheme) failed plan resolution: \(error)")
            }
        }
    }
}
