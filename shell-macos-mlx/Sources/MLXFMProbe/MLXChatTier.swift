import Foundation
import MLXLMCommon

/// The MLX chat tiers the bridge can serve, keyed by the model id that
/// crosses the bridge wire (`{"model": "…"}` on `turn` and `prewarm`,
/// CG-10 E3). The roster is the lane's MEASURED-GREEN set (amended
/// 2026-11-05, E1a/E1c): the operator picks a tier via `BRAIN_CHAT_MODEL`;
/// the kernel transports the string verbatim and THIS type owns the
/// weights. An id outside this map refuses by name at first turn — never a
/// silent fallback to another model.
///
/// ## THE ENGINE FLOOR (a pin, not a preference)
///
/// Every Qwen3.5 tier here requires **swift-jinja ≥ 3.0.0**: on the 1.3.0
/// engine (what `swift-transformers 0.1.x` resolved until the CG-10 bump)
/// the slice evaluator returns EMPTY for any implicit-range slice with a
/// step — `messages[::-1]` on a 2+ element list — and Qwen3.5's template
/// reverse-scans the conversation through exactly that construct, so every
/// turn after the first raised `No user query found in messages.`
/// regardless of factory path or config (measured by the `template-debug`
/// bisect, E1a). Upstream fixed it in swift-jinja #99 + #101 + the Jinja2
/// conformance suite #95 (3.0.0, 2026-10-27); the bump is OURS alone in
/// `Package.swift` (swift-transformers 1.4.0 → swift-jinja 3.0.0). Pinned
/// by the engine-floor test.
///
/// ## THE QWEN3.5 CONFIG PATTERN (E1a-measured, `probeConfiguration`'s
/// "full" mode)
///
/// The mlx-community Qwen3.5 quants (and the pipenetwork quant) all ship
/// the broken `tokenizer_class: "TokenizersBackend"` — not a tokenizer
/// class but the loader's own backend enum name — so loading throws
/// `.unsupportedTokenizer` naming the bad value. The fix is the
/// reasoningQwen pattern generalized: weights from the quant repo,
/// tokenizer from the OFFICIAL repo (`Qwen/Qwen3.5-<size>`), the family's
/// empty-string EOT marker, and thinking tags. (The VLM story was wrong and
/// is retracted: the family is `Qwen3_5ForConditionalGeneration` at the
/// source, but MLXLLM's `qwen3_5` handler builds ONLY the text tower —
/// `Qwen35.swift` — so every tier here runs TEXT-ONLY, no MLXVLM.)
///
/// ## THE LOAD PATH
///
/// `MLXBridge.loadContainer` constructs through `LLMModelFactory` DIRECTLY
/// (the text tower): the ModelFactoryRegistry probes the VLM factory first,
/// and the VLM processor's own prepare path is where the Qwen3.5 turn-2
/// throw was witnessed live — the text-tower path keeps the continuation
/// render on the plain tokenizer chat template, the surface PR #472 fixed.
public enum MLXChatTier: Sendable, Equatable {
    /// The E1-verified text tier (`mlx-community/Qwen3-8B-4bit`): arms
    /// B/C/D green on the OLD engine — the Qwen3 template lacks the raise,
    /// so it never depended on the slice. Plain configuration, the
    /// E1-measured shape.
    case qwen3_8b_4bit

    /// `mlx-community/Qwen3.5-2B-4bit` — Qwen3.5 text tower, E1c green on
    /// the FIXED engine (turn 2 recalls, tool continuation answers, cold
    /// control declines honestly).
    case qwen3_5_2b_4bit

    /// `pipenetwork/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-MLX-5bit`
    /// — the OPERATOR'S named pick for the agentic tier (7.07 GB, 5-bit,
    /// 3-link provenance Qwen → nightmedia DS9 → pipenetwork). E1c green:
    /// `MULTI-TURN MEMORY WORKS on Qwen3.5 via ChatSession` — turn 2
    /// recalls, ARM D uses the dispatched tool result, cold control clean.
    ///
    /// POSTURE NOTE (a named operator act, recorded 2026-11-05): the
    /// earlier "uncensored refused (governed production)" refusal governs
    /// the PRODUCT tier roster, and the operator accepted this tier's
    /// provenance and fine-tune posture for the LOCAL, self-hosted MLX
    /// path. The refusal law is unchanged for anything kernel- or
    /// cloud-side.
    case qwen3_5_9b_heretic_5bit

    /// Maps a wire model id to its tier. Nil is a NAMED refusal upstream
    /// (`bad_request` naming the id) — never a fallback.
    public static func tier(modelId: String) -> MLXChatTier? {
        switch modelId {
        case "qwen3-8b-4bit": .qwen3_8b_4bit
        case "qwen3.5-2b-4bit": .qwen3_5_2b_4bit
        case "qwen3.5-9b-heretic-5bit": .qwen3_5_9b_heretic_5bit
        default: nil
        }
    }

    /// The load configuration — each tier's E-measured shape (see the type
    /// doc for the engine floor and the Qwen3.5 pattern).
    public var configuration: ModelConfiguration {
        switch self {
        case .qwen3_8b_4bit:
            ModelConfiguration(id: "mlx-community/Qwen3-8B-4bit")
        case .qwen3_5_2b_4bit:
            ModelConfiguration(
                id: "mlx-community/Qwen3.5-2B-4bit",
                tokenizerSource: .id("Qwen/Qwen3.5-2B"),
                extraEOSTokens: [""],
                reasoningConfig: .thinkTagsWithEnableThinking)
        case .qwen3_5_9b_heretic_5bit:
            ModelConfiguration(
                id: "pipenetwork/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-MLX-5bit",
                tokenizerSource: .id("Qwen/Qwen3.5-9B"),
                extraEOSTokens: [""],
                reasoningConfig: .thinkTagsWithEnableThinking)
        }
    }

    /// The weights' declared context (`text_config.max_position_embeddings`
    /// for the Qwen3.5 VLM-shell family), the ceiling the seed measurement
    /// refuses against — the MLX counterpart of the AFM arm's budget law.
    public var contextTokens: Int {
        switch self {
        case .qwen3_8b_4bit: 40_960
        case .qwen3_5_2b_4bit, .qwen3_5_9b_heretic_5bit: 262_144
        }
    }

    /// Whether this tier's chat template OPENS the think block in the
    /// prompt render, so the generated stream starts inside the reasoning
    /// and only `</think>` marks the answer (the ThinkFilter's primed
    /// mode). Qwen3.5: primed (template-debug A1 render ends
    /// `<|im_start|>assistant\n<think>`; witnessed live on the heretic —
    /// scratchpad prose with no opening tag). Qwen3: not primed (the 8B's
    /// stream carried the literal `<think>` chunks).
    public var primesThinkBlock: Bool {
        switch self {
        case .qwen3_8b_4bit: false
        case .qwen3_5_2b_4bit, .qwen3_5_9b_heretic_5bit: true
        }
    }

    /// E1 (CG-11): the tier's EVIDENCED KV compression scheme, or nil for
    /// full precision — the TurboQuant ratchet's landing spot. The default
    /// moves ONLY on the evidence file's per-scheme table (the probe
    /// battery + the answer-quality arms, run per scheme per tier): a
    /// scheme that never measured green stays nil here, and a scheme this
    /// table moves ON must have its row in that table. Measurement runs
    /// use `MLXBridge.kvSchemeOverride` (BRAIN_MLX_KV_SCHEME) instead of
    /// editing this property — the override is the instrument, this is the
    /// posture.
    ///
    /// Session-stable BY CONSTRUCTION: the scheme resolves once, at
    /// `makeSession`, and never varies across a session's turns — the
    /// package's ChatSession rebuilds the KV cache from the retained
    /// transcript whenever the resolved cache plan changes
    /// (`stored.main.plan != kvCachePlan`), so a mid-session scheme change
    /// would silently discard the prefix cache E4 pins.
    public var kvScheme: String? {
        nil
    }

    /// E2: the tier's evidenced spec-decode posture — OFF everywhere until
    /// the acceptance table moves it. The gate is
    /// `MLXBridge.specDecodeEvidenced` (≥ 40% acceptance, measured off the
    /// draft-model path's native `SpeculativeDecodingTelemetry`); a tier
    /// whose measured rate sits below the gate stays `false` here and that
    /// disable is a recorded decision, not a failure. Measurement runs use
    /// `MLXBridge.specDecodeOverride` (BRAIN_MLX_SPEC_DECODE) instead of
    /// editing this property.
    public var specDecode: Bool {
        false
    }
}
