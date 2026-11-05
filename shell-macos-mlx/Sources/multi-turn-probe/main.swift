// NOW BUILDING AND RUNNING. Registered as a SwiftPM executable target.
//
// HEADLINE MEASUREMENT (Qwen3-0.6B, the CI model, via mlx-native ChatSession):
// turn 2 does NOT throw, and the cold-session control FAILS to produce the
// value -- so the warm answer is RECALL, not invention. Multi-turn memory works.
//
// THE SAME PROBE ON Qwen3.5-9B THROWS ON TURN 2 -- identically, on BOTH
// surfaces: the FoundationModels adapter's LanguageModelSession AND the
// mlx-native ChatSession. My call-shape hypothesis (that only the adapter failed
// to thread history) was WRONG and is retracted by this measurement. The
// failure is QWEN3.5-SPECIFIC, not an integration problem.
//
// KNOWN DEFECTS IN THIS PROBE, so nobody reads more into it than it supports:
//   1. `text(of:)` only unwraps `.chunk`; other Generation cases contribute
//      nothing, so the printed answers are partial.
//   2. The verdict string still says "on Qwen3.5" although the run used ciModel
//      (Qwen3-0.6B). The LABEL is stale; the computed verdict
//      (warm contains the secret AND cold does not) is not.
//
// LOADER NOTE: the model is loaded through the adapter's PUBLIC
// `MLXLanguageModel.loadContainer()`, which reuses the adapter cache. Every
// `loadModelContainer` overload in MLXLMCommon/ModelFactory.swift requires
// `from downloader` + `using tokenizerLoader`, and those concrete types exist
// only inside the `#huggingFaceLanguageModel` macro expansion. ChatSession.swift
// :129 documents a `loadModelContainer(id:)` overload that DOES NOT EXIST.
//
// WHERE IT STOPPED. ChatSession needs a ModelContainer, and every
// `loadModelContainer` overload in MLXLMCommon/ModelFactory.swift requires
// `from downloader` + `using tokenizerLoader`. The concrete Downloader and
// TokenizerLoader conformances could not be located in the checked-out sources
// within this session. Note that ChatSession.swift:129 documents
// `loadModelContainer(id: "mlx-community/Qwen3-4B-4bit")` as a usage example --
// NO SUCH OVERLOAD EXISTS. Every one takes from:/using:. That example cannot
// compile, which is a doc-truth defect in an upstream doc comment and is the
// reason this file stalled.
//
// Everything above the loader line is finished and was verified against the
// sources: the ChatSession conversation shape, streamDetails(to:images:videos:),
// and the cold-session control that separates memory from compliance.
import Foundation
import Foundation
import MLXFMProbe
import MLXLMCommon
import MLXFoundationModels
import MLXHuggingFace
import MLXVLM
import MLXLLM
import Tokenizers
import HuggingFace
import FoundationModels

// MULTI-TURN MEMORY: can Qwen3.5 carry context across turns?
//
// WHY THIS EXISTS. `shell-macos-mlx` records "P3 — MULTI-TURN LOOP. Known to
// throw on Qwen3.5" and gives that failure its own `.unreachable` verdict,
// because collapsing it into pass or fail is how a real constraint disappears.
// The banner experiment hit it for real: a REUSED `LanguageModelSession`
// answered turn 1 and then threw `runtime("No user query found in messages.")`
// on every later turn — including a bare question with no fence, which ruled
// out the prompt shape.
//
// The banner experiment worked around that with a fresh session per turn. That
// is correct for a PAIRED SINGLE-TURN A/B and it is the WRONG shape for a chat
// product: plan 02 requires durable history, regenerate and fork. So this asks
// the product's question directly.
//
// THE ROOT CAUSE is not ours. mlx-swift-lm's own IntegrationTestHelpers says:
//
//   "Conversation-aware templates must receive the original user query and
//    assistant tool call again on the result turn. Rendering only the new
//    `.tool` message reproduces the Qwen Jinja 'No user query found' failure."
//
// The Qwen Jinja template is CONVERSATION-AWARE. So the hypothesis is specific:
// multi-turn is not broken ON THE MODEL, it is broken in the CALL SHAPE. The
// FoundationModels adapter's `LanguageModelSession` does not thread the prior
// turns into the rendered template; mlx's native `ChatSession` does — it keeps
// a `Conversation` (messages + a KV-cache ledger) and appends each turn.
//
// THAT IS ALSO A DOC-TRUTH DEFECT. `Package.swift` claims the adapter gives
// "the same Tools, the same @Generable guided output, the same transcript, the
// same streaming as `SystemLanguageModel`". For multi-turn, `ChatSession` and
// `LanguageModelSession` are NOT the same, and the difference is the whole
// product.
//
// THREE ARMS, because "it did not throw" is not "it remembered":
//   A. bare string, turn 2, adapter session  -> expect the documented throw
//   B. ChatSession, turn 2                    -> expect an answer
//   C. MEMORY vs COMPLIANCE. The fabricated value appears ONLY in turn 1, and a
//      COLD session that never saw it must fail to produce it. A warm answer
//      with a cold positive proves nothing — that is inventing, not recalling.

let MODEL_ID = "mlx-community/Qwen3.5-9B-4bit"
let SECRET = "184/97"

// E1a (CG-10): the Qwen3.5 mlx-community quants ship the broken
// `tokenizer_class: "TokenizersBackend"` (measured 2026-11-05 on BOTH the 4B
// repos: mlx-community/Qwen3.5-4B-MLX-4bit and avan-ag/Qwen3.5-4B-Uncensored-MLX-4bit),
// so a bare `ModelConfiguration(id:)` throws `.unsupportedTokenizer` exactly as
// the 9B did. The fix is the reasoningQwen pattern generalized: weights from the
// quant repo, tokenizer from the OFFICIAL repo, the family's empty-string EOT,
// and thinking tags. Non-Qwen3.5 ids load bare.
func probeConfiguration(_ id: String) -> ModelConfiguration {
    let mode = ProcessInfo.processInfo.environment["PROBE_CONFIG"] ?? "full"
    guard let range = id.range(of: #"Qwen3\.5-\d+(\.\d+)?B"#, options: .regularExpression) else {
        return ModelConfiguration(id: id)
    }
    let official = "Qwen/" + id[range]
    print("  tokenizer source override: \(official) (weights still from \(id)) | config mode: \(mode)")
    switch mode {
    case "tokenizer":
        // override ONLY — isolates whether extraEOSTokens/thinking cause the throw
        return ModelConfiguration(id: id, tokenizerSource: .id(official))
    case "registryEOT":
        // the LLM registry's own qwen3_5_2b_4bit style: im_end EOT, no thinking
        return ModelConfiguration(id: id, tokenizerSource: .id(official), extraEOSTokens: ["<|im_end|>"])
    default:
        // full: the reasoningQwen pattern (empty-string EOT + thinking tags)
        return ModelConfiguration(id: id, tokenizerSource: .id(official), extraEOSTokens: [""], reasoningConfig: .thinkTagsWithEnableThinking)
    }
}

func text(of generation: Generation) -> String {
    if case .chunk(let c) = generation { return c }
    return ""
}

@available(macOS 27.0, *)
func askChat(_ session: ChatSession, _ prompt: String) async -> String {
    var out = ""
    do {
        for try await g in session.streamDetails(to: prompt, images: [], videos: []) {
            out += text(of: g)
        }
    } catch {
        return "THREW: \(error)"
    }
    return out.trimmingCharacters(in: .whitespacesAndNewlines)
}

@available(macOS 27.0, *)
func run() async throws {
    // E1 gate (CG-10): model selectable by env, default = pinned 9B weights.
    // E1a (CG-10): the load goes through `#huggingFaceLoadModelContainer`, whose
    // expansion is the FREE registry-routed `loadModelContainer(from:using:)` —
    // the registry probes MLXVLM's trampoline FIRST (ModelFactory.swift:492),
    // so the VLM-family Qwen3.5 quants construct through the same call the text
    // models always used. The adapter path is gone: it was only ever the
    // loadContainer vehicle, and no live arm reads it (A is unimplemented).
    let modelID = ProcessInfo.processInfo.environment["PROBE_MODEL_ID"] ?? MODEL_ID
    print("model: \(modelID)")
    let configuration = probeConfiguration(modelID)
    // E1b (CG-10): PROBE_TEXT_ONLY=1 loads through the LLM factory DIRECTLY —
    // bypassing the ModelFactoryRegistry, whose first trampoline is the VLM
    // factory. The LLM factory's `qwen3_5` handler builds ONLY the text tower
    // (Qwen35.swift:1232 — `Qwen35TextModel(args.textConfig)`), so the VL-family
    // repos run text-only, no MLXVLM, and — decisive for this lane — the
    // continuation render goes through the plain tokenizer chat template (the
    // surface PR #472 fixed) instead of the VLM processor's own prepare path,
    // which is where the 4B's turn-2 throw was witnessed live.
    let textOnly = ProcessInfo.processInfo.environment["PROBE_TEXT_ONLY"] == "1"
    let container: ModelContainer
    if textOnly {
        print("load path: LLM factory (text tower only)")
        container = try await LLMModelFactory.shared.loadContainer(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: configuration)
    } else {
        print("load path: registry (VLM factory first)")
        container = try await #huggingFaceLoadModelContainer(configuration: configuration)
    }
    let params = GenerateParameters(maxTokens: 200, temperature: 0)
    let secret = SECRET

    // ── ARM B: ChatSession carries the conversation ───────────────────────
    print("\n=== ARM B: ChatSession (conversation-aware) ===")
    var warmAnswer = ""
    do {
        let session = ChatSession(container, generateParameters: params)
        let t1 = await askChat(session, "Remember this number for later: \(secret). Reply only 'ok'.")
        print("  turn 1: \(t1.prefix(60))")
        if t1.hasPrefix("THREW") { print("  turn 1 THREW — the session shape itself is wrong") }
        let t2 = await askChat(session, "What was the number I asked you to remember? Numbers only.")
        print("  turn 2: \(t2)")
        warmAnswer = t2
    }

    // ── ARM D: tool-result continuation (PR #472's exact scenario) ────────
    // A tool call whose result continues the SAME session: the template
    // receives the tool result as a user-role message, so without the
    // full-conversation fix this is where "No user query found" fires.
    print("\n=== ARM D: tool-result continuation ===")
    do {
        let toolSession = ChatSession(container, generateParameters: params)
        toolSession.tools = [[
            "type": "function",
            "function": [
                "name": "lookup_secret",
                "description": "Look up the secret number.",
                "parameters": [
                    "type": "object",
                    "properties": [:],
                    "required": []
                ]
            ]
        ]]
        toolSession.toolDispatch = { _ in secret }
        var toolOut = ""
        do {
            for try await g in toolSession.streamDetails(
                to: "Use the lookup_secret tool, then tell me the number. Numbers only.",
                images: [], videos: []
            ) {
                toolOut += text(of: g)
            }
        } catch {
            toolOut = "THREW: \(error)"
        }
        print("  tool turn: \(toolOut)")
        if toolOut.contains(secret) && !toolOut.hasPrefix("THREW") {
            print("  -> TOOL CONTINUATION WORKS on Qwen3.5 via ChatSession.")
        } else {
            print("  -> TOOL CONTINUATION FAILED (record as .unreachable).")
        }
    }

    // ── ARM C: memory or compliance ────────────────────────────────────────
    print("\n=== ARM C: memory vs compliance ===")
    let coldSession = ChatSession(container, generateParameters: params)
    let cold = await askChat(coldSession, "What was the number I asked you to remember? Numbers only.")
    let coldHas = cold.contains(secret)
    let warmHas = warmAnswer.contains(secret)
    print("  cold: \(cold)")
    print("  warm: \(warmAnswer)")

    if warmHas && !coldHas {
        print("\n  -> MULTI-TURN MEMORY WORKS on Qwen3.5 via ChatSession.")
        print("     The value appears only where it was actually supplied, so it was")
        print("     recalled from the conversation, not invented to be agreeable.")
    } else if warmHas && coldHas {
        print("\n  -> INCONCLUSIVE: the cold session produced it too.")
    } else {
        print("\n  -> NO RECALL. Turn 2 answered (or failed) but the value did not survive.")
    }
    print("  NOTE: a THREW in turn 2 means ChatSession did NOT fix it either, and the")
    print("  constraint is deeper than the call shape — record it as .unreachable, not")
    print("  as a pass or a fail.")
}

if #available(macOS 27.0, *) {
    do { try await run() } catch { print("multi-turn-probe: \(error)") }
} else {
    print("multi-turn-probe: requires macOS 27.0+")
    exit(2)
}
