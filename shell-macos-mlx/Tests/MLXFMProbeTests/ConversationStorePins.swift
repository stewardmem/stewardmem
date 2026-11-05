import Foundation
import Testing

import MLXLMCommon
@testable import MLXFMProbe

/// PINS THE RENDER CONTRACT the MLX chat tier's template correctness hangs
/// on. The Qwen chat template raises `No user query found in messages.` when
/// the rendered list carries no non-tool user message — the failure CG-10
/// exists to make unreachable. `ConversationStore.chatMessages` is the one
/// function whose output the template sees, so these pins hold its output,
/// not its code.
@Suite("Conversation store — the render contract")
struct ConversationStorePins {

    /// M-1 (CG-10). The scenario upstream's PR #472 exists for: a continuation
    /// whose newest traffic is tool-shaped. A render that kept only the new
    /// suffix — the pre-fix call shape — hands the template a list with no
    /// non-tool user message and reproduces the raise verbatim. The store must
    /// render the ORIGINAL query, ahead of the tool entry, every time.
    @Test("tool-only continuation renders WITH the original user query")
    func toolOnlyContinuationRendersWithTheOriginalQuery() {
        let store = ConversationStore(entries: [
            ConversationMessage(role: .user, text: "Remember this number for later: 184/97."),
            ConversationMessage(role: .assistant, text: "ok"),
            ConversationMessage(role: .tool, text: "184/97"),
        ])

        let rendered = store.chatMessages(instructions: nil)

        // The template's contract: at least one non-tool user message, and it
        // is the ORIGINAL query — not a paraphrase the store invented.
        let queryIndex = rendered.firstIndex {
            $0.role == .user && $0.content.contains("184/97")
        }
        let renderedList = rendered
            .map { "\($0.role.rawValue): \($0.content)" }
            .joined(separator: " | ")
        #expect(
            queryIndex != nil,
            "a tool-only continuation lost the original user query — the Qwen template raises on this list: \(renderedList)"
        )
        // The tool entry rides AFTER the query it answers; a reordered list
        // would answer a question the model cannot see.
        let toolIndex = rendered.firstIndex { $0.role == .tool }
        #expect(
            queryIndex != nil && toolIndex != nil && queryIndex! < toolIndex!,
            "the tool result must not precede the user query it continues"
        )
    }

    /// The instructions render FIRST and exactly once — the same law the AFM
    /// kind enforces through the session's Profile. A system message dropped
    /// from a seeded session would answer ungrounded; two would double-speak.
    @Test("instructions render first, exactly once, ahead of the transcript")
    func instructionsRenderFirstExactlyOnce() {
        var store = ConversationStore()
        store.seed([
            ConversationMessage(role: .user, text: "q1"),
            ConversationMessage(role: .assistant, text: "a1"),
        ])

        let rendered = store.chatMessages(instructions: "Answer from the evidence or abstain.")

        #expect(rendered.first?.role == .system, "instructions must ride first")
        let systemCount = rendered.filter { $0.role == .system }.count
        #expect(
            systemCount == 1,
            "exactly one system message — \(rendered.count) rendered entries carried \(systemCount)"
        )
        #expect(rendered.count == 3, "system + both transcript entries expected")

        // An empty instruction is not a message: rendering a placeholder the
        // template treats as content would speak twice.
        let bare = store.chatMessages(instructions: "")
        #expect(
            !bare.contains { $0.role == .system },
            "an empty instruction must render nothing, not a placeholder"
        )
    }

    /// The cache laws the authority chain rests on: seed REPLACES (a reused
    /// id must not accumulate a previous incarnation's entries), and a
    /// completed turn appends its PAIR at the final.
    @Test("seed replaces; a completed turn appends its user+assistant pair")
    func seedReplacesAndAppendPairs() {
        var store = ConversationStore(
            entries: [ConversationMessage(role: .user, text: "old incarnation")]
        )
        store.seed([
            ConversationMessage(role: .user, text: "q1"),
            ConversationMessage(role: .assistant, text: "a1"),
        ])
        #expect(store.entries.map(\.text) == ["q1", "a1"], "seed must replace, not append")

        store.append(user: "q2", assistant: "a2")
        #expect(store.entries.map(\.text) == ["q1", "a1", "q2", "a2"])
        #expect(store.entries[2].role == .user && store.entries[3].role == .assistant)
    }
}
