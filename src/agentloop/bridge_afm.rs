//! The AFM bridge client — the kernel's one egress to `brain-afm`.
//!
//! The bridge is a child process speaking JSONL over stdio (see
//! `shell-macos/Sources/brain-afm/main.swift` for the protocol's other half).
//! This module owns the child, multiplexes turns by request id, and honors
//! the drop-cancel contract the loop's provider seam established: dropping
//! the returned receiver triggers `op:cancel`, which kills the TURN, not the
//! session — the conversation's live state is the bridge's, and it survives.
//!
//! WHY A SEPARATE SEAM FROM `LlmProvider`. The provider trait is STATELESS by
//! contract (each `stream()` receives the whole message list — built for HTTP
//! providers). The AFM session is STATEFUL: the framework has no seed-history
//! API in the shape the loop would need per turn, and the trimmer
//! (`historyTransform`) lives inside the bridge. The chat route therefore
//! drives this client directly with per-turn prompts; a stateless
//! `AfmBridgeProvider` adapter is OWED to the round that puts tools in the
//! chat (LoopDriver adoption), and is not built speculatively here.
//!
//! Process-wide singleton via `chat_bridge()`, following the `screen.rs`
//! CLASSIFIER precedent: a child-process handle is process state, spawned
//! once from `config::afm_bridge_path()`, absent → the chat route answers
//! with a named refusal (fail-open for every other surface of the server).
//!
//! ponytail: no env knobs read inside (the path comes from config at the one
//! spawn site), no keys, no network — the child's only network is Apple's
//! own framework (PCC).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;

/// One streamed event from a turn, or its terminal outcome.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    /// An incremental text fragment. `replace` marks a wholesale revision
    /// (the model re-wrote its running answer): the receiver must discard
    /// prior deltas, not append.
    Delta { text: String, replace: bool },
    /// The turn's terminal success. TYPED as of LL-04 (§1.1): `abstain` and
    /// `claims` arrive through the bridge's guided generation (the Swift
    /// child's `GroundedAnswer`, `shell-macos/Sources/AppleFMProbe/
    /// GroundedAnswer.swift`). Both are ADDITIVE on the protocol: an old
    /// bridge that sends neither parses as `false` / empty — never an
    /// error. The kernel never edits these; it labels them.
    Final {
        text: String,
        tier: String,
        turn: i64,
        input_tokens: i64,
        output_tokens: i64,
        abstain: bool,
        claims: Vec<BridgeClaim>,
    },
    /// The turn's terminal failure, by the bridge's named code
    /// (`conversation_does_not_fit` carries `shortfall_tokens`).
    /// `feedback` carries the S6 capture marker: the bridge attached a
    /// Feedback Assistant artifact for this failure (held LOCALLY by the
    /// framework until the operator exports it). ADDITIVE (LL-04 §1.1):
    /// absent on an old bridge → `None`, never a parse error — the kernel
    /// must be able to run against any bridge revision it shipped with.
    Failed {
        code: String,
        message: Option<String>,
        shortfall_tokens: Option<i64>,
        feedback: Option<String>,
    },
}

/// One sentence-level claim the bridge's typed final carries: the model's
/// text, verbatim, and the 0-based grounding-hit indices it asserts. The
/// WIRE SHAPE IS OWNED by the Swift child (`GroundedClaim`); this struct
/// mirrors it, drift surfaces in the pins. `#[serde(default)]` on both
/// fields so a drift-shaped claim degrades to "uncited" — the honest label
/// for a claim that proves nothing — never a dropped turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BridgeClaim {
    pub text: String,
    pub cites: Vec<i64>,
}

/// The measured machine facts, cached by the bridge for its lifetime.
/// Wire names are the SWIFT side's camelCase (the protocol's shape is owned
/// by `brain-afm`; this struct mirrors it, drift surfaces in the pins).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeCapabilities {
    pub window: i64,
    pub budget: Option<i64>,
    pub languages: Vec<String>,
    pub pcc_window: Option<i64>,
    #[serde(default)]
    pub pcc_eligible: bool,
    pub pcc_generation: String,
    #[serde(default)]
    pub escalation_permitted: bool,
}

/// One seeded transcript entry: the persisted transcript the bridge
/// seeds a FRESH session from (first + last entries, kernel-built).
#[derive(Debug, Clone)]
pub struct BridgeHistoryEntry {
    pub role: String,
    pub text: String,
}

/// One turn request. `instructions` reaches the bridge only on a session's
/// FIRST turn (the bridge refuses a changed instruction later — a prefix
/// cache is invalidated by an instruction change, so merging would be a lie).
/// `history` seeds a session the child must CREATE (after a restart or
/// kill); an existing live session ignores it — its live transcript is
/// authoritative. `model` names the chat tier: absent = the AFM
/// tier, byte-identical to every caller that shipped before the field
/// existed — the GDL propose loop never sends it, so its sessions stay AFM
/// by construction. The CHILD owns the vocabulary (`ChatModelId` + `forChat`);
/// the kernel transports the configured string verbatim. Additive-optional:
/// an old bridge ignores unknown fields.
///
/// `schema` (round 14, Arm A): an optional JSON-schema DOCUMENT string —
/// present, the child generates against the RUNTIME schema
/// (`DynamicGenerationSchema` → `GenerationSchema` → `respond(to:schema:)`)
/// and the settled artifact JSON crosses as the answer; absent, the
/// compiled `GroundedAnswer` contract generates EXACTLY as it did before
/// this field existed — the two never blend (the chat spine sends no
/// schema; the wire body carries the key only when `Some`).
#[derive(Debug, Clone)]
pub struct BridgeTurn {
    pub session: String,
    pub prompt: String,
    pub instructions: Option<String>,
    pub max_response_tokens: Option<i64>,
    pub history: Option<Vec<BridgeHistoryEntry>>,
    pub model: Option<String>,
    pub schema: Option<String>,
}

/// Client-side failure taxonomy: transport-shaped, never the bridge's named
/// turn codes (those arrive as [`BridgeEvent::Failed`]).
#[derive(Debug)]
pub enum BridgeError {
    Unavailable(String),
    Protocol(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Unavailable(detail) => {
                write!(f, "the AFM bridge is not available: {detail}")
            }
            BridgeError::Protocol(detail) => {
                write!(f, "the AFM bridge protocol broke: {detail}")
            }
        }
    }
}

impl std::error::Error for BridgeError {}

/// The live child + the pending-turn table.
pub struct AfmBridge {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<BridgeEvent>>>>,
    capabilities: OnceLock<Arc<BridgeCapabilities>>,
}

impl AfmBridge {
    /// Spawn the child and start its reader task. The reader is the ONLY
    /// consumer of stdout; every line routes by id to the registered sender,
    /// and a dead child fails all pending turns at once.
    pub fn spawn(path: &Path) -> Result<Arc<Self>, BridgeError> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| BridgeError::Unavailable(format!("spawn {path:?}: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| BridgeError::Protocol("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BridgeError::Protocol("no stdout".into()))?;

        let pending: Arc<Mutex<HashMap<String, mpsc::Sender<BridgeEvent>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let Some(value) = serde_json::from_str::<serde_json::Value>(&line).ok() else {
                    continue;
                };
                let id = value.get("id").and_then(|v| v.as_str()).map(str::to_owned);
                route_line(&reader_pending, id, value);
            }
            // Child exit: every pending turn fails at once, by name — a
            // caller holding a stream must learn the bridge died, not hang
            // until the router's timeout.
            fail_all_pending(
                &reader_pending,
                "bridge_exited",
                "the bridge process exited",
            );
        });

        Ok(Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending,
            capabilities: OnceLock::new(),
        }))
    }

    /// The measured capabilities, requested once and cached for the child's
    /// lifetime (the bridge caches its own side identically).
    pub async fn capabilities(&self) -> Result<Arc<BridgeCapabilities>, BridgeError> {
        if let Some(cached) = self.capabilities.get() {
            return Ok(Arc::clone(cached));
        }
        let (tx, mut rx) = mpsc::channel::<BridgeEvent>(16);
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut pending = self.pending.lock().expect("bridge pending lock");
            pending.insert(id.clone(), tx);
        }
        self.write_line(&json!({ "id": id, "op": "capabilities" }))?;
        // Deltas never precede a capabilities reply, but the reader speaks
        // one event type; treat Final-ish payloads generically below.
        while let Some(event) = rx.recv().await {
            match event {
                BridgeEvent::Failed { code, message, .. } => {
                    self.pending
                        .lock()
                        .expect("bridge pending lock")
                        .remove(&id);
                    return Err(BridgeError::Protocol(format!(
                        "capabilities failed: {code} {message:?}"
                    )));
                }
                BridgeEvent::Delta { .. } => continue,
                BridgeEvent::Final { text, .. } => {
                    let caps: BridgeCapabilities = serde_json::from_str(&text)
                        .map_err(|e| BridgeError::Protocol(format!("capabilities body: {e}")))?;
                    let caps = Arc::new(caps);
                    let _ = self.capabilities.set(Arc::clone(&caps));
                    self.pending
                        .lock()
                        .expect("bridge pending lock")
                        .remove(&id);
                    return Ok(caps);
                }
            }
        }
        Err(BridgeError::Unavailable(
            "the bridge closed before answering capabilities".into(),
        ))
    }

    /// Begin one streamed turn. DROPPING the receiver cancels the turn (the
    /// bridge abandons its task; the session survives).
    pub async fn turn(
        &self,
        request: BridgeTurn,
    ) -> Result<(String, mpsc::Receiver<BridgeEvent>), BridgeError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::channel::<BridgeEvent>(64);
        self.pending
            .lock()
            .expect("bridge pending lock")
            .insert(id.clone(), tx);
        let mut body = json!({
            "id": id,
            "op": "turn",
            "session": request.session,
            "prompt": request.prompt,
        });
        if let Some(instructions) = &request.instructions {
            body["instructions"] = json!(instructions);
        }
        if let Some(cap) = request.max_response_tokens {
            body["maxResponseTokens"] = json!(cap);
        }
        if let Some(history) = &request.history {
            body["history"] = json!(
                history
                    .iter()
                    .map(|e| json!({ "role": e.role, "text": e.text }))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(model) = &request.model {
            body["model"] = json!(model);
        }
        // THE WIRE SCHEMA (round 14 Arm A): written ONLY when present —
        // a schema-absent turn serializes byte-identically to the
        // pre-field wire (the KT-3 control).
        if let Some(schema) = &request.schema {
            body["schema"] = json!(schema);
        }
        if self.write_line(&body).is_err() {
            self.pending
                .lock()
                .expect("bridge pending lock")
                .remove(&id);
            return Err(BridgeError::Unavailable(
                "writing to the bridge failed — the child is likely dead".into(),
            ));
        }
        Ok((id, rx))
    }

    /// Warm a session's model before its first turn: `session.prewarm(
    /// promptPrefix:)` on the child. The instructions ride the request so
    /// the prewarm ALSO creates the session under the instructions law (a
    /// later turn with changed instructions is still refused by name).
    /// The seeded history rides too — a prewarm that materializes the
    /// session WITHOUT the seed would poison seeding (the turn's later
    /// seed is ignored once the session lives). Fire-and-await: the child
    /// answers `ok` once the prefix is loaded.
    pub async fn prewarm(
        &self,
        session: &str,
        instructions: Option<&str>,
        history: Option<&[BridgeHistoryEntry]>,
        model: Option<&str>,
    ) -> Result<bool, BridgeError> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, mut rx) = mpsc::channel::<BridgeEvent>(16);
        self.pending
            .lock()
            .expect("bridge pending lock")
            .insert(id.clone(), tx);
        let mut body = json!({
            "id": id,
            "op": "prewarm",
            "session": session,
        });
        if let Some(instructions) = instructions {
            body["instructions"] = json!(instructions);
        }
        if let Some(history) = history {
            body["history"] = json!(
                history
                    .iter()
                    .map(|e| json!({ "role": e.role, "text": e.text }))
                    .collect::<Vec<_>>()
            );
        }
        if let Some(model) = model {
            body["model"] = json!(model);
        }
        if let Err(e) = self.write_line(&body) {
            self.pending
                .lock()
                .expect("bridge pending lock")
                .remove(&id);
            return Err(e);
        }
        while let Some(event) = rx.recv().await {
            match event {
                BridgeEvent::Failed { code, message, .. } => {
                    self.pending
                        .lock()
                        .expect("bridge pending lock")
                        .remove(&id);
                    return Err(BridgeError::Protocol(format!(
                        "prewarm failed: {code} {message:?}"
                    )));
                }
                BridgeEvent::Delta { .. } => continue,
                BridgeEvent::Final { .. } => {
                    self.pending
                        .lock()
                        .expect("bridge pending lock")
                        .remove(&id);
                    return Ok(true);
                }
            }
        }
        Err(BridgeError::Unavailable(
            "the bridge closed before answering prewarm".into(),
        ))
    }

    /// Cancel an in-flight turn by its SSE-facing request id. The bridge
    /// treats an unknown id as a no-op ("already done" == "never existed").
    pub fn cancel(&self, bridge_id: &str) {
        let _ = self.write_line(&json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "op": "cancel",
            "target": bridge_id,
        }));
    }

    fn write_line(&self, value: &serde_json::Value) -> Result<(), BridgeError> {
        let mut stdin = self.stdin.lock().expect("bridge stdin lock");
        serde_json::to_writer(&mut *stdin, value)
            .map_err(|e| BridgeError::Unavailable(format!("bridge stdin: {e}")))
            .and_then(|()| {
                stdin
                    .write_all(b"\n")
                    .map_err(|e| BridgeError::Unavailable(format!("bridge stdin: {e}")))
            })
            .and_then(|()| {
                stdin
                    .flush()
                    .map_err(|e| BridgeError::Unavailable(format!("bridge stdin: {e}")))
            })
    }

    /// Whether the child is still alive (best-effort; the reader's
    /// fail-all-pending is the authoritative signal).
    #[allow(dead_code)]
    pub fn alive(&self) -> bool {
        self.child
            .lock()
            .expect("bridge child lock")
            .try_wait()
            .map(|status| status.is_none())
            .unwrap_or(false)
    }
}

/// Route one parsed stdout line to its pending sender (or cache capabilities
/// in-flight via a synthetic Final — see `capabilities()`).
fn route_line(
    pending: &Arc<Mutex<HashMap<String, mpsc::Sender<BridgeEvent>>>>,
    id: Option<String>,
    value: serde_json::Value,
) {
    // The bridge's capabilities reply carries the payload as top-level keys,
    // not as a Final event — normalize it here so `capabilities()` can treat
    // one event shape.
    let event = if value.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        BridgeEvent::Final {
            text: value.to_string(),
            tier: "capabilities".into(),
            turn: 0,
            input_tokens: 0,
            output_tokens: 0,
            abstain: false,
            claims: Vec::new(),
        }
    } else {
        match value.get("event").and_then(|v| v.as_str()) {
            Some("delta") => BridgeEvent::Delta {
                text: value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                replace: value
                    .get("replace")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            Some("final") => BridgeEvent::Final {
                text: value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                tier: value
                    .get("tier")
                    .and_then(|v| v.as_str())
                    .unwrap_or("on_device")
                    .to_owned(),
                turn: value.get("turn").and_then(|v| v.as_i64()).unwrap_or(0),
                input_tokens: value
                    .get("inputTokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                output_tokens: value
                    .get("outputTokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0),
                // ADDITIVE (LL-04 §1.1): absent on an old bridge → the
                // untyped posture (no abstention, no claims), never a parse
                // error.
                abstain: value
                    .get("abstain")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                claims: value
                    .get("claims")
                    .and_then(|v| serde_json::from_value::<Vec<BridgeClaim>>(v.clone()).ok())
                    .unwrap_or_default(),
            },
            Some("error") => BridgeEvent::Failed {
                code: value
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned(),
                message: value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                shortfall_tokens: value.get("shortfallTokens").and_then(|v| v.as_i64()),
                feedback: value
                    .get("feedback")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
            },
            _ => return,
        }
    };

    let terminal = !matches!(event, BridgeEvent::Delta { .. });
    let Some(id) = id else { return };
    let mut pending = pending.lock().expect("bridge pending lock");
    if let Some(tx) = pending.get(&id) {
        let _ = tx.try_send(event);
    }
    if terminal {
        pending.remove(&id);
    }
}

fn fail_all_pending(
    pending: &Arc<Mutex<HashMap<String, mpsc::Sender<BridgeEvent>>>>,
    code: &str,
    message: &str,
) {
    let mut pending = pending.lock().expect("bridge pending lock");
    for (_, tx) in pending.drain() {
        let _ = tx.try_send(BridgeEvent::Failed {
            code: code.to_owned(),
            message: Some(message.to_owned()),
            shortfall_tokens: None,
            feedback: None,
        });
    }
}

/// The candidate locations of the built bridge child (the shipped-bundle
/// layout first, then the plain swift-build layout). ONE list, shared by
/// the chat spawn path's callers and every lane that drives the real child,
/// so a build-layout change moves all of them together.
pub(crate) fn bridge_path_candidates() -> Vec<std::path::PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    vec![
        repo_root.join("shell-macos/.build/out/Products/Debug/brain-afm"),
        repo_root.join("shell-macos/.build/arm64-apple-darwin/debug/brain-afm"),
    ]
}

/// The process-wide chat bridge, spawned from the configured path and
/// RESPAWNED when the child dies: a killed `brain-afm` must not 503 the
/// rest of the conversation — the next turn lands on a fresh child and
/// re-seeds its session from the persisted transcript. `None` means "no
/// bridge binary here" — the chat route's named refusal, never a crash:
/// every other surface of the server is unaffected.
static CHAT_BRIDGE: std::sync::RwLock<Option<Arc<AfmBridge>>> = std::sync::RwLock::new(None);

pub fn chat_bridge() -> Option<Arc<AfmBridge>> {
    if let Some(bridge) = CHAT_BRIDGE.read().expect("chat bridge lock").clone()
        && bridge.alive()
    {
        return Some(bridge);
    }
    // The write lock serializes respawns: two racing turns spawn at most
    // one replacement child (the loser re-checks under the lock).
    let mut guard = CHAT_BRIDGE.write().expect("chat bridge lock");
    if let Some(bridge) = guard.clone()
        && bridge.alive()
    {
        return Some(bridge);
    }
    let fresh = match crate::config::afm_bridge_path() {
        Some(path) if path.is_file() => AfmBridge::spawn(&path)
            .map_err(|e| {
                tracing::warn!("chat bridge spawn failed ({e}); chat turns will refuse by name");
                e
            })
            .ok(),
        _ => {
            tracing::info!("no AFM bridge binary; chat turns answer chat_bridge_unavailable");
            None
        }
    };
    *guard = fresh.clone();
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned protocol: drive the REAL bridge binary when it is built
    /// (the suite's own build produces it), asserting the wire contract the
    /// handler depends on. Skipped-with-prejudice when the binary is absent
    /// — the shell-macos suite owns the binary's own pins.
    #[tokio::test]
    async fn the_real_bridge_answers_capabilities_and_a_refusal_by_name() {
        let candidates = bridge_path_candidates();
        let Some(binary) = candidates.iter().find(|p| p.is_file()) else {
            eprintln!(
                "brain-afm not built; run `swift build` in shell-macos — SKIPPING (owned by the Swift suite)"
            );
            return;
        };
        let bridge = AfmBridge::spawn(binary).expect("spawn the real bridge");
        let caps = bridge.capabilities().await.expect("capabilities");
        assert_eq!(
            caps.window, 4096,
            "the on-device window is the measured 4096"
        );
        assert_eq!(
            caps.pcc_window,
            Some(32768),
            "the PCC window reads back 32,768 even while generation is refused"
        );
        // A turn BEFORE capabilities on a FRESH bridge child is refused by
        // name — spawn a second child to prove the ordering law.
        let fresh = AfmBridge::spawn(binary).expect("spawn fresh");
        let (_bridge_id, mut rx) = fresh
            .turn(BridgeTurn {
                session: "s".into(),
                prompt: "hi".into(),
                instructions: None,
                max_response_tokens: None,
                history: None,
                model: None,
                schema: None,
            })
            .await
            .expect("turn begins");
        match rx.recv().await {
            Some(BridgeEvent::Failed { code, .. }) => {
                assert_eq!(code, "bad_request", "the refusal is named");
            }
            other => panic!("expected the named refusal, got {other:?}"),
        }
    }

    /// THE TYPED MIRROR (LL-04 §1.1's kernel half): a final carrying the
    /// bridge's guided-generation fields parses into the typed event, and a
    /// final carrying NONE of them (an old bridge, pre-LL-04) parses
    /// additively — `false` / empty, never an error.
    #[test]
    fn the_typed_final_mirror_parses_and_defaults_additive() {
        let typed = serde_json::json!({
            "id": "t1", "event": "final",
            "text": "Green tea contains EGCG.",
            "tier": "on_device", "turn": 3,
            "inputTokens": 120, "outputTokens": 24,
            "abstain": false,
            "claims": [
                {"text": "Green tea contains EGCG.", "cites": [0, 2]},
                {"text": "It also carries L-theanine.", "cites": [1]}
            ],
        });
        let event = parse_final_line(typed);
        let BridgeEvent::Final {
            abstain, claims, ..
        } = event
        else {
            panic!("the typed final parses to a Final");
        };
        assert!(!abstain);
        assert_eq!(claims.len(), 2, "both claims cross verbatim");
        assert_eq!(claims[0].cites, vec![0, 2]);
        assert_eq!(claims[1].text, "It also carries L-theanine.");

        let abstaining = serde_json::json!({
            "id": "t2", "event": "final",
            "text": "", "tier": "on_device", "turn": 4,
            "inputTokens": 9, "outputTokens": 1,
            "abstain": true, "claims": [],
        });
        let BridgeEvent::Final {
            abstain, claims, ..
        } = parse_final_line(abstaining)
        else {
            panic!("the abstaining final parses to a Final");
        };
        assert!(abstain, "the model's abstention crosses");
        assert!(claims.is_empty(), "an abstention carries no claims");

        // The ADDITIVE law: an old bridge's final has neither key.
        let legacy = serde_json::json!({
            "id": "t3", "event": "final",
            "text": "prose only", "tier": "on_device", "turn": 5,
            "inputTokens": 1, "outputTokens": 2,
        });
        let BridgeEvent::Final {
            abstain, claims, ..
        } = parse_final_line(legacy)
        else {
            panic!("the legacy final parses to a Final");
        };
        assert!(!abstain, "absent abstain defaults false");
        assert!(claims.is_empty(), "absent claims default empty");
    }

    /// CG-10 E3, the model field's behavioral half: the field is
    /// additive-optional on the wire and the CHILD owns the gate. An unknown
    /// id is `bad_request` naming the id, refused at PARSE time — no
    /// registry slot, no model work, no capabilities needed beyond the
    /// ordering law — so this half is machine-independent. (Red-proof:
    /// damage the child's `resolveWireModel` to pass unknown ids through →
    /// this pin reds; revert → green.)
    ///
    /// AMENDED 2026-11-06 (the CG-11 roster amendment): the pin's second
    /// arm asserted `qwen3.5-9b-4bit` refuses `model_excluded` with the
    /// roster cause — and the ADMISSION of that id by measurement retired
    /// the exclusion, so this mirror redded on the next run (the mirrors
    /// doing their job again: a Swift-side roster flip redded the Rust
    /// assert). The excluded arm is GONE because there is no excluded id:
    /// the admission's machine-independent coverage lives on the Swift
    /// side (the routing pins + the allCases XOR law); if a FUTURE
    /// exclusion enters, its arm comes back HERE — parse-time, cause as
    /// the message — per the same law this pin has always held.
    #[tokio::test]
    async fn the_model_field_refuses_unknown_ids_by_name() {
        let candidates = bridge_path_candidates();
        let Some(binary) = candidates.iter().find(|p| p.is_file()) else {
            eprintln!(
                "brain-afm not built; run `swift build` in shell-macos — SKIPPING (owned by the Swift suite)"
            );
            return;
        };
        let bridge = AfmBridge::spawn(binary).expect("spawn the real bridge");
        bridge.capabilities().await.expect("capabilities first");

        // An unknown id: bad_request, and the refusal NAMES the id.
        let (_id, mut rx) = bridge
            .turn(BridgeTurn {
                session: "cg10-unknown-model".into(),
                prompt: "hi".into(),
                instructions: None,
                max_response_tokens: None,
                history: None,
                model: Some("no-such-model".into()),
                schema: None,
            })
            .await
            .expect("turn begins");
        match rx.recv().await {
            Some(BridgeEvent::Failed { code, message, .. }) => {
                assert_eq!(code, "bad_request", "an unknown model refuses by name");
                let said = message.unwrap_or_default();
                assert!(
                    said.contains("no-such-model"),
                    "the refusal must name the offending id, got: {said}"
                );
            }
            other => panic!("expected the named refusal, got {other:?}"),
        }
    }

    /// CG-10 E3, the model lock: a session's model is fixed at first use —
    /// the conversation lives in ONE ledger — and a mismatching later
    /// request refuses `model_locked` as a REGISTRY decision (machine
    /// posture cannot reorder it: the lock is decided before any
    /// materialization, so it holds even where the first tier could not
    /// materialize at all). (Red-proof: damage the lock comparison in the
    /// child's resolve → this pin reds; revert → green.)
    #[tokio::test]
    async fn a_session_model_is_locked_at_first_use() {
        let candidates = bridge_path_candidates();
        let Some(binary) = candidates.iter().find(|p| p.is_file()) else {
            eprintln!(
                "brain-afm not built; run `swift build` in shell-macos — SKIPPING (owned by the Swift suite)"
            );
            return;
        };
        let bridge = AfmBridge::spawn(binary).expect("spawn the real bridge");
        bridge.capabilities().await.expect("capabilities first");

        // First use WITHOUT the field: the AFM slot is born here (the
        // prewarm may answer ok or model_unavailable — machine posture —
        // but the SLOT exists either way, which is the registry-first law).
        let _ = bridge
            .prewarm(
                "cg10-locked-session",
                Some("You ground answers."),
                None,
                None,
            )
            .await;

        // The mismatching tier refuses, named, with BOTH ids.
        let locked = bridge
            .prewarm(
                "cg10-locked-session",
                Some("You ground answers."),
                None,
                Some("qwen3-8b-4bit"),
            )
            .await;
        match locked {
            Err(BridgeError::Protocol(text)) => {
                assert!(
                    text.contains("model_locked"),
                    "the mismatch refuses by name, got: {text}"
                );
                assert!(
                    text.contains("qwen3-8b-4bit") && text.contains("afm3-core"),
                    "the refusal names both sides of the mismatch, got: {text}"
                );
            }
            other => panic!("expected the model_locked refusal, got {other:?}"),
        }
    }

    /// THE CROSS-SURFACE DRIFT PIN: the Rust mirror (`BridgeClaim`,
    /// `BridgeEvent::Final`) and the Swift child's schema must name the
    /// SAME wire vocabulary. The Swift file is not compiled in this lane,
    /// so the pin reads the declaration out of the source tree — the same
    /// posture the docs-truth pins take with documents. A renamed Swift
    /// property or a renamed Rust field reds THIS pin, in THIS lane.
    #[test]
    fn the_swift_child_declares_the_typed_schema_the_mirror_parses() {
        let schema = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/shell-macos/Sources/AppleFMProbe/GroundedAnswer.swift"
        ))
        .expect("the Swift schema file is in the tree");
        for declaration in [
            "@Generable",
            "public struct GroundedAnswer",
            "public var abstain: Bool",
            "public var answer: String",
            "public var claims: [GroundedClaim]",
            "public struct GroundedClaim",
            "public var text: String",
            "public var cites: [Int]",
        ] {
            assert!(
                schema.contains(declaration),
                "the Swift schema drifted from the mirror: missing `{declaration}`"
            );
        }
        // And the bridge child still streams the typed shape end to end.
        let child = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/shell-macos/Sources/brain-afm/main.swift"
        ))
        .expect("the bridge child is in the tree");
        assert!(
            child.contains("generating: GroundedAnswer.self"),
            "the child must ask the model for the TYPED answer, not prose"
        );
        assert!(
            child.contains("\"abstain\": typed.abstain")
                && child.contains("\"claims\": typed.claims"),
            "the child's final event must carry the typed fields verbatim"
        );
    }

    /// PREWARM ROUND-TRIP (T-1): the `prewarm` op warms the session's
    /// model before its first turn, and an unknown op is refused BY NAME
    /// (the child's own default arm — the refusal is the contract).
    #[tokio::test]
    async fn the_real_bridge_answers_prewarm_and_refuses_unknown_ops_by_name() {
        let candidates = bridge_path_candidates();
        let Some(binary) = candidates.iter().find(|p| p.is_file()) else {
            eprintln!(
                "brain-afm not built; run `swift build` in shell-macos — SKIPPING (owned by the Swift suite)"
            );
            return;
        };
        let bridge = AfmBridge::spawn(binary).expect("spawn the real bridge");
        bridge.capabilities().await.expect("capabilities first");
        let warmed = bridge
            .prewarm(
                "pw-session",
                Some("You are a memory-grounded assistant."),
                None,
                None,
            )
            .await
            .expect("prewarm round-trips");
        assert!(warmed, "the prewarm op answers ok");
        // The unknown-op refusal is named by the child's default arm.
        let (tx, mut rx) = mpsc::channel::<BridgeEvent>(16);
        let id = "unknown-op-probe".to_string();
        bridge
            .pending
            .lock()
            .expect("bridge pending lock")
            .insert(id.clone(), tx);
        bridge
            .write_line(&json!({ "id": id, "op": "frobnicate" }))
            .expect("write the unknown op");
        match rx.recv().await {
            Some(BridgeEvent::Failed { code, message, .. }) => {
                assert_eq!(code, "bad_request", "the refusal is named");
                assert!(
                    message
                        .as_deref()
                        .unwrap_or_default()
                        .contains("unknown op frobnicate"),
                    "the refusal names the op: {message:?}"
                );
            }
            other => panic!("expected the named refusal, got {other:?}"),
        }
    }

    /// UNSEEDABLE HISTORY IS REFUSED BY NAME (T-3): a fresh session whose
    /// seeded history cannot fit the measured budget never starts — the
    /// refusal is named (`history_unseedable`), and the transcript is
    /// unchanged (the session still answers a normal turn afterwards).
    #[tokio::test]
    async fn the_real_bridge_refuses_unseedable_history_by_name() {
        let candidates = bridge_path_candidates();
        let Some(binary) = candidates.iter().find(|p| p.is_file()) else {
            eprintln!(
                "brain-afm not built; run `swift build` in shell-macos — SKIPPING (owned by the Swift suite)"
            );
            return;
        };
        let bridge = AfmBridge::spawn(binary).expect("spawn the real bridge");
        bridge.capabilities().await.expect("capabilities first");
        // A fresh session seeded with far more transcript than the 2,710
        // budget can hold: the child must refuse by name, not truncate
        // silently into amnesia.
        let huge = "x".repeat(40_000);
        let (tx, mut rx) = mpsc::channel::<BridgeEvent>(64);
        let id = "oversize-seed-probe".to_string();
        bridge
            .pending
            .lock()
            .expect("bridge pending lock")
            .insert(id.clone(), tx);
        bridge
            .write_line(&json!({
                "id": id,
                "op": "turn",
                "session": "seed-too-big",
                "prompt": "hello",
                "instructions": "You are a memory-grounded assistant.",
                "history": [{"role": "user", "text": huge}],
            }))
            .expect("write the oversize turn");
        match rx.recv().await {
            Some(BridgeEvent::Failed { code, .. }) => {
                assert_eq!(
                    code, "history_unseedable",
                    "the oversize seed is refused by name"
                );
            }
            other => panic!("expected the named refusal, got {other:?}"),
        }
        // Transcript unchanged: the same session key still serves a
        // normal turn (the refusal poisoned nothing).
        let (_bridge_id, mut rx2) = bridge
            .turn(BridgeTurn {
                session: "seed-too-big".into(),
                prompt: "Green tea is rich in EGCG. What is green tea?".into(),
                instructions: Some("You are a memory-grounded assistant.".into()),
                max_response_tokens: Some(64),
                history: None,
                model: None,
                schema: None,
            })
            .await
            .expect("turn begins");
        let mut saw_final = false;
        while let Some(ev) = rx2.recv().await {
            if matches!(ev, BridgeEvent::Final { .. }) {
                saw_final = true;
                break;
            }
        }
        assert!(saw_final, "the refused seed left the session usable");
    }

    /// One `event: final` JSON line through the real parser path.
    fn parse_final_line(value: serde_json::Value) -> BridgeEvent {
        let pending: Arc<Mutex<HashMap<String, mpsc::Sender<BridgeEvent>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, mut rx) = mpsc::channel::<BridgeEvent>(4);
        let id = value
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("t")
            .to_owned();
        pending.lock().expect("pending lock").insert(id.clone(), tx);
        route_line(&pending, Some(id), value);
        match rx.try_recv() {
            Ok(event) => event,
            Err(e) => panic!("the parser dropped the final line: {e}"),
        }
    }

    /// THE S6 MARKER CROSSES THE BOUNDARY (the drift the live run caught):
    /// an error line carrying `feedback:"attached"` parses with the marker
    /// INTACT — the kernel's audit copy sees the attachment, not a silently
    /// dropped field. An error line WITHOUT the field (an old bridge)
    /// parses `None` — the additive law, never a parse error.
    #[test]
    fn failed_events_carry_the_feedback_marker_and_default_to_none() {
        let with = parse_final_line(serde_json::json!({
            "id": "e1", "event": "error", "code": "generation_failed",
            "message": "May contain sensitive content", "shortfallTokens": null,
            "feedback": "attached"
        }));
        let BridgeEvent::Failed {
            code,
            feedback,
            shortfall_tokens,
            ..
        } = with
        else {
            panic!("an error line must parse as Failed");
        };
        assert_eq!(code, "generation_failed");
        assert_eq!(feedback.as_deref(), Some("attached"));
        assert_eq!(shortfall_tokens, None);

        let without = parse_final_line(serde_json::json!({
            "id": "e2", "event": "error", "code": "conversation_does_not_fit",
            "shortfallTokens": 1234
        }));
        let BridgeEvent::Failed {
            feedback,
            shortfall_tokens,
            ..
        } = without
        else {
            panic!("the legacy error line must parse as Failed");
        };
        assert_eq!(
            feedback, None,
            "an old bridge carries no marker — additive default"
        );
        assert_eq!(shortfall_tokens, Some(1234));
    }
}
