//! The hand-down act surface: `POST /workflow/hand-down`.
//!
//! Admin-only, audit-per-call. The handler parses typed JSON only and
//! calls the domain act; no gate logic lives here. The conformance
//! receipt is the whole precondition — the kernel never reads the
//! sibling pack.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;

use crate::server::bootstrap::AppState;
use crate::workflow::hand_down::{ConformanceReceipt, hand_down_in_tx};

use super::{HandlerError, authorize};

/// The act request: the source run plus the conformance receipt.
/// Unknown fields are refused by the typed decode.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HandDownRequest {
    pub procedure_id: i64,
    pub conformance: ConformanceReceipt,
}

/// `POST /workflow/hand-down` — mark a resolved procedure as
/// executable by the lowest tier, behind the conformance precondition.
pub(crate) async fn post_hand_down(
    State(state): State<Arc<AppState>>,
    principal: crate::handlers::auth::OptPrincipal,
    Json(body): Json<HandDownRequest>,
) -> Result<Json<serde_json::Value>, HandlerError> {
    let principal = principal.0;
    authorize(&principal, crate::auth::Action::Admin, "", "global")?;
    if body.procedure_id <= 0 {
        return Err(HandlerError::bad_request(
            "hand_down_refused",
            "procedure_id must be positive",
        ));
    }
    let pool = state.pool.clone();
    let procedure_id = body.procedure_id;
    let conformance = body.conformance;
    let result = tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| format!("{e}"))?;
        let mut tx =
            crate::workflow::tx::WorkflowTx::begin(&mut conn).map_err(|e| format!("{e}"))?;
        let result = hand_down_in_tx(tx.tx(), procedure_id, &conformance);
        match result {
            Ok(r) => {
                tx.commit().map_err(|e| format!("{e}"))?;
                Ok(r)
            }
            Err(e) => Err(e.0),
        }
    })
    .await
    .map_err(|e| HandlerError::internal(format!("{e}")))?;
    let result =
        result.map_err(|msg: String| HandlerError::unprocessable("hand_down_refused", msg))?;
    let mark = result.mark();
    Ok(Json(serde_json::json!({
        "procedure_id": mark.procedure_id,
        "conformance_run_id": mark.conformance.run_id,
        "snapshot_digest": mark.snapshot_digest,
        "created": result.is_created(),
    })))
}
