import Foundation
import HuggingFace
import MLXHuggingFace
import MLXLLM
import MLXLMCommon
import Tokenizers

/// The bridge-facing MLX chat path (CG-10 E3): container load, seeded
/// session construction, and the per-turn stream, in ONE place — the MLX
/// package owns its generation parameters for the same reason the shell's
/// greedy lock owns `GenerationOptions` there: the sampling vocabulary
/// cannot drift in a tree that is not scanned for it. This file IS that
/// tree's sampling site, and its law is the MLX counterpart of the shell's:
/// **`temperature: 0` is the greedy mode for an MLX generate** — argmax at
/// every step, the setting every measured claim in this lane ran with (the
/// multi-turn probe's `GenerateParameters(maxTokens: 200, temperature: 0)`).
/// Any future knob lands here, named, beside this comment.
///
/// The 27 gate is the adapter surface's, not MLX's: `MLXFMProbe.model` and
/// the capability vocabulary are `@available(macOS 27.0, *)` (the
/// FoundationModels adapter types in their signatures), so the bridge path
/// through this facade inherits the gate. `ChatSession` itself is not
/// gated — but the child's `.mlx` arms guard 27 anyway, so a pre-27 host
/// answers `model_unavailable` BY NAME rather than failing mid-tier.
@available(macOS 27.0, *)
public enum MLXBridge {

    /// Load the tier's container — the cold path the bridge's `prewarm` op
    /// exists to overlap with the grounding window. Weight download, mmap
    /// and metal compile all happen here, once; the caller caches the result
    /// for the process lifetime (the MLX counterpart of the shell's
    /// `BudgetCache`).
    ///
    /// The factory is `LLMModelFactory` DIRECTLY, not the
    /// ModelFactoryRegistry: the registry probes the VLM factory first, and
    /// the VLM processor's own prepare path is where the Qwen3.5 turn-2
    /// throw was witnessed live (E1a). The LLM factory's `qwen3_5` handler
    /// builds only the text tower, so every tier — including the
    /// VLM-shell-architected Qwen3.5 family — runs TEXT-ONLY, and the
    /// continuation render stays on the plain tokenizer chat template, the
    /// surface PR #472 fixed. This also drops the adapter hop (`MLXFMProbe.
    /// model` → `loadContainer`): it was only ever the load vehicle, and no
    /// live arm reads the adapter.
    public static func loadContainer(tier: MLXChatTier) async throws -> ModelContainer {
        try await LLMModelFactory.shared.loadContainer(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: tier.configuration)
    }

    /// E1 (CG-11): the KV-scheme OVERRIDE for measurement runs — the probe's
    /// `PROBE_KV_SCHEME` counterpart on the serving path, read ONCE per
    /// process so a mid-process change cannot rebuild sessions' caches. The
    /// tier's evidenced posture stays in `MLXChatTier.kvScheme`; the
    /// override is how the per-scheme arms run WITHOUT editing the roster.
    /// An unrecognized string is not validated here on purpose: it reaches
    /// the engine and fails closed at generation start
    /// (`unsupportedLegacyScheme`, P-1's pinned upstream gate) — a typo in
    /// the environment refuses the turn BY NAME, never a silent fall-back
    /// to full precision.
    public static let kvSchemeOverride: String? =
        ProcessInfo.processInfo.environment["BRAIN_MLX_KV_SCHEME"]

    /// The resolved KV scheme for a tier: the measurement override first,
    /// then the tier's evidenced posture. The ONE resolution site — the
    /// sampling law of this file covers the KV posture the same way it
    /// covers `temperature`.
    public static func resolvedKVScheme(for tier: MLXChatTier) -> String? {
        kvSchemeOverride ?? tier.kvScheme
    }

    /// Build the session a turn runs on: the store's render (system + full
    /// ordered transcript — the M-1 contract) seeds the `ChatSession`'s
    /// conversation ledger ("Prompt Re-hydration", the history-init the
    /// upstream fix rides). The `instructions` parameter of `ChatSession`
    /// stays nil BY CONSTRUCTION: the store is the single render source, and
    /// a second system-prompt channel would be two places to drift.
    ///
    /// - Parameters:
    ///   - tier: the session's chat tier — the KV posture's owner
    ///     (`resolvedKVScheme(for:)`) and, with E2, the spec-decode roster.
    ///   - seed: the wire's conversation (already validated by the child's
    ///     wire law). Empty is the ordinary first turn.
    ///   - maxTokens: the wire's response clamp (16…1024), the same number
    ///     the AFM arm feeds `maximumResponseTokens`.
    public static func makeSession(
        container: ModelContainer,
        tier: MLXChatTier,
        instructions: String?,
        seed: [ConversationMessage],
        maxTokens: Int
    ) -> (session: ChatSession, store: ConversationStore) {
        let store = ConversationStore(entries: seed)
        let session = ChatSession(
            container,
            instructions: nil,
            history: store.chatMessages(instructions: instructions),
            generateParameters: GenerateParameters(
                maxTokens: maxTokens,
                kvScheme: resolvedKVScheme(for: tier),
                temperature: 0))
        return (session, store)
    }

    /// The seed measurement — the MLX counterpart of the AFM arm's
    /// `tokenCount` law. Measured with the LOADED model's own tokenizer over
    /// the seed's texts; not the template render (the chat wrapper adds a
    /// bounded per-message overhead), which is why the refusal ceiling is
    /// the tier's context with the AFM arm's margin: a seed that would not
    /// fit the window is refused BY NAME before a session carries it.
    public static func seedTokenCount(
        _ seed: [ConversationMessage], container: ModelContainer
    ) async -> Int {
        var total = 0
        for message in seed {
            total += await container.encode(message.text).count
        }
        return total
    }

    /// One turn: append the user prompt, stream the answer, return the
    /// settled text and the measured token counts.
    ///
    /// Stream semantics DIFFER from the AFM arm's and the caller must know:
    /// FoundationModels snapshots are CUMULATIVE (the child suffix-emits),
    /// while `Generation.chunk` here is a per-fragment stream — each chunk
    /// APPENDS, so `onDelta` receives fragments, never cumulative text, and
    /// there is no replace path. `.info` arrives once at completion with the
    /// token counts; `.promptTokenCount + .cachedPromptTokenCount` is the
    /// true input (the cache-served half counted where it belongs).
    ///
    /// Tool-call cases contribute nothing: the chat tier configures no
    /// tools, and a tool-call-shaped model output without a dispatcher is
    /// rejected by the parser upstream — it is not answer text.
    ///
    /// Cancellation: the stream is left on `Task.isCancelled`, mirroring the
    /// AFM arm's cancel-kills-the-TURN law; the caller then appends nothing
    /// to the store (the append-at-final law).
    ///
    /// The deltas and the settled text are the filter's CLEAN output — the
    /// measured witness (CG-10 E3, the live multi-turn exchange) showed the
    /// raw stream carrying reasoning text on this path, so the filter's
    /// suppression is what makes the answer the answer.
    ///
    /// - Parameter primedThinkBlock: TRUE for tiers whose chat template
    ///   OPENS the think block in the prompt render (Qwen3.5 — the
    ///   template-debug A1 tail ends `<|im_start|>assistant\n<think>`), so
    ///   the generated stream starts INSIDE the reasoning: it begins with
    ///   scratchpad text and only `</think>` marks the answer (found live
    ///   on the heretic 9B — its reasoning arrived with no opening tag).
    ///   FALSE for tiers where the model emits the opening tag itself
    ///   (Qwen3 — the 8B's stream carried literal `<think>` chunks).
    public static func streamTurn(
        session: ChatSession,
        prompt: String,
        maxTokens: Int,
        primedThinkBlock: Bool,
        onDelta: @Sendable (String) -> Void
    ) async throws -> (text: String, inputTokens: Int, outputTokens: Int) {
        // The per-turn clamp is constructed HERE for the same reason
        // `makeSession`'s parameters are: one sampling site, one law (see
        // the type doc). It REPLACES the session's maxTokens only — the KV
        // posture (`kvScheme`) rides the session's own parameters untouched,
        // and maxTokens is NOT part of the resolved KV plan (the plan reads
        // maxKVSize/kvBits/kvScheme/kvGroupSize/quantizedKVStart —
        // KVCachePlan.swift), so this mutation can never rebuild the
        // session's cache mid-conversation.
        //
        // FOUND IN CG-11 E1 GROUND: the landed shape constructed these
        // per-turn parameters and never passed them anywhere —
        // `ChatSession.streamDetails` takes no parameters and reads the
        // SESSION's `generateParameters` — so the wire's response clamp was
        // silently inert on this tier (the AFM arm honors the same number at
        // `maximumResponseTokens:`). The assignment below makes the clamp
        // real: the dead local is gone, the documented 16…1024 contract
        // holds on both tiers.
        var parameters = session.generateParameters
        parameters.maxTokens = maxTokens
        session.generateParameters = parameters
        var filter = ThinkFilter(primedThinkBlock: primedThinkBlock)
        var inputTokens = 0
        var outputTokens = 0
        for try await generation in session.streamDetails(
            to: prompt, images: [], videos: []
        ) {
            if Task.isCancelled { break }
            switch generation {
            case .chunk(let fragment):
                guard !fragment.isEmpty else { continue }
                let clean = filter.ingest(fragment)
                if !clean.isEmpty {
                    onDelta(clean)
                }
            case .info(let info):
                inputTokens = info.promptTokenCount + info.cachedPromptTokenCount
                outputTokens = info.generationTokenCount
            case .toolCall, .rejectedToolCall:
                continue
            }
        }
        let full = filter.finish()
        return (full, inputTokens, outputTokens)
    }
}

/// The deterministic `<think>`-block filter for the bridge's MLX stream.
///
/// WHY THIS EXISTS, and why it is not model-side cleanup: on the
/// `ChatSession` path the reasoning tokens ride the raw token stream — the
/// upstream `ReasoningTokenCollector` splits reasoning only inside the
/// MLXFoundationModels adapter machinery (`MLXLanguageModel.swift:1854`),
/// never in `ChatSession.streamDetails` (verified in-tree at the pinned
/// revision, `3b339ad6`; the live multi-turn witness carried the reasoning
/// verbatim in both deltas and final). The bridge's answer is the ANSWER,
/// not the scratchpad, so the filter suppresses everything inside a think
/// block — streamed, not post-hoc: deltas are the clean text as it
/// arrives.
///
/// ## THE PRIMED CASE (Qwen3.5)
///
/// Qwen3.5's chat template OPENS the think block in the prompt itself —
/// the template-debug A1 render ends `<|im_start|>assistant\n<think>` — so
/// the generated stream starts INSIDE the reasoning and only `</think>`
/// marks the answer (found live on the heretic 9B: scratchpad prose with no
/// opening tag, then the tag, then the answer). For those tiers pass
/// `primedThinkBlock: true` and the filter STARTS in the suppressed phase:
/// everything until `</think>` is scratchpad, by the template's own
/// contract. Qwen3 tiers emit the opening tag themselves; they run
/// unprimed and the tag drives the state machine.
///
/// LAWS: deterministic string processing only (no model in the gate — the
/// filter judges delimiters, never content); an unclosed think block at
/// stream end suppresses to the end (a truncated think block is not
/// answer); text outside any block passes through byte-identical. Tag
/// fragments spanning chunk boundaries are held back and resolved on the
/// next fragment (the carry), so a delimiter split across chunks cannot
/// leak.
public struct ThinkFilter: Sendable {
    private enum Phase: Sendable {
        case answer
        case thinking
    }

    private var phase: Phase
    /// Text seen but not yet classified — a possible partial delimiter at
    /// the tail. Emitted (or dropped) at the next ingest or at `finish`.
    private var carry: String = ""
    /// The clean answer accumulated across the whole stream.
    private var clean = ""

    /// - Parameter primedThinkBlock: the template opens the think block in
    ///   the prompt (Qwen3.5), so the stream starts in the suppressed
    ///   phase. See the type doc.
    public init(primedThinkBlock: Bool = false) {
        phase = primedThinkBlock ? .thinking : .answer
    }

    /// Ingest one raw fragment; return the clean text it makes visible.
    public mutating func ingest(_ fragment: String) -> String {
        var buffer = carry + fragment
        carry = ""
        var out = ""

        while !buffer.isEmpty {
            switch phase {
            case .answer:
                if let range = buffer.range(of: Self.openTag) {
                    out += buffer[..<range.lowerBound]
                    buffer = String(buffer[range.upperBound...])
                    phase = .thinking
                } else {
                    // Hold back a possible partial `<think>` at the tail;
                    // everything before it is clean and final.
                    let hold = Self.partialSuffixLength(of: buffer, prefixOf: Self.openTag)
                    let emitCount = buffer.count - hold
                    if emitCount > 0 {
                        out += buffer.prefix(emitCount)
                        buffer = String(buffer.suffix(hold))
                    }
                    carry = buffer
                    buffer = ""
                }
            case .thinking:
                if let range = buffer.range(of: Self.closeTag) {
                    buffer = String(buffer[range.upperBound...])
                    phase = .answer
                } else {
                    // Inside a think block everything is suppressed; hold
                    // back a possible partial `</think>`.
                    let hold = Self.partialSuffixLength(of: buffer, prefixOf: Self.closeTag)
                    buffer = String(buffer.suffix(hold))
                    carry = buffer
                    buffer = ""
                }
            }
        }

        clean += out
        return out
    }

    /// The settled clean text. Called at stream end: any carried text is
    /// resolved by the law (clean in `.answer`, suppressed in `.thinking` —
    /// an unclosed think block is never answer).
    public mutating func finish() -> String {
        if phase == .answer, !carry.isEmpty {
            clean += carry
            carry = ""
        }
        return clean
    }

    private static let openTag = "<think>"
    private static let closeTag = "</think>"

    /// The length of the longest PROPER suffix of `text` that is a prefix
    /// of `tag` — the part that might complete into the tag on the next
    /// fragment. Full occurrences are handled by the range search above.
    static func partialSuffixLength(of text: String, prefixOf tag: String) -> Int {
        let max = min(tag.count - 1, text.count)
        if max <= 0 { return 0 }
        for length in stride(from: max, through: 1, by: -1) {
            let suffix = text.suffix(length)
            if tag.hasPrefix(suffix) {
                return length
            }
        }
        return 0
    }
}
