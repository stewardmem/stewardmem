import Foundation
import MLXLMCommon

// THE BRIDGE'S CONVERSATION CACHE for the MLX tier — the seam CG-10 exists to
// close. The Qwen chat template is conversation-aware: it raises
// `No user query found in messages.` when the rendered list carries no
// non-tool user message, so the conversation must reach the template WHOLE.
// Upstream fixed `ChatSession` to pass its full conversation on every
// continuation (mlx-swift-lm PR #472, verified live by the multi-turn probe:
// turn 2 answers, the tool continuation answers, the cold control fails to
// recall). What upstream does NOT own is the bridge's side of the contract:
// a session this process CREATES after a restart or a bridge kill starts
// empty, and seeding it is our problem. This store is that seed's source.

/// One conversation entry in the bridge's cache — the WIRE shape
/// (`{"role": …, "text": …}` on the bridge protocol's `history` field), not
/// any framework's transcript type. The store converts to `Chat.Message` at
/// the render; the bridge child converts to `Transcript.Entry` for the AFM
/// kind. Keeping the entry framework-free is what lets one cache serve both
/// session kinds without either framework leaking into the wire shape.
public struct ConversationMessage: Sendable, Equatable {
    public enum Role: String, Sendable {
        case user
        case assistant
        /// A tool result. The wire never sends one today (Round 1 chat has no
        /// tools), but the render contract does not get to assume that: the
        /// Qwen failure class is precisely a list whose newest traffic is
        /// tool-shaped, so the store carries the role and the render law
        /// covers it.
        case tool
    }

    public let role: Role
    public let text: String

    public init(role: Role, text: String) {
        self.role = role
        self.text = text
    }
}

/// The conversation cache a seeded MLX `ChatSession` is constructed FROM.
///
/// ## THE AUTHORITY CHAIN, and why this is not a second store that can drift
///
/// The kernel's persisted transcript (`chat_turns`) is the authority. This
/// store is a CACHE of it, filled exactly twice in a session's life: seeded
/// from the wire's `history` field at session-start (the kernel's first+last
/// entries, CG-09 S2), and appended per completed turn. Between those, the
/// `ChatSession` built from it holds the conversation — it is the renderer,
/// not a rival ledger. The drift class the AFM half of this protocol already
/// refuses ("a live session's transcript is authoritative") is structurally
/// absent here, because nothing reads the store back over a live session: a
/// re-materialization always seeds from the WIRE, never from a stale cache.
///
/// ## THE APPEND LAW
///
/// A turn appends its user message and its assistant final TOGETHER, at the
/// final. A cancelled or failed turn appends NOTHING — which is exactly what
/// `ChatSession` records for such a turn (a cancelled generation has no
/// assistant turn to replay and its conversation is unchanged). Store and
/// session therefore cannot disagree about whether a turn happened.
///
/// ## THE RENDER LAW — the reason this type exists
///
/// `chatMessages(instructions:)` renders `system + full ordered transcript`
/// EVERY time, never a delta. A render that dropped the leading user turns
/// would hand the Qwen template a query-less list and reproduce the exact
/// raise this lane closed — so the law is pinned, not documented (the
/// `tool_only_continuation_renders_with_the_original_user_query` pin renders
/// precisely that shape and demands the original query back).
///
/// Rendering is O(n) string work; the model's cost is bounded by
/// `ChatSession`'s KV prefix compare, which processes only the new suffix of
/// a conversation whose rendered prefix is unchanged.
public struct ConversationStore: Sendable, Equatable {
    public private(set) var entries: [ConversationMessage]

    public init(entries: [ConversationMessage] = []) {
        self.entries = entries
    }

    /// Session-start reconciliation: the wire's seed REPLACES whatever a
    /// fresh store would have. Not append — a store that accumulated a
    /// previous incarnation's entries under a reused id would fabricate
    /// history neither the kernel nor the model ever saw.
    public mutating func seed(_ entries: [ConversationMessage]) {
        self.entries = entries
    }

    /// One completed turn, appended as the pair the conversation actually
    /// gained. The AFM side's append law lives in the bridge child (its
    /// session's transcript is its own authority); this is the MLX side's.
    public mutating func append(user: String, assistant: String) {
        entries.append(ConversationMessage(role: .user, text: user))
        entries.append(ConversationMessage(role: .assistant, text: assistant))
    }

    /// THE RENDER: `system + full ordered transcript`, every call. The
    /// instructions ride first as the system message — the same law the AFM
    /// kind enforces through the session's Profile — and an empty instruction
    /// renders nothing rather than a placeholder the template would treat as
    /// content.
    public func chatMessages(instructions: String?) -> [Chat.Message] {
        var out: [Chat.Message] = []
        if let instructions, !instructions.isEmpty {
            out.append(.system(instructions))
        }
        out.append(
            contentsOf: entries.map { entry in
                switch entry.role {
                case .user: Chat.Message.user(entry.text)
                case .assistant: Chat.Message.assistant(entry.text)
                case .tool: Chat.Message.tool(entry.text)
                }
            }
        )
        return out
    }
}
