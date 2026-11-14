import Foundation
import MLXLMCommon
import MLXLLM
import MLXHuggingFace
import HuggingFace
import Testing
import Tokenizers
// Both macro expansions name modules directly (see KVSchemePins for the
// same pair the multi-turn-probe target carries).

@testable import MLXFMProbe

/// L1 (IMPROVE19): the thinking-budget seam's pins — the latency lever
/// lands env-gated OFF, and these pins hold its two contract halves:
///
/// 1. THE KNOB (hermetic): `parseThinkingBudget` is total and
///    fail-closed — a malformed `BRAIN_MLX_THINKING_BUDGET` degrades to
///    OFF with the reason named on stderr, never to a guessed budget. The
///    grammar is exactly `max=N[,minAnswer=M]`; extra fields, negatives,
///    and non-numbers are malformed, not ignorable.
/// 2. THE TIER GATE (hermetic): the budget's delimiters must come from a
///    DECLARED reasoning protocol. Every Qwen3.5 tier carries
///    `.thinkTagsWithEnableThinking` (so the budget CAN apply); the plain
///    Qwen3-8B tier declares none (so it refuses BY NAME — the S0 law).
///    If a future tier edit drops the reasoning config, a fast tier
///    silently loses the budget seam — this pin reds before that rots.
/// 3. THE APPLY CORE (live, `BRAIN_FM_MEASURE=1`, the CI 0.6B): the
///    session-wrapper shape composes (`session.components` grows the
///    budget's processor chain beside any existing factory), and the
///    headroom law fails closed at generation start — a clamp that cannot
///    fit budget + Unicode completion + transition + answer reserve is
///    refused BY NAME before prefill (the P-1 shape, witnessed through
///    OUR seam, not upstream's).
@Suite("Thinking budget pins (L1)", .serialized)
struct ThinkingBudgetPins {

    static let live = ProcessInfo.processInfo.environment["BRAIN_FM_MEASURE"] == "1"

    // ── 1. THE KNOB ──────────────────────────────────────────────────────

    @Test("parse: max only (minAnswer defaults to 1)")
    func parseMaxOnly() {
        guard #available(macOS 27.0, *) else { return }
        let parsed = MLXBridge.parseThinkingBudget("max=120")
        #expect(parsed?.maximumTokenCount == 120)
        #expect(parsed?.minimumAnswerTokenCount == 1)
        // The transition is the S0 decision: the documented conservative
        // opt-in, baked into every parsed config.
        #expect(parsed?.transitionOverride == .immediate)
    }

    @Test("parse: max + minAnswer")
    func parseMaxAndMin() {
        guard #available(macOS 27.0, *) else { return }
        let parsed = MLXBridge.parseThinkingBudget("max=120,minAnswer=64")
        #expect(parsed?.maximumTokenCount == 120)
        #expect(parsed?.minimumAnswerTokenCount == 64)
    }

    @Test("parse: whitespace around fields tolerated")
    func parseWhitespace() {
        guard #available(macOS 27.0, *) else { return }
        let parsed = MLXBridge.parseThinkingBudget(" max=90 , minAnswer=32 ")
        #expect(parsed?.maximumTokenCount == 90)
        #expect(parsed?.minimumAnswerTokenCount == 32)
    }

    @Test("parse: zero budget is legal (close reasoning immediately)")
    func parseZero() {
        guard #available(macOS 27.0, *) else { return }
        let parsed = MLXBridge.parseThinkingBudget("max=0")
        #expect(parsed?.maximumTokenCount == 0)
    }

    @Test("parse: malformed forms are nil — the OFF posture")
    func parseMalformed() {
        guard #available(macOS 27.0, *) else { return }
        for raw in [
            "",              // empty
            "120",           // no max= prefix
            "max=",          // no number
            "max=abc",       // non-numeric
            "max=-5",        // negative
            "minAnswer=10",  // missing the required max
            "max=10,answer=5",  // wrong field name
            "max=10,minAnswer=-1",  // negative reserve
            "max=10,minAnswer=10,extra=1",  // extra components
        ] {
            #expect(
                MLXBridge.parseThinkingBudget(raw) == nil,
                "the malformed knob '\(raw)' must parse to nil (OFF), never a guessed budget")
        }
    }

    @Test("parse: nil input (env unset) is nil")
    func parseNil() {
        guard #available(macOS 27.0, *) else { return }
        #expect(MLXBridge.parseThinkingBudget(nil) == nil)
    }

    /// The read-once static is the parser applied to the live env value —
    /// the wiring shape itself, pinned so a future edit cannot fork the
    /// static from the pinned grammar.
    @Test("the read-once static equals the parser on the live env value")
    func staticConsistency() {
        guard #available(macOS 27.0, *) else { return }
        let raw = ProcessInfo.processInfo.environment["BRAIN_MLX_THINKING_BUDGET"]
        #expect(MLXBridge.thinkingBudgetOverride == MLXBridge.parseThinkingBudget(raw))
    }

    /// OFF at rest: the suite runs knob-unset by convention — the seam is
    /// inert without the env, the S1 acceptance's own clause.
    @Test("OFF by default: unset env means a nil override")
    func offByDefault() {
        guard #available(macOS 27.0, *) else { return }
        if ProcessInfo.processInfo.environment["BRAIN_MLX_THINKING_BUDGET"] == nil {
            #expect(MLXBridge.thinkingBudgetOverride == nil)
        }
    }

    // ── 2. THE TIER GATE ─────────────────────────────────────────────────

    /// Every fast tier (the round's S2 set: 2B / 4B-unc / 9B-heretic, plus
    /// the roster's other Qwen3.5 members) declares a reasoning protocol —
    /// the data condition the apply seam's refusal path rests on.
    @Test("every Qwen3.5 tier declares a reasoning protocol")
    func fastTiersDeclareProtocols() {
        guard #available(macOS 27.0, *) else { return }
        for tier in [
            MLXChatTier.qwen3_5_2b_4bit,
            MLXChatTier.qwen3_5_4b_uncensored_4bit,
            MLXChatTier.qwen3_5_4b_huihui_4bit,
            MLXChatTier.qwen3_5_9b_heretic_5bit,
            MLXChatTier.qwen3_5_9b_heretic_4bit,
            MLXChatTier.qwen3_5_9b_4bit,
        ] {
            #expect(
                tier.configuration.reasoningConfig != nil,
                "\(tier) lost its reasoning config — the budget would refuse it BY NAME")
        }
    }

    /// The plain Qwen3-8B tier declares none (no reasoningConfig argument
    /// in its configuration) — the named-refusal arm's data condition.
    @Test("the plain Qwen3-8B tier declares no protocol (refuses BY NAME)")
    func plainTierRefuses() {
        guard #available(macOS 27.0, *) else { return }
        #expect(MLXChatTier.qwen3_8b_4bit.configuration.reasoningConfig == nil)
    }

    // ── 3. THE APPLY CORE (live) ─────────────────────────────────────────

    /// The CI 0.6B through the LLM-factory path, carrying a DECLARED
    /// think-tags protocol for the pin (the budget needs delimiters to
    /// compile; the 0.6B's bare config has none of its own).
    @available(macOS 27.0, *)
    static func container() async throws -> ModelContainer {
        try await LLMModelFactory.shared.loadContainer(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: ModelConfiguration(
                id: "mlx-community/Qwen3-0.6B-4bit",
                reasoningConfig: .thinkTagsWithEnableThinking))
    }

    /// The apply core composes: after the call the session's components
    /// carry the budget's factory chain (fresh per generation — the
    /// engine's own contract) and the parameter validator rides the
    /// session.
    @Test("applyThinkingBudget composes into session.components", .enabled(if: live))
    func applyComposes() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        let session = ChatSession(
            container, generateParameters: GenerateParameters(maxTokens: 64, temperature: 0))
        #expect(session.components.logitProcessorFactory == nil, "precondition: no factory before the budget")
        try await MLXBridge.applyThinkingBudget(
            session: session,
            budget: try ThinkingBudgetConfiguration(
                maximumTokenCount: 40, transitionOverride: .immediate),
            reasoning: .thinkTagsWithEnableThinking,
            container: container)
        #expect(
            session.components.logitProcessorFactory != nil,
            "the budget's processor chain must ride session.components after the apply")
    }

    /// The headroom law, witnessed through OUR seam: a turn whose clamp
    /// cannot fit budget + Unicode completion + transition + answer
    /// reserve is refused BY NAME at generation start, BEFORE prefill —
    /// the P-1 shape on the budget path.
    @Test("an undersized clamp refuses at generation start", .enabled(if: live))
    func undersizedClampRefuses() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        let session = ChatSession(
            container, generateParameters: GenerateParameters(maxTokens: 1, temperature: 0))
        try await MLXBridge.applyThinkingBudget(
            session: session,
            budget: try ThinkingBudgetConfiguration(
                maximumTokenCount: 40, transitionOverride: .immediate),
            reasoning: .thinkTagsWithEnableThinking,
            container: container)
        do {
            _ = try await session.respond(to: "hello")
            Issue.record(
                "an undersized clamp was ACCEPTED with the budget armed — the headroom law is gone: required exceeds maxTokens=1")
        } catch let error as ThinkingBudgetError {
            guard case .insufficientGenerationTokenLimit = error else {
                Issue.record("refused, but by the wrong error: \(error)")
                return
            }
        }
    }

    /// The declared-protocol law, witnessed through OUR seam: a nil
    /// reasoning config refuses BY NAME — the delimiters come from a
    /// declared protocol, never a guess.
    @Test("a nil reasoning protocol refuses BY NAME", .enabled(if: live))
    func nilProtocolRefuses() async throws {
        guard #available(macOS 27.0, *) else { return }
        let container = try await Self.container()
        let session = ChatSession(
            container, generateParameters: GenerateParameters(maxTokens: 64, temperature: 0))
        do {
            try await MLXBridge.applyThinkingBudget(
                session: session,
                budget: try ThinkingBudgetConfiguration(
                    maximumTokenCount: 40, transitionOverride: .immediate),
                reasoning: nil,
                container: container)
            Issue.record("a nil reasoning protocol was ACCEPTED — the declared-protocol law is gone")
        } catch let error as MLXBridge.ThinkingBudgetRefusal {
            #expect(
                error.reason.contains("declared"),
                "the refusal must name the declared-protocol law: \(error.reason)")
        }
    }
}
