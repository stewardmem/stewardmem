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
    /// them from the loop below.
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
            ChatModelId.qwen3_5.sessionKind == .mlx,
            "the excluded 9B is still an MLX-class model — its routing does not depend on its refusal (forChat gates that separately)"
        )
    }

    /// The E1-earned flip, held so it cannot silently revert: the 8B serves
    /// chat (arms B/C/D green, 2026-11-04, re-verified on the rebased branch
    /// 2026-11-05), `forChat` admits it, and no refusal reason rides on it.
    /// The allCases law in `qwen35CannotEnterAChatPath` already demands the
    /// XOR with `chatRefusalReason`; this pin holds the MEASURED side of it.
    @Test("the verified MLX text tier is admitted through forChat")
    func verifiedMLXTierServesChat() {
        #expect(
            ChatModelId.qwen3_8b_4bit.mayServeChat,
            "the probe-verified tier must stay chat-capable — reverting this re-excludes a measured model"
        )
        #expect(throws: Never.self) {
            try ChatModelId.forChat(.qwen3_8b_4bit)
        }
        #expect(
            ChatModelId.qwen3_8b_4bit.chatRefusalReason == nil,
            "an admitted model carries no refusal reason"
        )
    }

    /// The law over the whole set: every id routes somewhere, every
    /// MLX-class id (both Qwen cases) routes `.mlx`, and `mayServeChat` — the
    /// gate — is orthogonal to routing (qwen3_5 routes `.mlx` while forChat
    /// refuses it; the loader gap is not a routing question).
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
