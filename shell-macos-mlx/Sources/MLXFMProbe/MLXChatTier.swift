import Foundation
import MLXLMCommon

/// The MLX chat tiers the bridge can serve, keyed by the model id that
/// crosses the bridge wire (`{"model": "qwen3-8b-4bit"}` on `turn` and
/// `prewarm`, CG-10 E3).
///
/// The split of ownership: the shell's `ChatModelId` NAMES a tier and gates
/// it through `forChat` (the exclusion law lives there); THIS type owns the
/// weights — the repo id, the load configuration, and the declared context.
/// A `ChatModelId` case that forgets its row here fails at first turn with a
/// named refusal, never a silent fallback to another model.
///
/// Everything in `qwen3_8b_4bit`'s row is measured, not assumed:
/// - `mlx-community/Qwen3-8B-4bit` loads through the MLXLLM stack
///   (`model_type: qwen3`, `Qwen3ForCausalLM` — a TEXT architecture, unlike
///   the 9B's VLM pack that refuses to load; CG-10 E1).
/// - The multi-turn probe ran arms B/C/D green on these weights — turn 2
///   recalls, the tool-result continuation answers, the cold control fails
///   to recall — re-verified on the CG-10 branch after its rebase
///   (2026-11-05).
/// - `max_position_embeddings: 40960` in the weights' own config.json — the
///   seed-measurement ceiling reads the same number.
/// - The configuration is PLAIN — the E1-measured shape. One override was
///   tried and REVERTED: an explicit `reasoningConfig: .thinkTagsWithEnableThinking`
///   was meant to strip `<think>` blocks from the stream, but (a) the
///   factory honors an explicit config INSTEAD of the model's own
///   declaration (`QwenReasoningProtocol.qwen3` — `isSpecialToken: true`,
///   budget transitions, tool-call implicit ends — details the simple
///   preset gets wrong), and (b) the strip does not exist on this path
///   anyway: the upstream `ReasoningTokenCollector` runs only in the
///   MLXFoundationModels adapter machinery (`MLXLanguageModel.swift`),
///   never in `ChatSession`'s stream. The bridge's answer text is cleaned
///   by OUR deterministic filter instead — `MLXBridge.ThinkFilter`, pinned.
public enum MLXChatTier: Sendable, Equatable {
    /// The E1-verified text tier. The only MLX model that has completed a
    /// multi-turn conversation on the development machine.
    case qwen3_8b_4bit

    /// Maps a wire model id to its tier. Nil is a NAMED refusal upstream
    /// (`bad_request` naming the id) — never a fallback.
    public static func tier(modelId: String) -> MLXChatTier? {
        switch modelId {
        case "qwen3-8b-4bit": .qwen3_8b_4bit
        default: nil
        }
    }

    /// The load configuration — PLAIN, the E1-measured shape (see the type
    /// doc for the reasoning-config override that was tried and reverted).
    public var configuration: ModelConfiguration {
        switch self {
        case .qwen3_8b_4bit:
            ModelConfiguration(id: "mlx-community/Qwen3-8B-4bit")
        }
    }

    /// The weights' declared context (`max_position_embeddings`), the
    /// ceiling the seed measurement refuses against — the MLX counterpart
    /// of the AFM arm's budget law.
    public var contextTokens: Int {
        switch self {
        case .qwen3_8b_4bit: 40_960
        }
    }
}
