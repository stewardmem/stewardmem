import Foundation
// The load-path macros name modules directly (see KVSchemePins).
import HuggingFace
import MLXHuggingFace
import MLXLMCommon
import MLXLLM
import Testing
import Tokenizers

@testable import MLXFMProbe

/// P-5 (CG-11): the native prefix cache is a MEASUREMENT, not an inference.
/// On a shared-prefix session the second turn's FRESH prefill
/// (`promptTokenCount` — the cache-served half rides `cachedPromptTokenCount`)
/// must be STRICTLY LESS than the first turn's, and the served half must be
/// non-zero: a future refactor (or a package bump) that silently loses the
/// prefix reuse reds here. The wire cannot carry this pin — the bridge's
/// `inputTokens` is the SUM of the two halves (C-2's ground: turn 2's sum
/// grows while its fresh half shrinks), so the pin reads the package's own
/// counts where they live.
///
/// LIVE, gated on `BRAIN_FM_MEASURE=1` like the other live pins; the 0.6B
/// CI model (Qwen3 family — the roster tier whose template renders
/// prefix-stable) keeps the load small.
///
/// THE RECORDED BOUNDARY (the probe's measured split, CG-11 E1): the
/// Qwen3.5 tiers (2B, heretic 9B) show `cachedPromptTokenCount == 0` on
/// turn 2 — the primed-think template's per-turn render is NOT
/// prefix-stable (the final assistant turn renders differently once it has
/// content), so the native prefix cache cannot hit there. That is the
/// template's own contract, upstream's surface — recorded, not chased in
/// this lane.
@Suite("Prefix cache pins (P-5)", .serialized)
struct PrefixCachePins {

    static let live = ProcessInfo.processInfo.environment["BRAIN_FM_MEASURE"] == "1"

    @available(macOS 27.0, *)
    static func container() async throws -> ModelContainer {
        try await LLMModelFactory.shared.loadContainer(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: ModelConfiguration(id: "mlx-community/Qwen3-0.6B-4bit"))
    }

    @available(macOS 27.0, *)
    static func turn(_ session: ChatSession, _ prompt: String) async throws
        -> (fresh: Int, cached: Int) {
        var fresh = 0
        var cached = 0
        for try await generation in session.streamDetails(to: prompt, images: [], videos: []) {
            if case .info(let info) = generation {
                fresh = info.promptTokenCount
                cached = info.cachedPromptTokenCount
            }
        }
        return (fresh, cached)
    }

    @Test("second-turn prefill is strictly less than first-turn on a shared prefix",
          .enabled(if: live))
    func secondTurnPrefillShrinks() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        let session = ChatSession(
            container,
            generateParameters: GenerateParameters(maxTokens: 16, temperature: 0))

        // The SHAPE makes the assertion robust: turn 1's render is the
        // LONGER prompt (a padded instruction), turn 2's fresh half is the
        // 16-token-capped answer plus a short question — so fresh₂ < fresh₁
        // can only fail if the prefix reuse itself is gone.
        let t1 = try await Self.turn(
            session,
            "Here is some context to keep in mind: the workshop opens at nine, "
                + "the ledger is updated at noon, and the walk happens at dusk. "
                + "Remember this number for later: 42. Reply only 'ok'.")
        #expect(t1.fresh > 0, "turn 1 prefills the whole prompt (nothing cached yet)")

        let t2 = try await Self.turn(session, "The number? Numbers only.")
        #expect(
            t2.cached > 0,
            "turn 2's cache-served half is non-zero — the prefix was reused (fresh=\(t2.fresh), cached=\(t2.cached))")
        #expect(
            t2.fresh < t1.fresh,
            "turn 2's FRESH prefill is strictly less than turn 1's (t1=\(t1.fresh), t2=\(t2.fresh)) — a refactor that lost the prefix reuse reds here")
    }
}
