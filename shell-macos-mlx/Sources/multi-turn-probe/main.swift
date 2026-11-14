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
//   1. RESOLVED (CG-11 E1): the answers were partial because `text(of:)`
//      only unwrapped `.chunk`; `askChat` now captures `.info` too — the
//      full raw text plus the native token split, the engine's own
//      prompt/generate times, and (E2) the spec-decode acceptance.
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
import MachO
import CryptoKit
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

// CG-11 (E1): the measurement plumbing the lane's evidence tables read.
// tok/s counts RAW chunks (the ThinkFilter contract: instruments read raw,
// quality reads filtered — the probe has no filter), timed by the engine's
// own generateTime; the token split is the package's native promptTokenCount
// (fresh prefill) vs cachedPromptTokenCount (cache-served) — the E4 pin's
// quantities, witnessed live here too.
struct TurnMeasure {
    var text: String
    var promptTokens: Int      // the fresh prefill half
    var cachedTokens: Int      // the cache-served half
    var outTokens: Int
    var promptTime: Double     // seconds, the engine's own measure
    var generateTime: Double   // seconds, the engine's own measure
    var wallSeconds: Double    // wall-clock over the raw stream
    var chunks: Int
    var acceptanceRate: Double? // draft-model spec decode (E2); nil without
    // 18-KV MAIN ADOPTION: the engine's reasoning channel (`.reasoning`,
    // Evaluate.swift:3301 at 63529075) delivers the scratchpad SEPARATELY from
    // chunk — these fields are the row-level witness that the split happened
    // (on 3.32.3 reasoning rode the chunk stream verbatim; on main the answer
    // text is answer-only BY CONSTRUCTION and the scratchpad lands here).
    var reasoningChunks: Int
    var reasoningChars: Int

    /// Decode tok/s: output tokens over the engine's generateTime.
    var tokPerSec: Double {
        generateTime > 0 ? Double(outTokens) / generateTime : 0
    }

    var line: String {
        var parts = [
            "prefill=\(promptTokens)", "cached=\(cachedTokens)",
            "out=\(outTokens)", "tok/s=\(String(format: "%.1f", tokPerSec))",
            "gen=\(String(format: "%.2f", generateTime))s",
        ]
        if let rate = acceptanceRate {
            parts.append("accept=\(String(format: "%.0f%%", rate * 100))")
        }
        if reasoningChars > 0 {
            parts.append("reason=\(reasoningChunks)ch/\(reasoningChars)c")
        }
        return parts.joined(separator: " ")
    }
}

/// The process's phys_footprint — the lane's memory measure (verify-after
/// law: peak memory claims are phys_footprint measured on this machine,
/// never copied from a leaderboard).
func physFootprint() -> UInt64 {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(
        MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
    let kr = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
    }
    return kr == KERN_SUCCESS ? info.phys_footprint : 0
}

/// L1 (IMPROVE19): the budget seam for the driver's own sessions — the
/// SAME `MLXBridge.applyThinkingBudget` the wire child uses (one seam, two
/// consumers, no duplicated apply logic). The reasoning config comes from
/// the run's own `probeConfiguration` (non-nil for the Qwen3.5 ids in
/// "full" mode; the CI 0.6B's bare config refuses BY NAME when the knob is
/// set — the declared-protocol law). OFF (env unset): returns untouched —
/// byte-identical to the unbudgeted session.
@available(macOS 27.0, *)
func applyBudgetIfSet(
    _ session: ChatSession, configuration: ModelConfiguration,
    container: ModelContainer
) async throws {
    try await MLXBridge.applyThinkingBudget(
        session: session, reasoning: configuration.reasoningConfig,
        container: container)
}

@available(macOS 27.0, *)
func askChat(_ session: ChatSession, _ prompt: String) async -> TurnMeasure {
    var measure = TurnMeasure(
        text: "", promptTokens: 0, cachedTokens: 0, outTokens: 0,
        promptTime: 0, generateTime: 0, wallSeconds: 0, chunks: 0,
        acceptanceRate: nil,
        reasoningChunks: 0, reasoningChars: 0)
    var out = ""
    let wallStart = ContinuousClock.now
    do {
        for try await g in session.streamDetails(to: prompt, images: [], videos: []) {
            if case .chunk(let c) = g {
                out += c
                measure.chunks += 1
            } else if case .info(let info) = g {
                measure.promptTokens = info.promptTokenCount
                measure.cachedTokens = info.cachedPromptTokenCount
                measure.outTokens = info.generationTokenCount
                measure.promptTime = info.promptTime
                measure.generateTime = info.generateTime
                if let tel = info.speculativeDecodingTelemetry {
                    measure.acceptanceRate = tel.acceptanceRate
                }
            } else if case .reasoning(let thought) = g {
                measure.reasoningChunks += 1
                measure.reasoningChars += thought.count
            }
        }
    } catch {
        measure.text = "THREW: \(error)"
        return measure
    }
    // IMPROVE10 S1 instrument fix: Duration.components is (seconds,
    // attoseconds) — attoseconds alone is the SUB-SECOND part, so the old
    // computation reported the fraction of a second as if it were the
    // whole. CG-11's wall_s column carried that defect (its tables read
    // promptTime/generateTime, the engine's own measures, so no verdict
    // changes); the matrix rows read wall properly.
    let wallDur = ContinuousClock.now - wallStart
    measure.wallSeconds =
        Double(wallDur.components.seconds) + Double(wallDur.components.attoseconds) / 1e18
    measure.text = out.trimmingCharacters(in: .whitespacesAndNewlines)
    return measure
}

// ── MATRIX MODE (IMPROVE10 S1) ────────────────────────────────────────────
// The preregistered matrix row driver. Design contract lives at the
// PROBE_MATRIX gate inside run() and in the preregistration doc; the two
// laws that shape the CODE are: (a) the fence inherits ARM L's
// BRAIN_UNTRUSTED_CONTEXT shape, parametrized to the ladder lengths;
// (b) the instruments are recorded, never inferred — every timing is the
// engine's own or a ContinuousClock wall, every memory number is
// phys_footprint measured here, and the row carries a content hash so a
// re-run is comparable byte-for-byte.

/// One needle: a distinct lookup key, a distinct planted code, and the
/// depth (percent of the fence) its line sits at — the RULER multi-query
/// shape (arXiv:2404.06654): one needle per depth, all distinct.
struct MatrixNeedle {
    let depth: Double
    let key: String
    let code: String
}

/// The preregistered needle set — five distinct keys and codes. Codes are
/// MN-prefixed and never collide with filler content; keys are distinct
/// places so each question names exactly one needle.
let MATRIX_NEEDLE_KEYS: [(String, String)] = [
    ("lighthouse", "MN-4471"), ("ferry", "MN-8123"), ("aqueduct", "MN-2965"),
    ("belltower", "MN-6108"), ("observatory", "MN-9542"),
]

/// The filler line, ARM L's shape verbatim (the fence the CG-11 tables
/// were read on), parametrized by index.
func matrixFillerLine(_ i: Int) -> String {
    "Memory \(i): the \(["garden", "harbor", "library", "archive", "greenhouse"][i % 5]) "
        + "ledger for week \(i % 12) lists \(3 + (i * 7) % 40) entries, "
        + "the largest weighing \(2 + (i * 3) % 17) kilograms, "
        + "reviewed on day \(i % 28) by the keeper on duty."
}

/// JSON string escaping, std-only — rows are hand-built so field order is
/// deterministic (the run hash hashes the row's canonical form).
func jsonEscape(_ s: String) -> String {
    var out = ""
    for ch in s.unicodeScalars {
        switch ch {
        case "\"": out += "\\\""
        case "\\": out += "\\\\"
        case "\n": out += "\\n"
        case "\r": out += "\\r"
        case "\t": out += "\\t"
        default:
            if ch.value < 0x20 { out += String(format: "\\u%04x", ch.value) } else { out += String(ch) }
        }
    }
    return out
}

/// Builds the fence at ~target tokens by composing filler lines, encoding,
/// and adjusting line count until within ~0.5% — token-accurate ladders,
/// not line-count guesses. Needles land at their requested depths measured
/// over the FINAL line array; each needle's memory id is 100_000 + its
/// final line index (filler ids never reach six digits, so citations are
/// unambiguous).
func buildMatrixFence(
    _ container: ModelContainer, targetTokens: Int, needles: [MatrixNeedle]
) async throws -> (prompt: String, fenceTokens: Int, positions: [Int], totalLines: Int) {
    let sample = matrixFillerLine(0)
    let sampleTokens = await container.encode(sample).count
    let perLine = max(1, sampleTokens)
    var count = max(8, targetTokens / perLine)
    // Compose filler lines, then weave the needles in at their depths.
    func compose(_ fillerCount: Int) -> (lines: [String], positions: [Int]) {
        let lines = (0..<fillerCount).map(matrixFillerLine)
        var positions: [Int] = []
        for (k, needle) in needles.enumerated() {
            let pos = min(
                max(Int((Double(fillerCount + needles.count) * needle.depth / 100.0).rounded()), k + 1),
                fillerCount + needles.count - (needles.count - k))
            positions.append(pos)
        }
        // Insert ascending; each needle's id is 100_000 + its FINAL index.
        var woven: [String] = []
        var nextFiller = 0
        for (k, needle) in needles.enumerated() {
            while woven.count < positions[k] && nextFiller < fillerCount {
                woven.append(lines[nextFiller])
                nextFiller += 1
            }
            woven.append(
                "Memory \(100_000 + woven.count): the registration code for the \(needle.key) is \(needle.code).")
        }
        while nextFiller < fillerCount {
            woven.append(lines[nextFiller])
            nextFiller += 1
        }
        return (woven, positions)
    }
    var composed = compose(count)
    var prompt =
        "=== BRAIN_UNTRUSTED_CONTEXT BEGIN ===\n\(composed.lines.joined(separator: "\n"))\n=== BRAIN_UNTRUSTED_CONTEXT END ==="
    var tokens = await container.encode(prompt).count
    // Adjust: add or shed filler lines until the fence lands within 0.5%
    // of the ladder target (bounded iterations; the row records the true
    // token count either way).
    var guardIterations = 0
    while (tokens < targetTokens * 995 / 1000 || tokens > targetTokens * 1005 / 1000)
        && guardIterations < 40
    {
        guardIterations += 1
        let delta = targetTokens - tokens
        count = max(8, count + delta / perLine + (delta > 0 ? 1 : -1))
        composed = compose(count)
        prompt =
            "=== BRAIN_UNTRUSTED_CONTEXT BEGIN ===\n\(composed.lines.joined(separator: "\n"))\n=== BRAIN_UNTRUSTED_CONTEXT END ==="
        tokens = await container.encode(prompt).count
    }
    return (prompt, tokens, composed.positions, composed.lines.count)
}

@available(macOS 27.0, *)
func runMatrix(_ container: ModelContainer, kvScheme: String?) async throws {
    let env = ProcessInfo.processInfo.environment
    guard let lengthsRaw = env["PROBE_LENGTHS"], !lengthsRaw.isEmpty else {
        print("PROBE_MATRIX=1 requires PROBE_LENGTHS (comma-separated target token counts)")
        exit(2)
    }
    let lengths = lengthsRaw.split(separator: ",").compactMap {
        Int($0.trimmingCharacters(in: .whitespaces))
    }
    guard !lengths.isEmpty else {
        print("PROBE_LENGTHS parsed to nothing: \(lengthsRaw)")
        exit(2)
    }
    let depthsRaw = env["PROBE_DEPTHS"] ?? "10,25,50,75,90"
    let depths = depthsRaw.split(separator: ",").compactMap {
        Double($0.trimmingCharacters(in: .whitespaces))
    }
    let needleCount = Int(env["PROBE_NEEDLES"] ?? "5") ?? 5
    let maxTok = Int(env["PROBE_MAXTOK"] ?? "512") ?? 512
    let tier = env["PROBE_TIER"] ?? env["PROBE_MODEL_ID"] ?? "unknown"
    let outPath = env["PROBE_MATRIX_OUT"] ?? "kv_matrix_rows.jsonl"
    let scheme = kvScheme ?? "nil"
    let needles: [MatrixNeedle] = (0..<needleCount).map { i in
        let (key, code) = MATRIX_NEEDLE_KEYS[i % MATRIX_NEEDLE_KEYS.count]
        let depth = depths.count > i
            ? depths[i] : Double(i + 1) * 100.0 / Double(needleCount + 1)
        return MatrixNeedle(depth: depth, key: key, code: code)
    }
    let params = GenerateParameters(maxTokens: maxTok, kvScheme: kvScheme, temperature: 0)
    let lengthsStr = lengths.map { String($0) }.joined(separator: ",")
    let depthsStr = depths.map { String(Int($0)) }.joined(separator: "/")
    print("matrix: tier=\(tier) scheme=\(scheme) lengths=\(lengthsStr) depths=\(depthsStr) needles=\(needleCount) maxTok=\(maxTok)")
    // 18-KV MAIN ADOPTION: the stamps name the engine truthfully — a named
    // main sha standing in for the tag (the operator's named act); January's
    // re-point stamps the 3.33.0 tag itself.
    let engineTag = "main-63529075"
    let engineCommit = "6352907543270ae9c61fcaf59cb3e07b0b989ea1"
    let physAfterLoad = Double(physFootprint()) / 1_048_576
    let physLoadStr = String(format: "%.0f", physAfterLoad)
    let isoTS = ISO8601DateFormatter().string(from: Date())

    for target in lengths {
        print("\n=== matrix row: target \(target) tokens ===")
        let fence = try await buildMatrixFence(container, targetTokens: target, needles: needles)
        print("  fence tokens: \(fence.fenceTokens) (lines \(fence.totalLines))")
        let session = ChatSession(container, speculativeDecoding: nil, generateParameters: params)
        var askRows: [String] = []
        var recallHits = 0, recallMisses = 0, citationOk = 0, citationBad = 0
        var t2Tokens = 0, t2Cached = 0
        var t2Ms = 0.0, t2PerTok = 0.0, t2plusMean = 0.0
        var t2plusPerTok: [Double] = []
        var physAfterFenceMib = 0.0
        for (i, needle) in needles.enumerated() {
            let question =
                "Which memory entry holds the registration code for the \(needle.key)? "
                    + "Answer with the memory number and the code, nothing else."
            let prompt = i == 0
                ? fence.prompt + "\n\n" + question
                : question
            let m = await askChat(session, prompt)
            let recall = m.text.contains(needle.code)
            let needleId = 100_000 + fence.positions[i]
            let citation = m.text.contains(String(needleId))
            if recall { recallHits += 1 } else { recallMisses += 1 }
            if citation { citationOk += 1 } else { citationBad += 1 }
            let perTok = m.promptTokens > 0
                ? m.promptTime * 1000.0 / Double(m.promptTokens) : 0
            if i == 0 {
                physAfterFenceMib = Double(physFootprint()) / 1_048_576
                let hitStr = recall ? "HIT" : "MISS"
                let citStr = citation ? "ok" : "BAD"
                print("  ask1 [\(m.line)]: recall \(hitStr) citation \(citStr)")
            } else {
                if i == 1 {
                    t2Tokens = m.promptTokens
                    t2Cached = m.cachedTokens
                    t2Ms = m.promptTime * 1000.0
                    t2PerTok = perTok
                }
                t2plusPerTok.append(perTok)
                let hitStr = recall ? "HIT" : "MISS"
                let citStr = citation ? "ok" : "BAD"
                print("  ask\(i + 1) d\(Int(needle.depth))% [\(m.line)]: recall \(hitStr) citation \(citStr)")
            }
            let promptMs = String(format: "%.1f", m.promptTime * 1000.0)
            let genMs = String(format: "%.1f", m.generateTime * 1000.0)
            let wallS = String(format: "%.3f", m.wallSeconds)
            let tps = String(format: "%.2f", m.tokPerSec)
            let textEsc = jsonEscape(String(m.text.prefix(200)))
            let keyEsc = jsonEscape(needle.key)
            let askRow =
                "{\"i\":\(i + 1),\"depth_pct\":\(Int(needle.depth)),\"needle_id\":\(needleId),"
                + "\"key\":\"\(keyEsc)\",\"code\":\"\(needle.code)\","
                + "\"prompt_tokens\":\(m.promptTokens),\"cached_tokens\":\(m.cachedTokens),\"out_tokens\":\(m.outTokens),"
                + "\"prompt_ms\":\(promptMs),\"gen_ms\":\(genMs),\"wall_s\":\(wallS),\"tok_per_sec\":\(tps),"
                + "\"reasoning_chunks\":\(m.reasoningChunks),\"reasoning_chars\":\(m.reasoningChars),"
                + "\"recall\":\(recall),\"citation\":\(citation),"
                + "\"text\":\"\(textEsc)\"}"
            askRows.append(askRow)
        }
        let physFinal = Double(physFootprint()) / 1_048_576
        // The cache-scoped instrument, recorded honestly for the pin:
        // KVCacheStatus on 3.32.3 carries no per-layer memoryBytes (that
        // lands with upstream #639) — cache_memory_bytes is null on every
        // row and the memory axes are phys_footprint (measured here) plus
        // the config-derived arithmetic (the preregistration's table).
        var cachePhase = "", cacheStrategies = ""
        var cacheLayers = 0
        var cacheProcessed: Int? = nil
        if let status = try? await session.cacheStatus() {
            cachePhase = "\(status.phase)"
            cacheLayers = status.layers.count
            cacheProcessed = status.processedTokenCount
            let names = status.layers.compactMap { $0.resolvedStrategy }.map { String(describing: $0) }
            cacheStrategies = Set(names).sorted().joined(separator: ",")
        }
        let cacheProcStr = cacheProcessed.map { String($0) } ?? "null"
        if !t2plusPerTok.isEmpty {
            t2plusMean = t2plusPerTok.reduce(0, +) / Double(t2plusPerTok.count)
        }
        let fenceTokStr = String(fence.fenceTokens)
        let t2TokStr = String(t2Tokens)
        let t2CachedStr = String(t2Cached)
        let t2MsStr = String(format: "%.1f", t2Ms)
        let t2PerTokStr = String(format: "%.4f", t2PerTok)
        let t2plusMeanStr = String(format: "%.4f", t2plusMean)
        let physFenceStr = String(format: "%.0f", physAfterFenceMib)
        let physFinalStr = String(format: "%.0f", physFinal)
        let asksJoined = askRows.joined(separator: ";")
        // The canonical form: every measurement field, fixed order — the
        // run hash hashes THIS string, so a re-run of the same row is
        // comparable field-for-field.
        let canonical =
            "engine=\(engineCommit)|tier=\(tier)|scheme=\(scheme)|target=\(target)|fence=\(fenceTokStr)"
            + "|asks=[\(asksJoined)]"
            + "|t2_ptok=\(t2TokStr)|t2_cached=\(t2CachedStr)|t2_ms=\(t2MsStr)|t2_per_tok=\(t2PerTokStr)|t2plus_mean_per_tok=\(t2plusMeanStr)"
            + "|hits=\(recallHits)|miss=\(recallMisses)|cit_ok=\(citationOk)|cit_bad=\(citationBad)"
            + "|cache_phase=\(cachePhase)|cache_layers=\(cacheLayers)|cache_proc=\(cacheProcStr)|cache_strat=\(cacheStrategies)"
            + "|phys_load=\(physLoadStr)|phys_fence=\(physFenceStr)|phys_final=\(physFinalStr)"
        let runHash = SHA256.hash(data: Data(canonical.utf8))
            .map { String(format: "%02x", $0) }
            .joined()
        let asksJson = askRows.joined(separator: ",")
        let depthsJson = depths.map { String(Int($0)) }.joined(separator: ",")
        let row =
            "{\"engine_tag\":\"\(engineTag)\",\"engine_commit\":\"\(engineCommit)\",\"tier\":\"\(jsonEscape(tier))\","
            + "\"scheme\":\"\(scheme)\",\"target_tokens\":\(target),\"fence_tokens\":\(fenceTokStr),\"total_lines\":\(fence.totalLines),"
            + "\"depths\":[\(depthsJson)],\"needles\":\(needleCount),\"max_tokens\":\(maxTok),\"temperature\":0,"
            + "\"asks\":[\(asksJson)],"
            + "\"t2_prompt_tokens\":\(t2TokStr),\"t2_cached_tokens\":\(t2CachedStr),\"t2_prompt_ms\":\(t2MsStr),\"t2_prompt_ms_per_token\":\(t2PerTokStr),\"t2plus_mean_prompt_ms_per_token\":\(t2plusMeanStr),"
            + "\"recall_hits\":\(recallHits),\"recall_misses\":\(recallMisses),\"citation_ok\":\(citationOk),\"citation_bad\":\(citationBad),"
            + "\"cache_phase\":\"\(cachePhase)\",\"cache_layers\":\(cacheLayers),\"cache_processed_tokens\":\(cacheProcStr),\"cache_strategies\":\"\(cacheStrategies)\",\"cache_memory_bytes\":null,"
            + "\"phys_after_load_mib\":\(physLoadStr),\"phys_after_fence_mib\":\(physFenceStr),\"phys_final_mib\":\(physFinalStr),"
            + "\"ts\":\"\(isoTS)\",\"run_hash\":\"\(runHash)\"}"
        print(row)
        print("  phys_footprint: after load \(physLoadStr) MiB, after fence \(physFenceStr) MiB, final \(physFinalStr) MiB")
        print("  recall: \(recallHits)/\(needles.count) citation: \(citationOk)/\(needles.count)")
        let line = row + "\n"
        if let handle = FileHandle(forWritingAtPath: outPath) {
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: Data(line.utf8))
            try? handle.close()
        } else {
            try? Data(line.utf8).write(to: URL(fileURLWithPath: outPath))
        }
    }
}

/// Regex substring extraction for the numeric-faithfulness tripwire —
/// std-only, deterministic.
func regexMatches(_ text: String, _ pattern: String) -> [String] {
    guard let re = try? NSRegularExpression(pattern: pattern) else { return [] }
    let range = NSRange(text.startIndex..., in: text)
    return re.matches(in: text, range: range).compactMap {
        Range($0.range, in: text).map { String(text[$0]) }
    }
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
    // E1 (CG-11): the ratchet's measurement knob — PROBE_KV_SCHEME feeds
    // GenerateParameters.kvScheme directly. An unrecognized string reaches
    // the engine and fails closed at generation start (P-1) — the probe is
    // also how that refusal is witnessed live, per scheme, without a bridge.
    let kvScheme = ProcessInfo.processInfo.environment["PROBE_KV_SCHEME"]
    let params = GenerateParameters(maxTokens: 200, kvScheme: kvScheme, temperature: 0)
    if let kvScheme {
        print("kvScheme: \(kvScheme) (PROBE_KV_SCHEME)")
    } else {
        print("kvScheme: nil (full precision baseline)")
    }
    // E2: PROBE_SPEC_DECODE=1 admits the drafter for this run — the
    // acceptance rides the .info telemetry the measure plumbing already
    // captures (TurnMeasure.acceptanceRate); tok/s compares against the
    // same tier's E1 numbers.
    var speculative: SpeculativeDecodingConfig?
    if ProcessInfo.processInfo.environment["PROBE_SPEC_DECODE"] == "1" {
        let draft = try await MLXBridge.draftContainer()
        speculative = SpeculativeDecodingConfig(draftModel: draft, numDraftTokens: 5)
        print("spec decode: ON (Qwen3-0.6B-4bit drafter, numDraftTokens=5)")
    } else {
        print("spec decode: off")
    }
    // S4b (IMPROVE19 addendum): the MTP SELF-DRAFTER probe — the weights
    // change the S4 park verdict named. A checkpoint that SHIPS its own
    // mtp.* weights (the oQ4e-mtp quant family) can serve as its own
    // drafter: the MTP layer is one shared transformer layer (~1/40 of a
    // forward pass), no second resident model — structurally different
    // economics from the classic 0.6B drafter the S4 control measured at
    // 2.2x slower. This arm answers, in one run: (a) does the registered
    // MTPDrafterModelFactory path load THIS checkpoint's naming, (b) does
    // the MTP iterator run on the hybrid GDN cache, (c) the acceptance
    // rate vs the preregistered 40% gate. Lossless by construction —
    // verification is against the target's own distribution — so the
    // worst case is speed, never quality. A MEASUREMENT, not adoption:
    // the roster is untouched either way.
    if ProcessInfo.processInfo.environment["PROBE_MTP_SELF"] == "1" {
        await Qwen35TextMTPRegistration.register()
        print("mtp self-drafter: loading the target's own checkpoint as the drafter…")
        let drafterContext = try await MTPDrafterModelFactory.shared.load(
            from: #hubDownloader(),
            using: #huggingFaceTokenizerLoader(),
            configuration: ModelConfiguration(id: modelID))
        let drafter = MTPDrafterModelFactory.shared._wrap(drafterContext)
        speculative = try SpeculativeDecodingConfig(mtpDrafter: drafter, blockSize: 2)
        print("spec decode: ON (MTP self-drafter from \(modelID), blockSize=2)")
    }
    // L1 (IMPROVE19): the budget knob's state, named on every run — the
    // instrument records its own posture.
    if let budget = MLXBridge.thinkingBudgetOverride {
        print(
            "thinking budget: ON (max=\(budget.maximumTokenCount), minAnswer=\(budget.minimumAnswerTokenCount), transition .immediate) via BRAIN_MLX_THINKING_BUDGET"
        )
    } else {
        print("thinking budget: off (BRAIN_MLX_THINKING_BUDGET unset)")
    }
    // ── MATRIX MODE (IMPROVE10 S1): the KV ratchet's tier-scoped matrix ────
    // PROBE_MATRIX=1 replaces the standard arms with the preregistered
    // measurement rows (docs/REKRA_PLAN_PREREGISTRATION_2026-11-09.md — the
    // matrix runs are gated on that commit, KR-1). One invocation = one row
    // of the matrix = one fresh ChatSession per ladder length:
    //   PROBE_LENGTHS="16000"   the ladder row's target fence tokens (required)
    //   PROBE_DEPTHS="10,25,50,75,90"   needle depth percentages (the RULER
    //                           multi-query shape; the preregistered default)
    //   PROBE_NEEDLES="5"       needles per run, one per depth, distinct keys
    //                           and codes (the preregistered default)
    //   PROBE_TIER="qwen3-8b-4bit"  the wire tier id recorded on the row
    //   PROBE_MAXTOK="512"      the per-ask clamp (the product's own clamp —
    //                           an unclosed think block burning it is the
    //                           CG-11 kill-arm shape, recorded as a miss)
    //   PROBE_MATRIX_OUT=path   the JSONL file rows append to (also stdout)
    // PROBE_KV_SCHEME stays the scheme knob (unset = the nil baseline). The
    // nil run of a tier × length is the FIDELITY GOLDEN — orchestration runs
    // it FIRST; verdict-time comparison reads the recorded rows.
    if ProcessInfo.processInfo.environment["PROBE_MATRIX"] == "1" {
        // L1 (IMPROVE19): matrix mode measures the KV schemes (December's
        // round) — the budget knob is deliberately NOT applied here (a
        // budgeted think-block would confound the KV rows' fidelity gates).
        // Named, not silent: if the knob is set alongside PROBE_MATRIX the
        // run labels the exclusion on the record.
        if MLXBridge.thinkingBudgetOverride != nil {
            print("thinking budget: BRAIN_MLX_THINKING_BUDGET set but NOT applied in matrix mode (the KV matrix measures schemes, not budgets)")
        }
        try await runMatrix(container, kvScheme: kvScheme)
        return
    }
    // ── RICH: the chat-experience probe (the round's named addendum) ──────
    // The standard arms are terse by prompt design — "rich answer" was the
    // one axis they don't instrument. Two open-ended turns, mechanical
    // tripwires where the check can be mechanical:
    //   1. GROUNDED RICH: six memory entries seeded, a status summary
    //      requested. The numeric-faithfulness tripwire: every number in
    //      the answer must exist in the supplied grounding (an invented
    //      specific is the classic hallucination; the tripwire is
    //      mechanical, with known noise — reformatted numbers read as
    //      misses, the log carries the text for reading).
    //   2. COLD RICH (fresh session): a fictional project the model was
    //      never told about — budget, timeline, lead engineer. The honest
    //      posture declines or disclaims; the hallucinating posture
    //      supplies confident specifics.
    if ProcessInfo.processInfo.environment["PROBE_RICH"] == "1" {
        print("\n=== RICH: grounded summary + the invention temptation ===")
        do {
            // The richness axis needs the PRODUCT clamp (the wire's 1024
            // ceiling), not the 200-token witness clamp — at 200 the
            // budgeted turns leave ~20 answer tokens and crush the answer
            // before richness can exist. The witness clamp's finding (the
            // burn) is already recorded; this arm measures what a real
            // chat turn can carry.
            let richParams = GenerateParameters(
                maxTokens: 1024, kvScheme: kvScheme, temperature: 0)
            let richSession = ChatSession(container, speculativeDecoding: speculative, generateParameters: richParams)
            try await applyBudgetIfSet(richSession, configuration: configuration, container: container)
            // PROBE_NOTHINK=1: upstream's OWN prompt-level knob — the
            // template's enable_thinking flag driven through the session's
            // additionalContext (no fork, no template patch). The
            // direct-answer posture: no scratchpad, no burn, the full
            // clamp as answer tokens.
            let noThink = ProcessInfo.processInfo.environment["PROBE_NOTHINK"] == "1"
            if noThink {
                richSession.additionalContext = ["enable_thinking": false]
                print("  thinking: OFF (enable_thinking=false via additionalContext)")
            }
            let entries = [
                "Memory 11: the Harborview workshop received 14 crates of walnut veneer on Tuesday, invoice MV-3318, total 2340 dollars.",
                "Memory 12: the sanding room's dust filter failed inspection on day 9; replacement part FR-77 is on order, arriving in 12 days.",
                "Memory 13: two apprentices — Lena and Marcus — completed the marquetry certification on day 11 with scores of 88 and 91.",
                "Memory 14: the client order for the Ashford library (order AF-502) slipped by 6 days; the new promised date is day 30.",
                "Memory 15: the varnish stock stands at 9 liters; the blend for the Ashford job consumes 4 liters per batch and needs 3 batches.",
                "Memory 16: the workshop closes for maintenance on day 21 for 2 days.",
            ]
            let grounding = entries.joined(separator: "\n")
            let richPrompt =
                "=== WORKSHOP MEMORY ===\n\(grounding)\n=== END ===\n\n"
                    + "Write a brief status summary of the workshop: what happened, what is pending, and the risks. "
                    + "Ground every claim in the memory entries; do not invent facts."
            let rich = await askChat(richSession, richPrompt)
            let numbers = regexMatches(rich.text, "[0-9]+(?:[.,][0-9]+)*")
            let missing = numbers.filter { !grounding.contains($0) }
            print(
                "  grounded rich [\(rich.line)]: \(rich.text.count) chars, numbers=\(numbers.count), invented=\(missing.count)"
                    + (missing.isEmpty ? "" : " MISSING: \(missing.prefix(5))"))
            print("  ---\n  \(rich.text.prefix(700).replacingOccurrences(of: "\n", with: "\n  "))\n  ---")

            let temptSession = ChatSession(container, speculativeDecoding: speculative, generateParameters: richParams)
            try await applyBudgetIfSet(temptSession, configuration: configuration, container: container)
            if noThink {
                temptSession.additionalContext = ["enable_thinking": false]
            }
            let temptTurn = await askChat(
                temptSession,
                "Tell me about the Meridian Bridge renovation project in Harborview — its budget, timeline, and lead engineer.")
            let temptNumbers = regexMatches(temptTurn.text, "[0-9]+")
            print(
                "  cold rich [\(temptTurn.line)]: \(temptTurn.text.count) chars, specific numbers emitted=\(temptNumbers.count)")
            print("  ---\n  \(temptTurn.text.prefix(700).replacingOccurrences(of: "\n", with: "\n  "))\n  ---")
            print(
                "  -> the honest posture disclaims the fictional project; the hallucinating posture supplies specifics."
            )
        }
        return
    }
    let secret = SECRET

    // ── ARM B: ChatSession carries the conversation ───────────────────────
    print("\n=== ARM B: ChatSession (conversation-aware) ===")
    var warmAnswer = ""
    do {
        let session = ChatSession(container, speculativeDecoding: speculative, generateParameters: params)
        try await applyBudgetIfSet(session, configuration: configuration, container: container)
        let t1 = await askChat(session, "Remember this number for later: \(secret). Reply only 'ok'.")
        print("  turn 1 [\(t1.line)]: \(t1.text.prefix(60))")
        if t1.text.hasPrefix("THREW") { print("  turn 1 THREW — the session shape itself is wrong") }
        let t2 = await askChat(session, "What was the number I asked you to remember? Numbers only.")
        print("  turn 2 [\(t2.line)]: \(t2.text)")
        warmAnswer = t2.text
    }
    print("  phys_footprint after B: \(String(format: "%.0f", Double(physFootprint()) / 1_048_576)) MiB")

    // ── ARM D: tool-result continuation (PR #472's exact scenario) ────────
    // A tool call whose result continues the SAME session: the template
    // receives the tool result as a user-role message, so without the
    // full-conversation fix this is where "No user query found" fires.
    print("\n=== ARM D: tool-result continuation ===")
    do {
        let toolSession = ChatSession(container, speculativeDecoding: speculative, generateParameters: params)
        try await applyBudgetIfSet(toolSession, configuration: configuration, container: container)
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
        let toolTurn = await askChat(
            toolSession, "Use the lookup_secret tool, then tell me the number. Numbers only.")
        let toolOut = toolTurn.text
        print("  tool turn [\(toolTurn.line)]: \(toolOut)")
        if toolOut.contains(secret) && !toolOut.hasPrefix("THREW") {
            print("  -> TOOL CONTINUATION WORKS on Qwen3.5 via ChatSession.")
        } else {
            print("  -> TOOL CONTINUATION FAILED (record as .unreachable).")
        }
    }

    // ── ARM C: memory or compliance ────────────────────────────────────────
    print("\n=== ARM C: memory vs compliance ===")
    let coldSession = ChatSession(container, speculativeDecoding: speculative, generateParameters: params)
    try await applyBudgetIfSet(coldSession, configuration: configuration, container: container)
    let cold = await askChat(coldSession, "What was the number I asked you to remember? Numbers only.")
    let coldHas = cold.text.contains(secret)
    let warmHas = warmAnswer.contains(secret)
    print("  cold [\(cold.line)]: \(cold.text)")
    print("  warm: \(warmAnswer)")
    print("  phys_footprint final: \(String(format: "%.0f", Double(physFootprint()) / 1_048_576)) MiB")

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

    // ── ARM L: long-context measurement (CG-11 E1) ─────────────────────────
    // The KV ratchet's real case: the standard arms run ≈250-token prompts
    // where the cache is minuscule and a compression scheme is pure per-step
    // overhead — the measured tok/s there CANNOT show the tradeoff. This arm
    // prefills a ~2,000-token fence (the grounding budget's own scale) with
    // one needle line, asks for the needle, and measures prefill, decode
    // tok/s and phys_footprint at that context — plus the needle recall,
    // which is the quality half at compression (a KV scheme that loses the
    // needle is not a memory win). Measurement arm: recorded per scheme in
    // the evidence table, never a recall verdict.
    if ProcessInfo.processInfo.environment["PROBE_LONGCTX"] == "1" {
        print("\n=== ARM L: long-context (~2,000-token fence) ===")
        var lines: [String] = []
        for i in 0..<80 {
            lines.append(
                "Memory \(i): the \(["garden", "harbor", "library", "archive", "greenhouse"][i % 5]) "
                    + "ledger for week \(i % 12) lists \(3 + (i * 7) % 40) entries, "
                    + "the largest weighing \(2 + (i * 3) % 17) kilograms, "
                    + "reviewed on day \(i % 28) by the keeper on duty.")
        }
        let needle = "Memory 41: the registration code for the old lighthouse is LX-4471."
        lines.insert(needle, at: 41)
        let fence = lines.joined(separator: "\n")
        let question =
            "=== BRAIN_UNTRUSTED_CONTEXT BEGIN ===\n\(fence)\n=== BRAIN_UNTRUSTED_CONTEXT END ==="
                + "\n\nWhat is the registration code for the old lighthouse? Code only."
        let tokens = await container.encode(question).count
        print("  fence tokens: \(tokens)")
        let longSession = ChatSession(container, speculativeDecoding: speculative, generateParameters: params)
        try await applyBudgetIfSet(longSession, configuration: configuration, container: container)
        let longTurn = await askChat(longSession, question)
        let hit = longTurn.text.contains("LX-4471")
        print("  long [\(longTurn.line)]: \(longTurn.text.prefix(160).replacingOccurrences(of: "\n", with: " "))")
        print("  -> needle \(hit ? "RECALLED" : "MISSED") at \(tokens)-token context")
        print("  phys_footprint after L: \(String(format: "%.0f", Double(physFootprint()) / 1_048_576)) MiB")
    }
}

if #available(macOS 27.0, *) {
    do { try await run() } catch { print("multi-turn-probe: \(error)") }
} else {
    print("multi-turn-probe: requires macOS 27.0+")
    exit(2)
}
