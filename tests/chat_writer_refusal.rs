//! The writer-refusal pins (plan 18 §2 — the bridge-500's kernel-side half).
//!
//! The S4-amendment incident (`111c7bb9`'s OWED record): a writer-protocol
//! break (the kernel's handshake meeting a bridge that refuses or dies) has
//! to end the turn in the chat contract's NAMED-refusal vocabulary — the SSE
//! `error` frame the spine parses (`generation_failed` is the child's own
//! code; `bridge_exited` is the reader's name for a dead child) or the named
//! 503 posture — never a bare body, and never the masquerade: the cascade
//! writer drain treating a `Failed` terminal (or a closed channel) as an
//! EMPTY ANSWER stamped `mlx_native`.
//!
//! The lane is SELF-CONTAINED: it does not need the real brain-afm child.
//! The stub below is a session-keyed python3 script speaking the child's
//! line protocol — `capabilities` answers the measured shape, `prewarm`
//! finals, a `cascade-route-*` session finals the guided route, a
//! `cascade-chat-*` session fails BY NAME (`generation_failed`), the
//! `cascade-chat-exit-drill` session kills the child (the hardest break:
//! the process dies mid-turn), and any other session (the fall-through's
//! main writer) fails by name too. Because the kernel's bridge is a
//! process-global spawned from `BRAIN_AFM_BRIDGE`, the pins live in their
//! OWN test target (their own process): the stub is the only bridge this
//! process ever spawns, and no other lane's real-bridge posture can leak
//! in — nor can this stub leak out. The env window is serialized behind a
//! file-local mutex (the `CascadeGateEnv` shape), and a pin that meets a
//! child killed by an earlier pin's exit drill simply respawns it from the
//! same env: the stub's behavior is keyed on the SESSION, not the process.

mod common;

use axum::body::Body;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tower::ServiceExt;

const OP_TOKEN: &str = "r18-writer-refusal-token";
const GREEN_TEA: &str = "Green tea is made from Camellia sinensis leaves that are steamed soon after picking, which preserves catechins like EGCG. Its benefits include antioxidant support, improved metabolic markers, and a milder caffeine lift than coffee.";

static GATE_LOCK: Mutex<()> = Mutex::new(());
static STUB_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// The stub child: session-keyed, self-contained. Written once to the temp
/// dir; `chmod 0o755` makes it spawnable as `BRAIN_AFM_BRIDGE`. `None` =
/// python3 is absent (the shebang cannot resolve) — the pins skip WITH
/// THEIR NAME (the house convention); they never vacuous-pass, because a
/// spawn failure answers `chat_bridge_unavailable`, which pins 1-2 refuse.
fn stub_bridge() -> Option<&'static PathBuf> {
    STUB_PATH
		.get_or_init(|| {
			let probe = std::process::Command::new("python3")
				.arg("-c")
				.arg("print(1)")
				.output();
			let Ok(output) = probe else {
				eprintln!("python3 is absent — the stub bridge cannot run; SKIPPING (the stub owns these pins)");
				return None;
			};
			if !output.status.success() {
				eprintln!("python3 is absent — the stub bridge cannot run; SKIPPING (the stub owns these pins)");
				return None;
			}
			let path = std::env::temp_dir().join(format!(
				"brain-afm-writer-refusal-stub-{}.py",
				std::process::id()
			));
			std::fs::write(
				&path,
				r#"#!/usr/bin/env python3
import sys, json, os

def reply(line):
    try:
        req = json.loads(line)
    except Exception:
        return
    rid = req.get("id", "")
    op = req.get("op", "")
    if op == "capabilities":
        print(json.dumps({"id": rid, "ok": True, "window": 4096, "budget": 4096,
                          "languages": ["en"], "pccWindow": 4096, "pccEligible": True,
                          "pccGeneration": "stub", "escalationPermitted": False}), flush=True)
        return
    if op == "prewarm":
        print(json.dumps({"id": rid, "event": "final", "text": "",
                          "inputTokens": 0, "outputTokens": 0}), flush=True)
        return
    if op == "turn":
        session = req.get("session", "")
        if session.startswith("cascade-route-"):
            guided = json.dumps({"route": "synthesize", "confidence": 0.9})
            print(json.dumps({"id": rid, "event": "final", "text": guided,
                              "inputTokens": 1, "outputTokens": 1}), flush=True)
            return
        if session.startswith("cascade-chat-exit-drill"):
            os._exit(9)
        print(json.dumps({"id": rid, "event": "error", "code": "generation_failed",
                          "message": "the stub child refused the writer turn"}), flush=True)
        return

for line in sys.stdin:
    reply(line)
"#,
			)
			.expect("write the stub bridge script");
			let mut perms = std::fs::metadata(&path).expect("stub metadata").permissions();
			use std::os::unix::fs::PermissionsExt;
			perms.set_mode(0o755);
			std::fs::set_permissions(&path, perms).expect("chmod the stub");
			Some(path)
		})
		.as_ref()
}

/// The env window: the gate ARMED and the bridge pinned to the stub. Both
/// ride the same mutex-guarded window; both are removed on drop.
struct StubBridgeEnv;

impl StubBridgeEnv {
    fn set(stub: &PathBuf) -> (MutexGuard<'static, ()>, Self) {
        let guard = GATE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        unsafe {
            std::env::set_var("BRAIN_CHAT_CASCADE", "1");
            std::env::set_var("BRAIN_AFM_BRIDGE", stub);
        }
        (guard, Self)
    }
}

impl Drop for StubBridgeEnv {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var("BRAIN_CHAT_CASCADE");
            std::env::remove_var("BRAIN_AFM_BRIDGE");
        }
    }
}

async fn stub_app_with_memory(
    memory: &str,
) -> (std::sync::Arc<brain_server::AppState>, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mgr = SqliteConnectionManager::file(&db_path);
    let pool: brain_server::Pool = r2d2::Pool::builder().max_size(4).build(mgr).expect("pool");
    brain_server::migration::run_migration(
        &mut pool.get().expect("conn"),
        brain_server::config::DB_MMAP_SIZE_MIB,
    )
    .expect("migration");
    let model: std::sync::Arc<dyn brain_server::embed::Embedder> = std::sync::Arc::new(
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model"),
    );
    let tok_file = dir.path().join("tokens");
    std::fs::write(&tok_file, format!("{OP_TOKEN}\n")).expect("token file");
    let token_store = brain_server::auth::TokenStore::from_file(Some(tok_file));
    token_store.reload_parts_from(vec![OP_TOKEN.to_string()], None);
    let state = std::sync::Arc::new(brain_server::AppState {
        token_store,
        jwt_middleware_state: std::sync::Arc::new(JwtMiddlewareState::opaque_for_tests(
            pool.clone(),
            db_path.clone(),
        )),
        cors: tower_http::cors::CorsLayer::new(),
        chat_answer_cache: std::sync::Arc::new(
            brain_server::service::chat_answer_cache::ChatAnswerCache::new(),
        ),
        durability: Default::default(),
        loom: Default::default(),
        model,
        registry: brain_server::domain_registry::DomainRegistry::new(pool.clone(), &db_path, false),
        pool: pool.clone(),
        db_path: db_path.clone(),
        connection_tracker: std::sync::Arc::new(brain_server::http_limit::ConnectionTracker::new()),
        rate_limiter: std::sync::Arc::new(brain_server::http_limit::RateLimiter::new()),
        snapshot: brain_server::integrity::SnapshotState::default(),
        audit_chain_cache: std::sync::Arc::new(std::sync::Mutex::new(None)),
        auth_mode: brain_server::auth::AuthMode::Opaque,
        key_store: brain_server::auth::jwks::KeyStore::default(),
        revocation_cache: std::sync::Arc::new(
            brain_server::auth::revocation::RevocationCache::new(),
        ),
        jwt_issuer: String::new(),
        jwt_audience: String::new(),
        oidc_config: brain_server::handlers::well_known::OidcConfig::unconfigured(),
        ump_events: tokio::sync::broadcast::channel(brain_server::config::UMP_EVENT_BUFFER).0,
        alert_events: tokio::sync::broadcast::channel(brain_server::config::ALERT_EVENT_BUFFER).0,
        alert_seq: std::sync::atomic::AtomicU64::new(0),
        chain_watch: brain_server::alert::ChainWatchState::default(),
        concurrency: &brain_server::concurrency::CONCURRENCY,
    });
    let ingest = axum::http::Request::builder()
        .method("POST")
        .uri("/ingest/memory")
        .header("content-type", "text/plain")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(memory.to_string()))
        .expect("ingest request");
    let resp = app(state.clone()).oneshot(ingest).await.expect("oneshot");
    assert_eq!(
        resp.status(),
        axum::http::StatusCode::OK,
        "the memory must ingest"
    );
    (state, dir)
}

/// The turn's RAW shape: status + the parsed SSE events (a named 503 is a
/// legal terminal here — the refusal posture — so the status is returned,
/// never asserted-200 by the harness).
async fn post_turn_raw(
    state: std::sync::Arc<brain_server::AppState>,
    session: &str,
    message: &str,
) -> (axum::http::StatusCode, Vec<(String, serde_json::Value)>) {
    let turn = axum::http::Request::builder()
        .method("POST")
        .uri("/chat/turns")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .body(Body::from(format!(
            r#"{{"session":{session:?},"message":{message:?}}}"#
        )))
        .expect("chat request");
    let resp = app(state).oneshot(turn).await.expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 8 * 1024 * 1024)
        .await
        .expect("body")
        .to_vec();
    let text = String::from_utf8_lossy(&bytes);
    let mut events = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            current = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data: ")
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(rest)
        {
            events.push((current.clone(), v));
        } else if current.is_empty()
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
        {
            // A non-SSE body (the error envelope): recorded as the "json" event.
            events.push(("json".to_string(), v));
        }
    }
    (status, events)
}

/// The masquerade detector: a `final` stamped `mlx_native` whose text is
/// EMPTY is the failed-writer shape dressed as an answer — the exact
/// hidden-fallback class these pins exist to keep dead.
fn masquerades_as_answer(events: &[(String, serde_json::Value)]) -> bool {
    events.iter().any(|(name, v)| {
        name == "final"
            && v["tier"].as_str() == Some("mlx_native")
            && v["text"].as_str().map(str::is_empty).unwrap_or(false)
    })
}

/// The named-terminal detector: the turn ended in the contract's refusal
/// vocabulary — an SSE `error` frame carrying the child's own code, or the
/// named 503 posture (the envelope code is `recall_unavailable` — the
/// handler's one unavailable code — with the `chat_bridge_unavailable`
/// MARKER on the message: the exact shape the spine's posture table
/// classifies, the D1 contract).
fn named_terminal(status: axum::http::StatusCode, events: &[(String, serde_json::Value)]) -> bool {
    if status == axum::http::StatusCode::SERVICE_UNAVAILABLE {
        return events.iter().any(|(name, v)| {
            name == "json"
                && v["error"]["code"].as_str() == Some("recall_unavailable")
                && v["error"]["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("chat_bridge_unavailable"))
        });
    }
    events
        .iter()
        .any(|(name, v)| name == "error" && v["code"].as_str().is_some())
}

/// PIN 1 — the named-failure writer break: the stub's cascade writer arm
/// answers `Failed { code: "generation_failed" }` (the child's OWN refusal
/// vocabulary). The turn must carry that refusal — through the documented
/// loud fall-through to the existing flow, whose Failed relay is the named
/// SSE `error` frame — and never the masquerade.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)] // the env window spans the body's awaits (the house shape)
async fn a_failed_cascade_writer_turn_names_itself_never_masquerades() {
    let Some(stub) = stub_bridge() else { return };
    let (_lock, _env) = StubBridgeEnv::set(stub);
    let (state, _dir) = stub_app_with_memory(GREEN_TEA).await;
    let (status, events) =
        post_turn_raw(state, "default", "Summarize the green tea benefits.").await;
    assert!(
        !masquerades_as_answer(&events),
        "an empty mlx_native final is the failed writer dressed as an answer — events: {events:?}"
    );
    assert!(
        named_terminal(status, &events),
        "the writer's named refusal must surface (the SSE error frame with the child's code, \
		 or the named 503 posture) — status {status}, events: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(name, v)| name == "error" && v["code"].as_str() == Some("generation_failed")),
        "the child's own code must ride the refusal, not a generic fault — events: {events:?}"
    );
}

/// PIN 2 — the dead-child break: the stub's `cascade-chat-exit-drill` arm
/// KILLS the child mid-writer-turn, so the pending writer learns through
/// the reader's `fail_all_pending` (`bridge_exited`). The turn must end
/// NAMED — the fall-through meets the same dead child and answers the
/// named 503 posture, or the SSE carries the reader's `bridge_exited`
/// frame — never the masquerade, never a bare body.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn a_dead_cascade_writer_names_the_exit_never_masquerades() {
    let Some(stub) = stub_bridge() else { return };
    let (_lock, _env) = StubBridgeEnv::set(stub);
    let (state, _dir) = stub_app_with_memory(GREEN_TEA).await;
    let (status, events) = post_turn_raw(
        state,
        "exit-drill",
        "Summarize the green tea benefits, exit drill.",
    )
    .await;
    assert!(
        !masquerades_as_answer(&events),
        "an empty mlx_native final is the dead writer dressed as an answer — events: {events:?}"
    );
    assert!(
        named_terminal(status, &events),
        "the dead writer must end in the named vocabulary (bridge_exited / generation_failed / \
		 chat_bridge_unavailable) — status {status}, events: {events:?}"
    );
}

/// PIN 3 — the source law (the scan form, `common::code_only`'s
/// comment-stripped scan): the cascade writer drain must MAP the Failed
/// terminal through the named-drain outcome and the LOUD fall-through —
/// the masquerade's shape (a bare `while let` drain that only matches
/// `Final`) is what this pin keeps out of the tree.
#[test]
fn the_cascade_writer_drain_names_the_failed_terminal_in_source() {
    let src = common::code_only(include_str!("../src/handlers/chat.rs"));
    let drain_call = src
        .find(concat!("drain_", "writer_turn"))
        .expect("the cascade writer drain must run through the named-drain helper");
    let refused_arm = src
        .find("WriterDrain::Refused")
        .expect("the handler must map the writer's named refusal to the documented fall-through");
    let answer_arm = src
        .find("WriterDrain::Answer")
        .expect("the drain's answer shape must be the ONLY path to the mlx_native stamp");
    let stamp = src
        .find(r#""mlx_native""#)
        .expect("the writer tier stamp lives in the handler");
    assert!(
        drain_call < answer_arm && answer_arm < stamp,
        "the mlx_native stamp is only reachable through the drain's Answer arm — \
		 drain at {drain_call}, answer at {answer_arm}, stamp at {stamp}"
    );
    let _ = refused_arm; // the refusal arm's PRESENCE is the law (found above)
}
