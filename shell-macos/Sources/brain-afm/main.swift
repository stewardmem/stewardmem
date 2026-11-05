// brain-afm — the AFM bridge executable (the chat spine, Round 1).
//
// PROTOCOL (JSONL over stdio; one request per line in, one event per line out;
// exit on EOF). The kernel's bridge client is the only intended caller.
//
//   {"id":"c1","op":"capabilities"}
//     → {"id":"c1","ok":true,"window":4096,"budget":2710,
//        "languages":["en",…],"pccWindow":32768,"pccEligible":false,
//        "pccGeneration":"refused","escalationPermitted":false}
//
//   {"id":"t1","op":"turn","session":"s1","prompt":"…",
//    "instructions":"…"(first turn of a session only),"maxResponseTokens":512,
//    "model":"qwen3-8b-4bit"(optional — CG-10; absent = afm3-core)}
//     → zero or more {"id":"t1","event":"delta","text":"…"}
//       then exactly one terminal event:
//       {"id":"t1","event":"final","text":"…","abstain":false,
//        "claims":[{"text":"…","cites":[0,2]}],"inputTokens":N,"outputTokens":N,
//        "tier":"on_device","turn":N,"escalationRefused":false}
//       {"id":"t1","event":"error","code":…,"message":…,"shortfallTokens":N?}
//
// The `final` event is TYPED as of LL-04 (§1.1): the answer arrives through
// guided generation as a GroundedAnswer — abstain first, then the prose,
// then the sentence-level claims with the hit indices they assert. The
// kernel, not this process, judges those indices (no model in the gate).
//
// THE MODEL FIELD (CG-10 E3): additive-optional, like `history`. Absent →
// the AFM tier (`onDeviceAFM`), byte-identical to every caller that shipped
// before the field existed (the GDL propose loop among them — it never
// sends `model`, so its sessions stay AFM by construction). Present → the
// id is resolved through `ChatModelId` and gated through `forChat` BEFORE
// any registry or model work: an unknown id is `bad_request`, an excluded
// one (`qwen3.5-9b-4bit`) is `model_excluded` with its cause. A session's
// model is locked at first use — the conversation lives in ONE ledger
// (LanguageModelSession or ChatSession), and a mid-session tier flip would
// silently strand it — so a mismatching later request is `model_locked`.
// The `.mlx` kind runs the native `ChatSession` path (MLXFMProbe/MLXBridge):
// the store renders system + full transcript, the seeded history hydrates
// the conversation, deltas stream as raw fragments, and the final carries
// the same keys with `abstain:false, claims:[]` (the additive defaults —
// the MLX tier has no guided-generation schema yet) and `tier:"mlx_native"`.
//
//   {"id":"x1","op":"cancel","target":"t1"}   → abandons that turn's task
//
//   {"id":"p1","op":"prewarm","session":"s1","instructions":"…"(optional),
//    "history":[{"role":"user","text":"…"},{"role":"assistant","text":"…"}],
//    "model":"…"(optional, same law as turn)}
//     → on a FRESH session the history seeds it (measured against the
//       budget; history_unseedable by name when it cannot fit) —
//       ignored when the session already lives in this process.
//   {"id":"p1","op":"prewarm","session":"s1","instructions":"…"(optional)}
//     → creates the session under the instructions law and warms the model
//       via `session.prewarm(promptPrefix:)` — the kernel calls this before
//       the first turn so the cold load overlaps the grounding window.
//     → on the MLX kind the warm IS the materialization: the container
//       load (weights mmap + metal compile) is the cold cost, and it is
//       cached for the process lifetime after the first prewarm.
//     → {"id":"p1","ok":true} once the prefix is loaded; named error codes
//       (instructions_locked, model_unavailable, bad_request, model_excluded,
//       model_locked) otherwise.
//
// WHY SESSIONS LIVE HERE. The conversation's accumulated transcript — the
// thing HistoryBudget trims — can only exist inside a live
// `LanguageModelSession` (macOS 27 additionally allows seeding one from
// history; the rebuild path is a named Round-2 item, not shipped here). The
// kernel keeps its own audit copy (agent_session_events) and sends each
// turn's NEW prompt only; continuity is this process's state, keyed by the
// kernel's session id.
//
// macOS 27 IS REQUIRED for sessions: the trimmer rides `historyTransform`,
// which is a DynamicProfile method (27-only). On 26 the bridge answers
// capabilities and refuses turns with `model_unavailable` — named, not
// halfway-working. (Without the trimmer attached, an over-window session
// dies mid-generation with `contextSizeExceeded` — the exact failure the
// ChatModelLayer exists to prevent — so degrading silently is worse than
// refusing loudly.)
//
// WHAT THIS DOES NOT DO: no tools (Round 1 chat has none), no network except
// Apple's own framework (PCC), no keys, no tokens, no database. Errors are
// named codes, never prose parsed by the caller: model_unavailable ·
// conversation_does_not_fit · generation_failed · instructions_locked ·
// bad_request.
//
// PCC, measured not assumed: capabilities PROBES Private Cloud Compute with a
// one-line generation. On this machine (2026-10-25, macOS 27.0.1) the request
// is refused (`ModelManagerError 1046`) — NOT a hardware fact: PCC needs an
// APP-LEVEL entitlement (Apple developer-program enrollment; per-user daily
// quotas apply), so an unentitled process is refused while the model still
// reports itself "available". That split is why `pccEligible` and
// `pccGeneration` are separate fields: routing on eligibility alone produces
// a path that fails on first use. With the refusal measured,
// escalationPermitted is false and `.deep` stays nominal — exactly the
// posture ChatRouter was built for. The probe answer is cached for the
// process lifetime.

import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

import AppleFMProbe
import MLXFMProbe
import MLXLMCommon

// ─────────────────────────────────────────────────────────────────────────
// Output
// ─────────────────────────────────────────────────────────────────────────

/// One writer for stdout: JSONL lines, never interleaved. Every reply the
/// bridge emits crosses here. JSONSerialization with sorted keys keeps the
/// wire deterministic for the Rust side's fixtures.
final class LineOut: @unchecked Sendable {
    private let lock = NSLock()
    private let handle = FileHandle.standardOutput

    /// Optional values cross as JSON null, not as absent keys: the reader's
    /// contract is "every key is always present".
    func null(_ value: Any?) -> Any {
        value ?? NSNull()
    }

    func send(_ object: [String: Any]) {
        guard JSONSerialization.isValidJSONObject(object),
              let data = try? JSONSerialization.data(
                  withJSONObject: object,
                  options: [.sortedKeys, .withoutEscapingSlashes]
              )
        else { return }
        var line = data
        line.append(0x0A)
        lock.withLock {
            try? handle.write(contentsOf: line)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The in-flight registry (cancel support)
// ─────────────────────────────────────────────────────────────────────────

/// `op:cancel` names the request id of a running turn. The registry maps it
/// to that turn's Task; a cancel for an unknown/finished id is a no-op,
/// because "already done" and "never existed" are the same fact to a caller
/// holding a dropped stream.
final class InFlight: @unchecked Sendable {
    private let lock = NSLock()
    private var tasks: [String: Task<Void, Never>] = [:]

    func register(_ id: String, _ task: Task<Void, Never>) {
        lock.withLock { tasks[id] = task }
    }

    func cancel(_ id: String) {
        lock.withLock { tasks.removeValue(forKey: id) }?.cancel()
    }

    func finish(_ id: String) {
        lock.withLock { _ = tasks.removeValue(forKey: id) }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Errors — named codes on the wire, never prose a caller parses
// ─────────────────────────────────────────────────────────────────────────

enum BridgeError: Error {
    case modelUnavailable(String)
    case conversationDoesNotFit(shortfall: Int)
    case generationFailed(String)
    case instructionsLocked
    case badRequest(String)
    /// A fresh session's seeded history cannot fit the measured budget.
    /// Named, never silent amnesia: the turn is refused BEFORE the
    /// session materializes, so the transcript that failed seeding is
    /// unchanged and the next turn lands on the same terms.
    case historyUnseedable(shortfall: Int)
    /// A model id that `ChatModelId.forChat` refuses — the exclusion's
    /// cause crosses as the message (CG-10: the 9B's MLXVLM load gap).
    case modelExcluded(String)
    /// A session's model is locked at first use (the conversation lives in
    /// ONE ledger); a mismatching later request refuses rather than flips.
    case modelLocked(wanted: String, has: String)

    var code: String {
        switch self {
        case .modelUnavailable: "model_unavailable"
        case .conversationDoesNotFit: "conversation_does_not_fit"
        case .generationFailed: "generation_failed"
        case .instructionsLocked: "instructions_locked"
        case .badRequest: "bad_request"
        case .historyUnseedable: "history_unseedable"
        case .modelExcluded: "model_excluded"
        case .modelLocked: "model_locked"
        }
    }

    /// The human-readable part of the error event, for the codes that
    /// carry one.
    var message: String? {
        switch self {
        case .modelUnavailable(let m), .generationFailed(let m), .badRequest(let m),
            .modelExcluded(let m):
            m
        case .modelLocked(let wanted, let has):
            "session is locked to \(has); refusing \(wanted)"
        case .conversationDoesNotFit, .instructionsLocked, .historyUnseedable:
            nil
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// The typed answer — the @Generable schema lives in the AppleFMProbe
// library (`Sources/AppleFMProbe/GroundedAnswer.swift`), where the protocol
// pins can reach it: the bridge generates against it, the pins round-trip
// it, and the kernel's Rust mirror (`bridge_afm.rs`) reads its wire shape.
// ─────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────
// Capabilities (measured once, cached for the process lifetime)
// ─────────────────────────────────────────────────────────────────────────

#if canImport(FoundationModels)
struct Capabilities: Sendable {
    let window: Int
    let budget: Int?
    let languages: [String]
    let pccWindow: Int?
    let pccEligible: Bool
    /// "ok" · "refused" · "unavailable" — a MEASURED generation probe, not
    /// `isAvailable` (eligibility is true on machines that still refuse).
    let pccGeneration: String

    var escalationPermitted: Bool { pccGeneration == "ok" }

    @available(macOS 26.0, *)
    static func measure() async -> Capabilities {
        let model = SystemLanguageModel.default
        guard case .available = model.availability else {
            return Capabilities(
                window: 0, budget: nil, languages: [], pccWindow: nil,
                pccEligible: false, pccGeneration: "unavailable"
            )
        }
        let window = ChatWindow.onDevice().tokens ?? 0
        let budget = await BudgetCache.shared.measured()
        let languages = model.supportedLanguages.map {
            $0.minimalIdentifier.lowercased()
        }

        // PCC: window read (fast, local) + a one-line generation probe. A
        // refusal is an ANSWER, not a failure of this method.
        var pccWindow: Int?
        var pccEligible = false
        var pccGeneration = "unavailable"
        if #available(macOS 27.0, *) {
            let pcc = PrivateCloudComputeLanguageModel()
            pccEligible = pcc.isAvailable
            if case let .window(n) = await ChatWindow.privateCloud() {
                pccWindow = n
            }
            if await Self.probePCC(pcc) == true {
                pccGeneration = "ok"
            } else {
                pccGeneration = pccEligible ? "refused" : "unavailable"
            }
        }
        return Capabilities(
            window: window, budget: budget, languages: languages,
            pccWindow: pccWindow, pccEligible: pccEligible,
            pccGeneration: pccGeneration
        )
    }

    /// One bounded generation on PCC: "ok" in at most 8 tokens, greedy.
    /// Nil means the attempt could not be made or did not settle in time.
    @available(macOS 27.0, *)
    private static func probePCC(
        _ model: PrivateCloudComputeLanguageModel
    ) async -> Bool? {
        guard model.isAvailable else { return false }
        return await withTaskGroup(of: Bool?.self) { group in
            group.addTask {
                do {
                    let session = LanguageModelSession(
                        profile: LanguageModelSession.Profile {}
                            .model(model)
                    )
                    let options = GenerationOptions(
                        samplingMode: .greedy, maximumResponseTokens: 8
                    )
                    var sawText = false
                    for try await snapshot in session.streamResponse(
                        to: "Reply with exactly: ok", options: options
                    ) {
                        if !snapshot.content.isEmpty { sawText = true }
                    }
                    return sawText
                } catch {
                    return false
                }
            }
            group.addTask {
                // Bounded: a hung network path must not hang capabilities.
                // 20 s is generous against the one-word probe's expected
                // sub-second cost.
                try? await Task.sleep(nanoseconds: 20_000_000_000)
                return nil
            }
            let first = await group.next() ?? nil
            group.cancelAll()
            return first
        }
    }
}

/// The process-lifetime cache of the measured on-device budget — the same
/// number capabilities reports, so a session's trimmer and the router's
/// budget agree by construction. Measured once (async side), read anywhere.
final class BudgetCache: @unchecked Sendable {
    static let shared = BudgetCache()
    private let lock = NSLock()
    private var value: Int?

    @available(macOS 26.0, *)
    func measured() async -> Int? {
        if let cached = lock.withLock({ value }) {
            return cached
        }
        let measured = await ContextBudget.measureOnDeviceDefaults()
        lock.withLock { value = measured }
        return measured
    }
}
#endif

// ─────────────────────────────────────────────────────────────────────────
// Sessions
// ─────────────────────────────────────────────────────────────────────────

#if canImport(FoundationModels)
/// `@unchecked Sendable` with a stated invariant: the only mutable field
/// (`turn`) and every session API call happen INSIDE the per-session
/// `TurnGate`, which serializes turns by construction. Nothing else mutates.
@available(macOS 26.0, *)
final class BridgeSession: @unchecked Sendable {
    let session: LanguageModelSession
    let controller: HistoryBudget.Controller
    let planner: ChatTurnPlanner
    let instructions: String?
    private(set) var turn: Int = 0

    /// Builds the session with the trimmer ATTACHED — a session without
    /// `historyTransform` is not a degraded session but a doomed one (the
    /// planner would decide against a budget nothing enforces). The budget
    /// is handed in (measured on the async side, cached process-wide) so
    /// session construction stays sync under the sessions lock.
    init(
        instructions: String?, budget: Int, escalationPermitted: Bool,
        history: [Transcript.Entry] = []
    ) throws {
        let model = SystemLanguageModel.default
        guard case .available = model.availability else {
            throw BridgeError.modelUnavailable(
                "the on-device model is not available on this machine"
            )
        }
        // macOS 27 IS the session requirement: `historyTransform` is a
        // DynamicProfile method (27-only). Below it the turn is refused by
        // name rather than sent into a session whose window nothing guards.
        guard #available(macOS 27.0, *) else {
            throw BridgeError.modelUnavailable(
                "sessions require macOS 27 (historyTransform is a DynamicProfile method)"
            )
        }

        let controller = HistoryBudget.Controller(budgetTokens: budget)
        self.controller = controller
        self.instructions = instructions

        // Instructions ride the Profile's DynamicInstructions builder —
        // `Instructions` conforms, and a String is InstructionsRepresentable.
        let base = instructions.map { text in
            LanguageModelSession.Profile { Instructions(text) }
        } ?? LanguageModelSession.Profile {}
        // A fresh session may SEED from history (the kernel's persisted
        // transcript after a restart or a bridge kill). An empty history
        // is the ordinary live path. The trimmer still rides the profile,
        // so seeded entries are budgeted like any other transcript.
        self.session = LanguageModelSession(
            profile: base
                .model(model)
                .historyTransform(controller.transform),
            history: history
        )

        self.planner = ChatTurnPlanner(
            router: ChatRouter(
                escalationPermitted: escalationPermitted,
                onDeviceBudget: budget
            ),
            controller: controller
        )
    }

    func nextTurn() -> Int {
        turn += 1
        return turn
    }
}

/// All live sessions. Turns for one session serialize through a per-session
/// actor gate; the kernel drives one turn at a time, but the bridge does not
/// trust its caller with its own state.
@available(macOS 26.0, *)
final class Sessions: @unchecked Sendable {
    /// A session's PROTOCOL identity, distinct from its model-backed half:
    /// the instructions, the MODEL, and the turn gate exist the moment the
    /// id is first used, whether or not a session can be built on this
    /// machine. The instructions law lives on the slot, so the lock refuses a
    /// changed instruction even where no model can ever answer; the model
    /// law lives beside it (CG-10 E3) — the conversation lives in ONE ledger
    /// (`LanguageModelSession` or `ChatSession`), so a mid-session tier flip
    /// is refused, not merged.
    /// `@unchecked Sendable` with a stated invariant, the same shape as
    /// `BridgeSession`: the mutable fields (`bridge`, `mlx`) are written
    /// ONLY under the registry's lock (via `materialize`/`materializeMLX`)
    /// and every turn runs INSIDE the slot's `TurnGate`, which serializes
    /// them. Nothing else mutates.
    final class Slot: @unchecked Sendable {
        let instructions: String?
        let model: ChatModelId
        let gate = TurnGate()
        var bridge: BridgeSession?
        /// The `.mlx` kind's live half (CG-10): the `ChatSession` and the
        /// store it was rendered from. Nil until first materialization,
        /// exactly like `bridge` for the AFM kind.
        var mlx: MLXLive?
        init(instructions: String?, model: ChatModelId) {
            self.instructions = instructions
            self.model = model
        }
    }

    private let lock = NSLock()
    private var slots: [String: Slot] = [:]

    /// The one place a session id is created, and the one place the
    /// instructions law and the model law are decided: changing
    /// instructions later invalidates the entire cached prefix (Apple's own
    /// contract), and flipping a session's model would strand its ledger —
    /// so a later mismatch is REFUSED, not merged. Pure registry work — no
    /// model, no budget — so a refusal's ORDER does not depend on the
    /// machine.
    func resolve(id: String, instructions: String?, model: ChatModelId) throws -> Slot {
        try lock.withLock {
            if let slot = slots[id] {
                if let wanted = instructions, wanted != slot.instructions {
                    throw BridgeError.instructionsLocked
                }
                if model != slot.model {
                    throw BridgeError.modelLocked(
                        wanted: model.rawValue, has: slot.model.rawValue
                    )
                }
                return slot
            }
            let slot = Slot(instructions: instructions, model: model)
            slots[id] = slot
            return slot
        }
    }

    /// The model-backed half, built once per slot. This is the step that
    /// may answer `model_unavailable`; the slot it attaches to already
    /// carries the session's identity and its instructions lock.
    func materialize(
        _ slot: Slot, budget: Int, escalationPermitted: Bool,
        history: [Transcript.Entry] = []
    ) throws -> BridgeSession {
        try lock.withLock {
            if let bridge = slot.bridge { return bridge }
            let fresh = try BridgeSession(
                instructions: slot.instructions, budget: budget,
                escalationPermitted: escalationPermitted, history: history
            )
            slot.bridge = fresh
            return fresh
        }
    }

    /// The `.mlx` kind's materialization — attach-once under the same lock,
    /// the same law as `materialize`; the attached live half returns (the
    /// winner's, if a concurrent materialization got there first). The seed
    /// is validated BEFORE this call (the tier's context ceiling), so an
    /// unseedable history never attaches; what attaches carries the full
    /// rendered conversation.
    func materializeMLX(_ slot: Slot, live: MLXLive) -> MLXLive {
        lock.withLock {
            if let existing = slot.mlx { return existing }
            slot.mlx = live
            return live
        }
    }
}

/// The MLX kind's live half: the `ChatSession` a turn runs on, and the
/// store it was rendered from — appended at each FINAL (the append law:
/// a cancelled or failed turn adds nothing, mirroring the session's own
/// record behavior, so store and session cannot disagree about whether a
/// turn happened).
@available(macOS 26.0, *)
final class MLXLive {
    let session: ChatSession
    /// Var because the append law mutates it at each FINAL — and only
    /// there: every mutation happens inside the slot's gate (see the
    /// invariant on `Sessions.Slot`).
    var store: ConversationStore
    /// The 1-based turn counter — mutated only inside the slot's gate (the
    /// same invariant `BridgeSession.turn` states), so no lock is needed.
    private(set) var turn = 0

    init(session: ChatSession, store: ConversationStore) {
        self.session = session
        self.store = store
    }

    func nextTurn() -> Int {
        turn += 1
        return turn
    }
}

/// The process-lifetime MLX container cache — the tier's weights load ONCE
/// (the cold mmap + metal compile is exactly the cost the bridge's prewarm
/// op exists to overlap with the grounding window). Keyed by tier, guarded,
/// load-once; the AFM counterpart is `BudgetCache`.
final class MLXContainers: @unchecked Sendable {
    static let shared = MLXContainers()
    private let lock = NSLock()
    private var containers: [MLXChatTier: ModelContainer] = [:]
    private var loads: [MLXChatTier: Task<ModelContainer, Error>] = [:]

    @available(macOS 27.0, *)
    func container(for tier: MLXChatTier) async throws -> ModelContainer {
        if let cached = lock.withLock({ containers[tier] }) {
            return cached
        }
        // Coalesce concurrent first loads: a turn and its prewarm arriving
        // together must not compile the weights twice.
        let existing: Task<ModelContainer, Error>? = lock.withLock {
            if let load = loads[tier] { return load }
            let load = Task { try await MLXBridge.loadContainer(tier: tier) }
            loads[tier] = load
            return load
        }
        let container = try await existing!.value
        lock.withLock {
            containers[tier] = container
            loads[tier] = nil
        }
        return container
    }
}

/// An empty actor: one method, so calls serialize by construction.
actor TurnGate {
    func run(_ body: @Sendable () async throws -> Void) async rethrows {
        try await body()
    }
}
#endif

// ─────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────

@main
struct BrainAfmBridge {
    static func main() async {
        let out = LineOut()
        let inflight = InFlight()

        #if canImport(FoundationModels)
        guard #available(macOS 26.0, *) else {
            out.send([
                "event": "fatal", "code": "model_unavailable",
                "message": "FoundationModels requires macOS 26"
            ])
            return
        }
        let sessions = Sessions()
        var capabilities: Capabilities?

        // A stream error (not EOF) ends the bridge: the kernel's provider
        // treats child exit as a named failure and re-spawns on the next
        // turn, so a half-alive loop is worse than a dead one.
        do {
        for try await line in FileHandle.standardInput.bytes.lines {
            guard let data = line.data(using: .utf8),
                  let object = (try? JSONSerialization.jsonObject(with: data))
                    as? [String: Any],
                  let id = object["id"] as? String,
                  let op = object["op"] as? String
            else {
                out.send([
                    "event": "error", "code": "bad_request",
                    "message": "each line must be {id, op, …} JSON"
                ])
                continue
            }

            switch op {
            case "capabilities":
                if capabilities == nil {
                    capabilities = await Capabilities.measure()
                }
                guard let caps = capabilities else { fatalError("unreachable") }
                out.send([
                    "id": id, "ok": true,
                    "window": caps.window,
                    "budget": out.null(caps.budget),
                    "languages": caps.languages,
                    "pccWindow": out.null(caps.pccWindow),
                    "pccEligible": caps.pccEligible,
                    "pccGeneration": caps.pccGeneration,
                    "escalationPermitted": caps.escalationPermitted
                ])

            case "turn":
                guard let session = object["session"] as? String,
                      let prompt = object["prompt"] as? String
                else {
                    out.send([
                        "id": id, "event": "error", "code": "bad_request",
                        "message": "turn requires session and prompt"
                    ])
                    continue
                }
                let instructions = object["instructions"] as? String
                let maxResponse = object["maxResponseTokens"] as? Int
                let requestCap = min(max(maxResponse ?? 512, 16), 1024)
                // THE MODEL FIELD (CG-10 E3): additive-optional; absent is
                // the AFM tier. Resolved and gated through `forChat` HERE —
                // parse time, before the Task, so an unknown or excluded id
                // refuses by name without touching the registry or any
                // model, exactly like a malformed seed.
                let rawHistory = object["history"] as? [[String: Any]]
                let model: ChatModelId
                let conversation: [ConversationMessage]
                do {
                    model = try Self.resolveWireModel(object)
                    conversation = try Self.parseWireHistory(rawHistory)
                } catch let error as BridgeError {
                    var event: [String: Any] = [
                        "id": id, "event": "error", "code": error.code
                    ]
                    if let message = error.message {
                        event["message"] = message
                    }
                    out.send(event)
                    continue
                } catch {
                    out.send([
                        "id": id, "event": "error", "code": "bad_request",
                        "message": "history entries are malformed"
                    ])
                    continue
                }
                // Seeded history (transcript continuity): first + last
                // entries from the kernel's persisted transcript. Used
                // ONLY when this request creates the session; an existing
                // live session's transcript is authoritative and the
                // field is ignored. Parsed BEFORE the Task (entries are
                // Sendable; the raw dictionaries are not) — malformed
                // seed is a named refusal, never a skip.

                let task = Task {
                    defer { inflight.finish(id) }
                    // The session handle, hoisted so the error arms can
                    // reach it: a failure that happens AFTER the session
                    // exists attaches its feedback through the session; a
                    // failure before it (bad request, no capabilities,
                    // unmeasured budget) has nothing to attach through.
                    var liveSession: LanguageModelSession?
                    do {
                        guard let caps = capabilities else {
                            throw BridgeError.badRequest(
                                "capabilities must be requested before the first turn"
                            )
                        }
                        // The lock is a PROTOCOL decision, so it is resolved
                        // FIRST — before the budget measurement and the model
                        // itself, either of which answers `model_unavailable`
                        // on a machine without the model. A refusal whose
                        // ORDER depends on the machine's posture is not a
                        // contract; resolve() decides the instructions law
                        // and the model law against the registry alone, and
                        // materialization is the only step that needs a
                        // model.
                        let slot = try sessions.resolve(
                            id: session, instructions: instructions, model: model
                        )
                        // SESSION-TYPE ROUTING (CG-10): a VALUE off the id,
                        // not a branch someone forgot. The two kinds share
                        // the slot registry, the gate, and the wire; they
                        // differ in the ledger that holds the conversation.
                        switch slot.model.sessionKind {
                        case .afm:
                            guard let budget = await BudgetCache.shared.measured() else {
                                throw BridgeError.modelUnavailable(
                                    "the on-device budget could not be measured"
                                )
                            }
                            let seedEntries = try Self.transcriptEntries(from: conversation)
                            let bridgeSession = try await Self.seededBridgeSession(
                                sessions: sessions, slot: slot, budget: budget,
                                escalationPermitted: caps.escalationPermitted,
                                seedEntries: seedEntries
                            )
                            liveSession = bridgeSession.session
                            try await slot.gate.run {
                                let turnNo = bridgeSession.nextTurn()
                                try await Self.runTurn(
                                    id: id, prompt: prompt, turnNo: turnNo,
                                    cap: requestCap,
                                    bridgeSession: bridgeSession,
                                    out: out
                                )
                            }
                        case .mlx:
                            // The adapter surface (MLXFMProbe.model and the
                            // capability vocabulary) is 27-gated; a pre-27
                            // host refuses BY NAME rather than failing
                            // mid-tier.
                            guard #available(macOS 27.0, *) else {
                                throw BridgeError.modelUnavailable(
                                    "the MLX chat tier requires macOS 27 (the adapter surface is 27-gated)"
                                )
                            }
                            try await slot.gate.run {
                                let live = try await Self.seededMLXSession(
                                    sessions: sessions, slot: slot,
                                    conversation: conversation
                                )
                                let turnNo = live.nextTurn()
                                try await Self.runMLXTurn(
                                    id: id, prompt: prompt, turnNo: turnNo,
                                    cap: requestCap,
                                    slot: slot, live: live,
                                    out: out
                                )
                            }
                        }
                    } catch let error as BridgeError {
                        var event: [String: Any] = [
                            "id": id, "event": "error", "code": error.code
                        ]
                        if case let .conversationDoesNotFit(shortfall) = error {
                            event["shortfallTokens"] = shortfall
                        }
                        if case let .historyUnseedable(shortfall) = error {
                            event["shortfallTokens"] = shortfall
                        }
                        if case let .generationFailed(message) = error {
                            event["message"] = message
                        }
                        if case let .modelUnavailable(message) = error {
                            event["message"] = message
                        }
                        if case let .badRequest(message) = error {
                            event["message"] = message
                        }
                        if case let .modelExcluded(reason) = error {
                            event["message"] = reason
                        }
                        if case .modelLocked = error {
                            event["message"] = error.message
                        }
                        // THE FEEDBACK ATTACHMENT (CG-09 S6): a failed
                        // generation or a conversation that cannot fit
                        // attaches the turn's prompt + the desired-output
                        // slot through the session's own Feedback
                        // Assistant API. The attachment is held LOCALLY by
                        // the framework until the operator exports it
                        // through Apple's UI — nothing here sends, and a
                        // successful turn attaches nothing. The error
                        // event carries the marker so the kernel's audit
                        // copy sees the attachment happen (the parseable
                        // artifact the T-7 pin reads).
                        switch error {
                        case .generationFailed, .conversationDoesNotFit:
                            if let live = liveSession {
                                _ = live.logFeedbackAttachment(
                                    sentiment: .negative,
                                    issues: [],
                                    desiredResponseText: String(
                                        "a grounded, cited answer to: \(prompt)".prefix(2000)
                                    )
                                )
                                event["feedback"] = "attached"
                            }
                        default:
                            break
                        }
                        out.send(event)
                    } catch {
                        out.send([
                            "id": id, "event": "error",
                            "code": "generation_failed",
                            "message": String(String(describing: error).prefix(300))
                        ])
                    }
                }
                inflight.register(id, task)
                await task.value

            case "prewarm":
                guard let session = object["session"] as? String else {
                    out.send([
                        "id": id, "event": "error", "code": "bad_request",
                        "message": "prewarm requires session"
                    ])
                    continue
                }
                let instructions = object["instructions"] as? String
                let rawHistory = object["history"] as? [[String: Any]]
                do {
                    // The model field follows the same parse-time law as on
                    // `turn`: an unknown or excluded id refuses HERE, before
                    // the registry and any model work.
                    let model = try Self.resolveWireModel(object)
                    guard capabilities != nil else {
                        throw BridgeError.badRequest(
                            "capabilities must be requested before the first turn"
                        )
                    }
                    // The instructions law and the model law are PROTOCOL
                    // decisions: resolve the slot first (refusals precede
                    // any model work), then materialize and warm inside the
                    // session gate so a racing turn never observes a
                    // half-warmed session.
                    let slot = try sessions.resolve(
                        id: session, instructions: instructions, model: model
                    )
                    // The seed rides the prewarm too: a prewarm that
                    // materializes without it would poison seeding.
                    let conversation = try Self.parseWireHistory(rawHistory)
                    // SESSION-TYPE ROUTING (CG-10): the AFM kind warms via
                    // the session's own `prewarm(promptPrefix:)`; the MLX
                    // kind's warm IS the materialization — the container
                    // load is the cold cost, cached process-wide by
                    // MLXContainers after the first prewarm.
                    switch slot.model.sessionKind {
                    case .afm:
                        guard let budget = await BudgetCache.shared.measured() else {
                            throw BridgeError.modelUnavailable(
                                "the on-device budget could not be measured"
                            )
                        }
                        let seedEntries = try Self.transcriptEntries(from: conversation)
                        let bridgeSession = try await Self.seededBridgeSession(
                            sessions: sessions, slot: slot, budget: budget,
                            escalationPermitted: capabilities?.escalationPermitted ?? false,
                            seedEntries: seedEntries
                        )
                        let prefix = Prompt(slot.instructions ?? "")
                        await slot.gate.run {
                            bridgeSession.session.prewarm(promptPrefix: prefix)
                        }
                    case .mlx:
                        guard #available(macOS 27.0, *) else {
                            throw BridgeError.modelUnavailable(
                                "the MLX chat tier requires macOS 27 (the adapter surface is 27-gated)"
                            )
                        }
                        _ = try await Self.seededMLXSession(
                            sessions: sessions, slot: slot, conversation: conversation
                        )
                    }
                    out.send(["id": id, "ok": true])
                } catch let error as BridgeError {
                    var event: [String: Any] = [
                        "id": id, "event": "error", "code": error.code
                    ]
                    if case let .historyUnseedable(shortfall) = error {
                        event["shortfallTokens"] = shortfall
                    }
                    if case let .modelUnavailable(message) = error {
                        event["message"] = message
                    }
                    if case let .badRequest(message) = error {
                        event["message"] = message
                    }
                    if case let .modelExcluded(reason) = error {
                        event["message"] = reason
                    }
                    if case .modelLocked = error {
                        event["message"] = error.message
                    }
                    out.send(event)
                } catch {
                    out.send([
                        "id": id, "event": "error",
                        "code": "generation_failed",
                        "message": String(String(describing: error).prefix(300))
                    ])
                }

            case "cancel":
                if let target = object["target"] as? String {
                    inflight.cancel(target)
                }

            default:
                out.send([
                    "id": id, "event": "error", "code": "bad_request",
                    "message": "unknown op \(op)"
                ])
            }
        }
        } catch {
            out.send([
                "event": "fatal", "code": "generation_failed",
                "message": String(String(describing: error).prefix(300))
            ])
        }
        #else
        out.send([
            "event": "fatal", "code": "model_unavailable",
            "message": "built without FoundationModels"
        ])
        #endif
    }

    #if canImport(FoundationModels)
    /// The ONE seeded-materialization path, shared by `turn` and
    /// `prewarm`: a live session's transcript is authoritative (the
    /// seed is ignored); a session this request creates measures the
    /// seed against the budget and refuses BY NAME when unseedable —
    /// before materializing, so the transcript is unchanged.
    @available(macOS 26.0, *)
    static func seededBridgeSession(
        sessions: Sessions, slot: Sessions.Slot, budget: Int,
        escalationPermitted: Bool, seedEntries: [Transcript.Entry]
    ) async throws -> BridgeSession {
        var creatingHistory: [Transcript.Entry] = []
        if slot.bridge == nil && !seedEntries.isEmpty {
            guard #available(macOS 26.4, *) else {
                throw BridgeError.modelUnavailable(
                    "seed measurement requires macOS 26.4 (tokenCount)"
                )
            }
            let seedTokens = try await SystemLanguageModel.default.tokenCount(for: seedEntries)
            if seedTokens > budget {
                throw BridgeError.historyUnseedable(
                    shortfall: seedTokens - budget
                )
            }
            creatingHistory = seedEntries
        }
        return try sessions.materialize(
            slot, budget: budget,
            escalationPermitted: escalationPermitted, history: creatingHistory
        )
    }

    /// THE MODEL FIELD (CG-10 E3): resolve the optional `model` id through
    /// `ChatModelId` and gate it through `forChat`. Absent is the AFM tier.
    /// Parse-time law: an unknown id is `bad_request`, an excluded one is
    /// `model_excluded` WITH ITS CAUSE — both refuse here, before the
    /// registry or any model work, exactly like a malformed seed.
    static func resolveWireModel(_ object: [String: Any]) throws -> ChatModelId {
        guard let raw = object["model"] as? String else {
            return .onDeviceAFM
        }
        guard let id = ChatModelId(rawValue: raw) else {
            throw BridgeError.badRequest(
                "unknown model \(raw) — the bridge serves \(ChatModelId.allCases.map(\.rawValue).joined(separator: ", "))"
            )
        }
        do {
            return try ChatModelId.forChat(id)
        } catch let error as ChatModelError {
            if case let .excludedFromChat(_, reason) = error {
                throw BridgeError.modelExcluded(reason)
            }
            throw BridgeError.modelExcluded("the model is excluded from chat")
        }
    }

    /// Parse the kernel's seeded history field into the WIRE shape (the
    /// conversation cache both session kinds consume). A missing/empty field
    /// is no seed; anything malformed (missing role/text, or a role neither
    /// side names) is `bad_request` — the whole field is refused, never
    /// partially applied. The role law is the WIRE's and predates the kind
    /// split: `user` or `assistant`; the kernel's transcript carries nothing
    /// else.
    static func parseWireHistory(
        _ raw: [[String: Any]]?
    ) throws -> [ConversationMessage] {
        guard let raw, !raw.isEmpty else { return [] }
        var out: [ConversationMessage] = []
        for item in raw {
            guard let role = item["role"] as? String,
                  let text = item["text"] as? String
            else {
                throw BridgeError.badRequest(
                    "history entries need a role and text"
                )
            }
            switch role {
            case "user":
                out.append(ConversationMessage(role: .user, text: text))
            case "assistant":
                out.append(ConversationMessage(role: .assistant, text: text))
            default:
                throw BridgeError.badRequest(
                    "history role must be user or assistant"
                )
            }
        }
        return out
    }

    /// The AFM kind's conversion of the wire conversation into transcript
    /// entries — the segment construction this file's macOS 27 note records.
    @available(macOS 26.0, *)
    static func transcriptEntries(
        from conversation: [ConversationMessage]
    ) throws -> [Transcript.Entry] {
        guard !conversation.isEmpty else { return [] }
        guard #available(macOS 27.0, *) else {
            throw BridgeError.modelUnavailable(
                "transcript seeding requires macOS 27 (segment construction)"
            )
        }
        var out: [Transcript.Entry] = []
        out.reserveCapacity(conversation.count)
        for entry in conversation {
            switch entry.role {
            case .user:
                out.append(.prompt(
                    Transcript.Prompt(
                        segments: [.text(Transcript.TextSegment(content: entry.text))])
                ))
            case .assistant:
                out.append(.response(
                    Transcript.Response(
                        segments: [.text(Transcript.TextSegment(content: entry.text))])
                ))
            case .tool:
                // Unreachable through the wire law above; the compiler
                // demands the case and the render contract (M-1) names the
                // role — a tool entry on the AFM seed refuses rather than
                // guessing a transcript shape.
                throw BridgeError.badRequest(
                    "the AFM seed does not carry tool entries"
                )
            }
        }
        return out
    }

    /// The MLX kind's seeded materialization (CG-10 E3): the tier's
    /// container loads once per process (the prewarm overlap), the seed is
    /// measured against the TIER's own context — the MLX counterpart of the
    /// AFM arm's budget law, refusing `history_unseedable` BY NAME before a
    /// session carries it — and the store's render hydrates the
    /// conversation. A live session ignores the seed (its ledger is
    /// authoritative), exactly the law the AFM arm states.
    @available(macOS 27.0, *)
    static func seededMLXSession(
        sessions: Sessions, slot: Sessions.Slot, conversation: [ConversationMessage]
    ) async throws -> MLXLive {
        if let live = slot.mlx { return live }
        let tier = try Self.mlxTier(for: slot.model)
        let container = try await MLXContainers.shared.container(for: tier)
        if !conversation.isEmpty {
            let seedTokens = await MLXBridge.seedTokenCount(conversation, container: container)
            let ceiling = tier.contextTokens - tier.contextTokens / 10
            if seedTokens > ceiling {
                throw BridgeError.historyUnseedable(shortfall: seedTokens - ceiling)
            }
        }
        let built = MLXBridge.makeSession(
            container: container,
            instructions: slot.instructions,
            seed: conversation,
            maxTokens: 1024
        )
        let live = MLXLive(session: built.session, store: built.store)
        return sessions.materializeMLX(slot, live: live)
    }

    /// The `.mlx` tier behind a resolved `ChatModelId` — unreachable for an
    /// AFM id (the caller dispatches on `sessionKind` first); a named trap
    /// rather than a silent fallback if the routing ever drifts.
    @available(macOS 26.0, *)
    static func mlxTier(for model: ChatModelId) throws -> MLXChatTier {
        guard let tier = MLXChatTier.tier(modelId: model.rawValue) else {
            throw BridgeError.modelUnavailable(
                "no MLX configuration for \(model.rawValue) — see MLXChatTier"
            )
        }
        return tier
    }

    /// One MLX turn inside the slot's gate: stream fragments as deltas,
    /// emit exactly one terminal event, append the pair to the store at the
    /// FINAL (the append law — a cancelled or failed turn adds nothing).
    /// The MLX tier has no guided-generation schema, so the final's
    /// `abstain`/`claims` ride the wire's additive defaults (false/empty)
    /// and the kernel's own zero-hit abstain gate still runs upstream of
    /// this process — the honest boundary, named here rather than faked.
    @available(macOS 27.0, *)
    static func runMLXTurn(
        id: String, prompt: String, turnNo: Int, cap: Int,
        slot: Sessions.Slot, live: MLXLive, out: LineOut
    ) async throws {
        let answer: String
        let inputTokens: Int
        let outputTokens: Int
        do {
            let result = try await MLXBridge.streamTurn(
                session: live.session,
                prompt: prompt,
                maxTokens: cap,
                onDelta: { fragment in
                    out.send(["id": id, "event": "delta", "text": fragment])
                }
            )
            answer = result.text
            inputTokens = result.inputTokens
            outputTokens = result.outputTokens
        } catch let error as BridgeError {
            throw error
        } catch {
            throw BridgeError.generationFailed(String(describing: error))
        }

        if Task.isCancelled {
            // The reader is gone; the terminal event goes nowhere, but the
            // session stays usable — cancel kills the TURN, not the
            // conversation. Nothing is appended (the append law).
            return
        }

        // The append law: the store gains exactly what the session gained,
        // at the final.
        live.store.append(user: prompt, assistant: answer)

        // An MLX stream that produced no text at all is STILL an empty
        // answer on this tier — silence is forwarded as silence (the
        // kernel's deterministic abstain gate has already run upstream;
        // faking an abstain flag the tier does not compute would be a
        // lie on the wire). Named in the audit by the tier field.
        out.send([
            "id": id, "event": "final", "text": answer,
            "abstain": false,
            "claims": [] as [[String: Any]],
            "inputTokens": inputTokens, "outputTokens": outputTokens,
            "tier": "mlx_native", "turn": turnNo,
            "escalationRefused": false
        ])
    }

    /// One turn on an existing session: plan (which may refuse BEFORE the
    /// request leaves), stream snapshots as incremental deltas, emit exactly
    /// one terminal event. Runs inside the session's gate — serialized by
    /// construction.
    @available(macOS 26.0, *)
    static func runTurn(
        id: String, prompt: String, turnNo: Int, cap: Int,
        bridgeSession: BridgeSession, out: LineOut
    ) async throws {
        let model = SystemLanguageModel.default
        let session = bridgeSession.session

        // Measurement is the floor the whole planner stands on:
        // `tokenCount` needs macOS 26.4, and a turn that cannot be measured
        // cannot be routed — it is refused by name rather than sent blind.
        guard #available(macOS 26.4, *) else {
            throw BridgeError.modelUnavailable(
                "turn measurement requires macOS 26.4 (tokenCount)"
            )
        }

        // Measure then route, per the planner's own law. The whole prompt —
        // fenced recall grounding included — is the turn's demand, so it is
        // measured as one unit and `evidenceTokens` is 0 by construction,
        // not by omission. An unmeasurable prompt fails closed (0 would
        // certify an unknown size as a fitting one).
        let entries = Array(session.transcript)
        let promptTokens = try await model.tokenCount(for: prompt)
        do {
            let decision = try await bridgeSession.planner.prepare(
                turn: turnNo,
                evidenceTokens: 0,
                entries: entries,
                incomingTokens: promptTokens,
                cost: { entry in
                    // The layer's cost closure is non-throwing BY DESIGN —
                    // the failure policy belongs to this caller. An entry
                    // that cannot be measured maps to an enormous cost, so
                    // the plan cannot fit it and the turn fails closed as
                    // conversation_does_not_fit: an unmeasured entry is a
                    // cost nobody knows, never a free one.
                    (try? await model.tokenCount(for: [entry])) ?? (1 << 30)
                }
            )
            // Round 1: `.deep` is nominal (PCC refused on every machine
            // measured so far), so a deep decision cannot occur with
            // escalationPermitted false — and a state that cannot be reached
            // is refused, not faked.
            if decision.tier == .deep {
                throw BridgeError.generationFailed(
                    "escalated tier is not available in this build"
                )
            }
        } catch let error as ChatTurnError {
            if case let .conversationDoesNotFit(shortfall) = error {
                throw BridgeError.conversationDoesNotFit(shortfall: shortfall)
            }
            throw BridgeError.generationFailed(String(describing: error))
        }

        let options = GenerationOptions(
            samplingMode: .greedy, maximumResponseTokens: cap
        )
        // THE TYPED CONTRACT (LL-04 §1.1): guided generation asks the model
        // for a GroundedAnswer, not prose — the cite-or-abstain shape is the
        // measured `guidedGeneration` capability, a SCHEMA rather than a
        // regex over the text. Deltas ride the partial `answer` exactly as
        // they rode the raw string before (suffix emit; wholesale-replace on
        // a revision); `abstain` and `claims` are read only from the SETTLED
        // value, never from a partial snapshot, so no reader ever sees a
        // half-decided abstention.
        var full = ""
        var settled: GeneratedContent?
        var inputTokens = 0
        var outputTokens = 0
        do {
            for try await snapshot in session.streamResponse(
                to: prompt, generating: GroundedAnswer.self, options: options
            ) {
                if Task.isCancelled { break }
                let content = snapshot.content.answer ?? ""
                if content.count > full.count {
                    // Snapshots are cumulative; emit only the suffix.
                    let delta = String(content.dropFirst(full.count))
                    full = content
                    out.send(["id": id, "event": "delta", "text": delta])
                } else if content != full {
                    // A revision (not append): replace wholesale, marked, so
                    // no reader reconstructs a Frankenstein from suffixes.
                    full = content
                    out.send([
                        "id": id, "event": "delta", "text": content,
                        "replace": true
                    ])
                }
                settled = snapshot.rawContent
                if #available(macOS 27.0, *) {
                    inputTokens = max(
                        inputTokens, snapshot.usage.input.totalTokenCount
                    )
                    outputTokens = max(
                        outputTokens, snapshot.usage.output.totalTokenCount
                    )
                }
            }
        } catch {
            throw BridgeError.generationFailed(String(describing: error))
        }

        if Task.isCancelled {
            // The reader is gone; the terminal event goes nowhere, but the
            // session stays usable — cancel kills the TURN, not the
            // conversation.
            return
        }

        // The COMPLETE typed answer decodes from the last snapshot's raw
        // content — the one place the framework guarantees every field is
        // settled. A stream that produced no content at all IS an
        // abstention: the model said nothing, and silence is never a
        // supported answer. A present-but-undecodable answer is a named
        // failure, not a prose fallback — falling back to the raw string
        // would re-open the unverified-citation hole this schema closes.
        let typed: GroundedAnswer
        if let settled {
            do {
                typed = try GroundedAnswer(settled)
            } catch {
                throw BridgeError.generationFailed(
                    "the typed answer did not decode: \(String(describing: error))"
                )
            }
        } else {
            typed = GroundedAnswer(abstain: true, answer: "", claims: [])
        }

        out.send([
            "id": id, "event": "final", "text": typed.answer,
            "abstain": typed.abstain,
            "claims": typed.claims.map { claim in
                ["text": claim.text, "cites": claim.cites] as [String: Any]
            },
            "inputTokens": inputTokens, "outputTokens": outputTokens,
            "tier": "on_device", "turn": turnNo,
            "escalationRefused": false
        ])
    }
    #endif
}
