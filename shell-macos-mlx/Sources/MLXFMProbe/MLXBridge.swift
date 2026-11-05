import Foundation
import MLXLMCommon

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
    public static func loadContainer(tier: MLXChatTier) async throws -> ModelContainer {
        let adapter = try await MLXFMProbe.model(
            configuration: tier.configuration,
            capabilities: [.toolCalling, .guidedGeneration, .reasoning])
        return try await adapter.loadContainer()
    }

    /// Build the session a turn runs on: the store's render (system + full
    /// ordered transcript — the M-1 contract) seeds the `ChatSession`'s
    /// conversation ledger ("Prompt Re-hydration", the history-init the
    /// upstream fix rides). The `instructions` parameter of `ChatSession`
    /// stays nil BY CONSTRUCTION: the store is the single render source, and
    /// a second system-prompt channel would be two places to drift.
    ///
    /// - Parameters:
    ///   - seed: the wire's conversation (already validated by the child's
    ///     wire law). Empty is the ordinary first turn.
    ///   - maxTokens: the wire's response clamp (16…1024), the same number
    ///     the AFM arm feeds `maximumResponseTokens`.
    public static func makeSession(
        container: ModelContainer,
        instructions: String?,
        seed: [ConversationMessage],
        maxTokens: Int
    ) -> (session: ChatSession, store: ConversationStore) {
        let store = ConversationStore(entries: seed)
        let session = ChatSession(
            container,
            instructions: nil,
            history: store.chatMessages(instructions: instructions),
            generateParameters: GenerateParameters(maxTokens: maxTokens, temperature: 0))
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
    /// raw stream carrying `<think>…</think>` on this path, so the filter's
    /// suppression is what makes the answer the answer.
    public static func streamTurn(
        session: ChatSession,
        prompt: String,
        maxTokens: Int,
        onDelta: @Sendable (String) -> Void
    ) async throws -> (text: String, inputTokens: Int, outputTokens: Int) {
        // The per-turn parameters are constructed HERE for the same reason
        // `makeSession`'s are: one sampling site, one law (see the type doc).
        let parameters = GenerateParameters(maxTokens: maxTokens, temperature: 0)
        var filter = ThinkFilter()
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
/// revision, `3b339ad6`; the live multi-turn witness carried the think
/// text verbatim in both deltas and final). The bridge's answer is the
/// ANSWER, not the scratchpad, so the filter suppresses everything inside
/// an open `<think>` block — streamed, not post-hoc: deltas are the clean
/// text as it arrives.
///
/// LAWS: deterministic string processing only (no model in the gate — the
/// filter judges delimiters, never content); an unclosed `<think>` at
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

    private var phase: Phase = .answer
    /// Text seen but not yet classified — a possible partial delimiter at
    /// the tail. Emitted (or dropped) at the next ingest or at `finish`.
    private var carry: String = ""
    /// The clean answer accumulated across the whole stream.
    private var clean = ""

    public init() {}

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
