import Foundation
import Testing

@testable import AppleFMProbe

/// PINS THE SESSION-TYPE ROUTING (CG-10 pin M-3): every chat model id routes
/// to the session implementation that owns its statefulness — AFM models to
/// the `LanguageModelSession` path, MLX models to the native `ChatSession`
/// path. The routing is a VALUE on `ChatModelId` (`sessionKind`), so this pin
/// holds the value's output; the exhaustive switch behind it is
/// compile-checked over `allCases`, and a case added without routing fails
/// to build rather than routing by accident.
@Suite("Chat session routing")
struct ChatSessionRoutingPins {

    /// The named tiers, stated plainly so a reader does not have to infer
    /// them from the loop below. The roster is the measured-green set
    /// (E1a/E1c 2026-11-05 + the CG-11 roster amendment 2026-11-06): the
    /// 8B on the old engine; the Qwen3.5 2B, the operator's pipenetwork
    /// 9B heretic quant, the mlx-community 9B 4-bit (its exclusion
    /// RETIRED by measurement), and the operator-named avan-ag 4B
    /// uncensored on the fixed engine.
    @Test("AFM routes to LanguageModelSession; MLX tiers route to ChatSession")
    func namedTiersRouteByKind() {
        #expect(
            ChatModelId.onDeviceAFM.sessionKind == .afm,
            "the AFM tier must ride the LanguageModelSession path (its trimming and guided generation live there)"
        )
        #expect(
            ChatModelId.qwen3_8b_4bit.sessionKind == .mlx,
            "the E1-verified MLX text tier must ride the native ChatSession path"
        )
        #expect(
            ChatModelId.qwen3_5_2b.sessionKind == .mlx,
            "the E1c-verified Qwen3.5 2B text tower must ride the native ChatSession path"
        )
        #expect(
            ChatModelId.qwen3_5_9b_heretic.sessionKind == .mlx,
            "the operator's named 9B tier must ride the native ChatSession path"
        )
        #expect(
            ChatModelId.qwen3_5_9b_4bit.sessionKind == .mlx,
            "the admitted 9B 4-bit (exclusion retired by measurement) must ride the native ChatSession path"
        )
        #expect(
            ChatModelId.qwen3_5_4b_uncensored.sessionKind == .mlx,
            "the operator-named 4B uncensored must ride the native ChatSession path"
        )
    }

    /// The earned flips, held so they cannot silently revert: the
    /// measured-green tiers serve chat, `forChat` admits them, and no
    /// refusal reason rides on them. The allCases law in
    /// `qwen35CannotEnterAChatPath` already demands the XOR with
    /// `chatRefusalReason`; this pin holds the MEASURED side of it.
    @Test("the measured-green MLX tiers are admitted through forChat")
    func verifiedMLXTiersServeChat() {
        for id in [ChatModelId.qwen3_8b_4bit, .qwen3_5_2b, .qwen3_5_9b_heretic,
                   .qwen3_5_9b_4bit, .qwen3_5_4b_uncensored] {
            #expect(
                id.mayServeChat,
                "\(id.rawValue) is probe-verified — reverting its flip re-excludes a measured model"
            )
            #expect(throws: Never.self) {
                try ChatModelId.forChat(id)
            }
            #expect(
                id.chatRefusalReason == nil,
                "\(id.rawValue) is admitted and carries no refusal reason"
            )
        }
    }

    /// The law over the whole set: every id routes somewhere, every
    /// MLX-class id routes `.mlx`, and `mayServeChat` — the gate — is
    /// orthogonal to routing. The gate is vacuous over today's roster
    /// (nothing is excluded since the CG-11 amendment) but stays pinned:
    /// a future exclusion must re-populate `chatRefusalReason` in the
    /// same breath as flipping `mayServeChat`, or this reds.
    @Test("routing is exhaustive over allCases; the gate stays orthogonal")
    func routingIsExhaustiveAndGateOrthogonal() {
        for id in ChatModelId.allCases {
            switch id.sessionKind {
            case .afm:
                // The AFM path is the measured default; anything routed there
                // must be servable by construction.
                #expect(id == .onDeviceAFM, "\(id.rawValue) routed .afm — only the AFM tier may")
            case .mlx:
                #expect(
                    id != .onDeviceAFM,
                    "the AFM tier must never route to the MLX ledger"
                )
            }
            // The gate stays orthogonal to the routing value: every id is
            // either chat-capable or explains itself (the same XOR law the
            // ChatModelId pin holds, restated here so a routing change cannot
            // break it silently).
            #expect(
                id.mayServeChat != (id.chatRefusalReason != nil),
                "\(id.rawValue) is neither usable nor explained"
            )
        }
    }
}
