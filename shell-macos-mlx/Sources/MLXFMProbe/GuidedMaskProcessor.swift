// THE GUIDED MASK PROCESSOR (round 14, Arm D).
//
// The bridge between the pinned engine's grammar constraint and the
// parameters-driven ChatSession API: `GenerationComponents
// .logitProcessorFactory` hands each generation a fresh processor value,
// whose `process` applies the grammar's accepted-token bitmask to the
// logits and whose `didSample` commits the sampled token so the matcher
// advances. This is the documented integration the pinned 3.32.3 release
// added (`GrammarConstraint` + the logit-processor seam) — no upstream
// patch, no fork: the constraint is the pinned checkout's own type.
//
// THE HONESTY SHAPE (the LCD-bias caution, named where the masking
// happens): guided generation GUARANTEES SHAPE, never truth — every
// masked logit is a shape decision, and the kernel's gates still judge
// the artifact's content. The processor masks; it never scores.
//
// THE MASK MAPPING IS DELIBERATELY PLAIN: the constraint hands back an
// Int32 word array (bit i of word w = token w*32+i); the expansion to a
// boolean MLXArray runs on the CPU once per step. A measurement arm can
// afford the honest cost — the M5 wall-clock rows carry it as-is rather
// than hiding it under a fused kernel.

import Foundation
import MLX
import MLXGuidedGeneration
import MLXLMCommon

struct GuidedMaskProcessor: LogitProcessor {
    /// The constraint is a class whose internal lock serializes every
    /// matcher call; the factory hands each generation a fresh PROCESSOR
    /// value sharing the one compiled constraint — the compile (the cold
    /// cost) happened once at session construction, off the main actor.
    private let constraint: GrammarConstraint
    private let vocabSize: Int

    init(constraint: GrammarConstraint, vocabSize: Int) {
        self.constraint = constraint
        self.vocabSize = vocabSize
    }

    /// Nothing to seed from the prompt: the matcher starts from the
    /// grammar's root.
    func prompt(_ prompt: MLXArray) {}

    /// Apply the grammar's next-token bitmask: tokens the grammar does not
    /// accept fall to -Infinity before sampling.
    func process(logits: MLXArray) -> MLXArray {
        guard let result = try? constraint.computeMask(), result.needsApply else {
            return logits
        }
        var accepted = [Bool](repeating: false, count: vocabSize)
        for (wordIndex, word) in result.mask.enumerated() {
            let base = wordIndex * 32
            for bit in 0..<32 {
                let token = base + bit
                if token < vocabSize, (word >> Int32(bit)) & 1 == 1 {
                    accepted[token] = true
                }
            }
        }
        let keep = MLXArray(accepted)
        let negInf = MLXArray(-Float.infinity)
        if logits.ndim == 2 {
            return which(keep.reshaped([1, vocabSize]), logits, negInf)
        }
        return which(keep, logits, negInf)
    }

    /// Commit the sampled token: the matcher advances (a rejection leaves
    /// the matcher unchanged — the engine's own fail-closed posture; the
    /// next mask is computed from the unchanged state).
    mutating func didSample(token: MLXArray) {
        let id = token.item(Int32.self)
        _ = try? constraint.commitToken(id)
    }
}
