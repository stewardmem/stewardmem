//! The journey fetch core (IMPROVE15) — ALL the journey surface's SQL, so
//! the handlers stay protocol adapters (the no-SQL-in-handlers gate's law).
//! The projection itself stays PURE in [`crate::workflow::journey`] (rows
//! in, events out); THIS module is the composition seam: it fetches the
//! chain rows on a caller-supplied connection and feeds the projection.
//!
//! The wire types live here (the `ProposalView` precedent: the queue's view
//! type lives in its own service core, and the handler re-exports it) — the
//! handler renders them, the core defines them.

use rusqlite::{Connection, OptionalExtension, params};

use crate::workflow::journey::{
    AppealFact, ApprovalFact, DsarFact, HoldFact, JourneyChain, JourneyEvent, JourneyRun,
};

/// The fetch core's error: a SQL failure is INTERNAL at the transport; the
/// core stays transport-blind (the handler maps it).
#[derive(Debug)]
pub struct JourneyError(pub String);

/// One projected case's journey block.
#[derive(Debug, serde::Serialize)]
pub struct JourneyCase {
    pub case_id: i64,
    pub status: String,
    pub opened_at: i64,
    pub events: Vec<JourneyEvent>,
}

/// The journey response: the client's identity block + one entry per case +
/// the client's OWN acts (holds, DSARs, the appeals — rendered even when the
/// client has no cases: an appeal filed before any case exists is exactly
/// the "received." the customer must see).
#[derive(Debug, serde::Serialize)]
pub struct JourneyResponse {
    pub client: String,
    pub domain: String,
    pub cases: Vec<JourneyCase>,
    pub domain_events: Vec<JourneyEvent>,
}

/// THE APPEAL'S DIGEST TARGET (the digest law's storage half): does a
/// DECIDED decision in this domain answer to the named digest? A streaming
/// scan with early exit over the domain's approved rows — the digest is the
/// same `review_digest` the approve law bound (you appeal bytes that were
/// shown and disposed, not bytes you name into existence). A rare customer
/// act; the scan is one pass.
pub fn appeal_target_answers(
    conn: &Connection,
    domain: &str,
    digest: &str,
) -> Result<bool, JourneyError> {
    let mut stmt = conn
        .prepare("SELECT content FROM proposals WHERE status = 'approved' AND domain = ?1")
        .map_err(|e| JourneyError(e.to_string()))?;
    let it = stmt
        .query_map(params![domain], |r| r.get::<_, String>(0))
        .map_err(|e| JourneyError(e.to_string()))?;
    for row in it {
        let row_content = row.map_err(|e| JourneyError(e.to_string()))?;
        if crate::handlers::gate::review_digest(&row_content) == digest {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The per-case fetch + project, on ONE domain-pool connection (the day-zero
/// scoping: the journey reads ONLY the client's own pool — there is no path
/// by which another client's rows enter this fn). Every input is a
/// chain-provable row; the projection is the PURE kernel fn. `case_filter`
/// narrows to one case id — a run outside the domain simply does not match
/// the WHERE, which IS the existence-blind refusal at the case grain.
pub fn journey_cases(
    conn: &Connection,
    domain: &str,
    client: &str,
    case_filter: Option<i64>,
) -> Result<JourneyResponse, JourneyError> {
    // The client's cases: kind='troubleshoot' runs in THIS domain (the GDL
    // case kind), oldest first.
    let mut stmt = conn
        .prepare(
            "SELECT id, created_at, updated_at, status, state_json FROM workflow_runs
              WHERE domain = ?1 AND kind = 'troubleshoot' AND (?2 IS NULL OR id = ?2)
              ORDER BY id ASC",
        )
        .map_err(|e| JourneyError(e.to_string()))?;
    let runs: Vec<(i64, i64, i64, String, String)> = stmt
        .query_map(params![domain, case_filter], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| JourneyError(e.to_string()))?
        .filter_map(Result::ok)
        .collect();
    drop(stmt);

    // The domain-level acts, read ONCE (they join every case render as
    // case_id 0 events):
    // the digest-bound review acts: approved proposals ⋈ the HITL ledger.
    let mut approvals: Vec<ApprovalFact> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT h.proposal_id, h.ts, h.actor FROM hitl_decision_events h
                  JOIN proposals p ON p.id = h.proposal_id
                  WHERE h.decision = 'approve' AND p.status = 'approved' AND p.domain = ?1",
            )
            .map_err(|e| JourneyError(e.to_string()))?;
        let it = stmt
            .query_map(params![domain], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| JourneyError(e.to_string()))?;
        for row in it.filter_map(Result::ok) {
            let (proposal_id, ts, actor) = row;
            // The receipt IS the approve-bound digest: the same
            // `review_digest` the approve law bound the decision to.
            let content: Option<String> = conn
                .query_row(
                    "SELECT content FROM proposals WHERE id = ?1 AND status = 'approved'",
                    params![proposal_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(content) = content {
                approvals.push(ApprovalFact {
                    proposal_id,
                    decided_at: ts,
                    digest: crate::handlers::gate::review_digest(&content),
                    actor,
                });
            }
        }
    }
    // the legal holds (the client's data-level acts).
    let mut holds: Vec<HoldFact> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, held_at, released_at, held_by FROM legal_holds ORDER BY id ASC")
            .map_err(|e| JourneyError(e.to_string()))?;
        let it = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(|e| JourneyError(e.to_string()))?;
        for row in it.filter_map(Result::ok) {
            let (id, held_at, released_at, held_by) = row;
            holds.push(HoldFact {
                id,
                held_at,
                released_at,
                actor: held_by.unwrap_or_default(),
            });
        }
    }
    // the completed data requests, their receipt = the certificate's
    // chain_head (the chain-provable digest the certificate itself anchors).
    let mut dsars: Vec<DsarFact> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT id, completed_at, certificate FROM dsar_requests
                  WHERE status = 'completed' ORDER BY id ASC",
            )
            .map_err(|e| JourneyError(e.to_string()))?;
        let it = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| JourneyError(e.to_string()))?;
        for row in it.filter_map(Result::ok) {
            let (id, completed_at, certificate) = row;
            let Some(completed_at) = completed_at else {
                continue;
            };
            let chain_head: Option<String> = certificate
                .as_deref()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(c).ok())
                .and_then(|v| {
                    v.get("chain_head")
                        .and_then(|h| h.as_str())
                        .map(String::from)
                });
            dsars.push(DsarFact {
                id,
                completed_at,
                certificate_digest: chain_head,
                actor: "dsar".to_string(),
            });
        }
    }
    // the domain's appeals: kind='appeal' proposals ⋈ their HITL disposals
    // (received at the filing; decided at the ledger's ts — the customer's
    // "received." / "decided." events, no reversal rendered).
    let mut appeals: Vec<AppealFact> = Vec::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT p.id, p.created_at, h.ts FROM proposals p
                  LEFT JOIN hitl_decision_events h
                    ON h.proposal_id = p.id AND h.decision IN ('approve','reject')
                  WHERE p.kind = 'appeal' AND p.domain = ?1
                  ORDER BY p.id ASC",
            )
            .map_err(|e| JourneyError(e.to_string()))?;
        let it = stmt
            .query_map(params![domain], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            })
            .map_err(|e| JourneyError(e.to_string()))?;
        for row in it.filter_map(Result::ok) {
            let (proposal_id, created_at, decided_at) = row;
            appeals.push(AppealFact {
                proposal_id,
                created_at,
                decided_at,
            });
        }
    }

    let mut cases: Vec<JourneyCase> = Vec::new();
    for (run_id, created_at, updated_at, status, state_json) in runs {
        // the run's journal checkpoints + case/decision rows (chain rows).
        let checkpoints: Vec<(i64, i64, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT seq, created_at, payload_json FROM agent_session_events
                      WHERE run_id = ?1 AND kind = 'control:gdl' ORDER BY seq ASC",
                )
                .map_err(|e| JourneyError(e.to_string()))?;
            let it = stmt
                .query_map(params![run_id], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| JourneyError(e.to_string()))?;
            it.filter_map(Result::ok).collect()
        };
        let decisions: Vec<(i64, i64, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, created_at, payload_json FROM outbox
                      WHERE run_id = ?1 AND topic = 'case/decision' ORDER BY id ASC",
                )
                .map_err(|e| JourneyError(e.to_string()))?;
            let it = stmt
                .query_map(params![run_id], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(|e| JourneyError(e.to_string()))?;
            it.filter_map(Result::ok).collect()
        };
        let chain = JourneyChain {
            run: JourneyRun {
                run_id,
                created_at,
                updated_at,
                status: &status,
                state_json: &state_json,
            },
            checkpoints: &checkpoints,
            decisions: &decisions,
            approvals: &approvals,
            holds: &holds,
            dsars: &dsars,
            appeals: &appeals,
        };
        cases.push(JourneyCase {
            case_id: run_id,
            status: status.clone(),
            opened_at: created_at,
            events: crate::workflow::journey::project(&chain),
        });
    }
    // the DOMAIN half: one projection, the client's own acts — rendered
    // independent of the case list (an appeal filed before any case exists
    // is exactly the "received." the customer must see).
    let domain_events = crate::workflow::journey::project_domain(&appeals, &holds, &dsars);
    Ok(JourneyResponse {
        client: client.to_string(),
        domain: domain.to_string(),
        cases,
        domain_events,
    })
}

/// The existence-blind refusal's case-grain check, used by the handler for
/// `?case=` — kept beside the fetch it narrows.
pub fn case_of_domain(conn: &Connection, domain: &str, case_id: i64) -> Result<bool, JourneyError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM workflow_runs WHERE id = ?1 AND domain = ?2 LIMIT 1",
            params![case_id, domain],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| JourneyError(e.to_string()))?
        .is_some())
}
