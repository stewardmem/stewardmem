//! The chat router — the chat spine's own wire tables (the agentloop law:
//! a loop-serving route ships its wire in its own release).
//!
//! `POST /chat/turns` — one memory-grounded, AFM-generated chat turn as an
//! SSE stream (`grounding`, `delta`*, `final` | `error`). See
//! `handlers::chat` for the turn contract.
//!
//! S4's live witness (round 21) SPLIT the turn route out of [`router`]: the
//! cascade's writer turn is minutes-scale when honest, and the handler
//! awaits the writer before the SSE response returns — the shared 30 s wall
//! 408'd every cold-tier writer turn (three live witnesses, three 408s at
//! exactly 30 s, zero bytes on the wire). The turn route merges OUTSIDE the
//! shared timeout via [`turn_router`], the IMPROVE06 document-ingest
//! precedent, with its own named budget
//! (`config::CHAT_TURN_TIMEOUT_SECS`). The session-store routes stay inside
//! the shared wall (fast reads).

use axum::Router;
use std::sync::Arc;

use axum::routing::{delete, get, patch, post};

use crate::AppState;

/// The turn route's own router, merged AFTER the shared timeout in mod.rs —
/// the document_ingest_router shape verbatim: the route carries its OWN
/// budget (`config::CHAT_TURN_TIMEOUT_SECS`); everything layered after the
/// merge point in mod.rs (CatchPanic, the RBAC route_layer, auth) still
/// covers it — `.layer()` applies to routes registered so far, and the
/// merge sits before them in source.
pub fn turn_router() -> Router<Arc<AppState>> {
    use axum::http::StatusCode;
    use std::time::Duration;
    Router::new()
        .route("/chat/turns", post(crate::handlers::chat::chat_turn))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(crate::config::CHAT_TURN_TIMEOUT_SECS),
        ))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        // The chat session store (the chat spine's durable store). One registration PER
        // METHOD (the /notes shape), so the authz matrix's registration
        // scan sees both methods. DELETE registers LAST: the matrix pairs a
        // path's first table row (Read, DELETE's own gate) with the
        // last-registered method.
        .route(
            "/chat/sessions",
            get(crate::handlers::chat_sessions::list_sessions),
        )
        .route(
            "/chat/sessions/{session}/turns",
            get(crate::handlers::chat_sessions::list_turns),
        )
        .route(
            "/chat/sessions/{session}",
            patch(crate::handlers::chat_sessions::rename_session),
        )
        .route(
            "/chat/sessions/{session}",
            delete(crate::handlers::chat_sessions::delete_session),
        )
}
