import Foundation
import Testing

@testable import MLXFMProbe

/// PINS THE THINK FILTER — the deterministic `<think>`-block suppressor the
/// MLX bridge's clean answer rides on. The live witness (CG-10 E3) showed
/// the raw ChatSession stream carrying the scratchpad verbatim; these pins
/// hold the filter's contract on the shapes that matter, including the
/// fragment-boundary cases a token stream produces routinely.
@Suite("Think filter — the clean-answer contract")
struct ThinkFilterPins {

    @Test("a plain answer passes through byte-identical")
    func plainAnswerPassesThrough() {
        var filter = ThinkFilter()
        #expect(filter.ingest("You drink green tea") == "You drink green tea")
        #expect(filter.ingest(" in the morning.") == " in the morning.")
        #expect(filter.finish() == "You drink green tea in the morning.")
    }

    @Test("a complete think block is suppressed from deltas and final")
    func completeBlockIsSuppressed() {
        var filter = ThinkFilter()
        var emitted = ""
        emitted += filter.ingest("<think>\nOkay, the user is asking.\n</think>\n\n")
        emitted += filter.ingest("You drink green tea in the morning.")
        #expect(emitted == "\n\nYou drink green tea in the morning.")
        #expect(
            filter.finish() == "\n\nYou drink green tea in the morning.",
            "the settled text carries no think content"
        )
    }

    @Test("a delimiter split across fragments does not leak")
    func splitDelimiterDoesNotLeak() {
        var filter = ThinkFilter()
        var emitted = ""
        // The open tag arrives in pieces.
        emitted += filter.ingest("<th")
        #expect(emitted.isEmpty, "a possible tag prefix is held back, not emitted")
        emitted += filter.ingest("ink>\nsecret reasoning\n</thi")
        #expect(emitted.isEmpty, "inside the block nothing emits")
        emitted += filter.ingest("nk>\nAnswer.")
        #expect(emitted == "\nAnswer.")
        #expect(filter.finish() == "\nAnswer.")
    }

    @Test("an unclosed think block suppresses to the end of the stream")
    func unclosedBlockSuppressesToEnd() {
        var filter = ThinkFilter()
        var emitted = ""
        emitted += filter.ingest("<think>\ntruncated reasoning, stream cut")
        #expect(emitted.isEmpty)
        #expect(
            filter.finish().isEmpty,
            "a truncated think block is never answer"
        )
    }

    @Test("think-looking text that never opens a block is kept")
    func thinkLikeTextWithoutOpenBlockIsKept() {
        var filter = ThinkFilter()
        // Angle brackets and partial words that never form the delimiter
        // are content, not structure.
        _ = filter.ingest("if x < then y <thinks it holds")
        #expect(
            filter.finish() == "if x < then y <thinks it holds",
            "without the full open delimiter, nothing is suppressed"
        )
    }
}
