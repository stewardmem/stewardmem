//! `portal` — the thin customer portal (IMPROVE15).
//!
//! THE AUTH DECISION (§3(c), preregistered): customer auth lives HERE, in
//! the portal — the kernel never becomes an identity provider (no customer
//! login endpoint, no customer passwords, no session store on the kernel).
//! The portal holds ONE per-client `client-auditor`-class credential (the
//! roster door's mint: `brain principal grant --name portal-{client} --role
//! client-auditor --domain {domain}` — read-only, ONE domain, tools empty by
//! default; the grant is enumerated in docs/DEPLOYMENT_PORTAL.md, §8.1's
//! access review). THE CREDENTIAL NEVER REACHES THE BROWSER: the token is a
//! process env held server-side, used only for the portal's server-side
//! kernel calls; the browser's only cookie is the portal's own session id.
//!
//! THE OFFLINE LAW (the III-6 reuse): when the kernel is unreachable, the
//! portal renders CACHED METADATA ONLY — counts and ages, never event text
//! (`src/portal/mod.rs`'s cache type has no field that could hold a
//! sentence).
//!
//! THE PROPOSE DOOR: every customer action files through the kernel's
//! customer doors (`POST /clients/{name}/appeals`,
//! `POST /clients/{name}/requests`) as a PROPOSAL — the portal writes
//! nothing itself, and nothing a customer does writes memory directly (the
//! operator disposes; nothing auto-reverses).

use askama::Template;
use axum::{
    Router,
    extract::{Form, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use std::sync::Arc;

use brain_server::portal::{
    CaseRow, EventRow, FiledPage, JourneyPage, LoginPage, MetadataCache, OfflinePage, PortalConfig,
    RefusedPage, SessionStore, now_secs, parse_journey, to_metadata,
};

struct PortalState {
    cfg: PortalConfig,
    http: reqwest::Client,
    sessions: SessionStore,
    cache: MetadataCache,
}

#[tokio::main]
async fn main() {
    let cfg = match PortalConfig::from_env() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("portal: {e}");
            std::process::exit(2);
        }
    };
    eprintln!(
        "portal: serving client `{}` on {} (kernel {}; the kernel credential stays server-side)",
        cfg.client, cfg.bind, cfg.kernel_url
    );
    let state = Arc::new(PortalState {
        cfg,
        http: reqwest::Client::new(),
        sessions: SessionStore::default(),
        cache: MetadataCache::default(),
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", post(logout))
        .route("/", get(journey))
        .route("/appeal", post(file_appeal))
        .route("/requests", post(file_request))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&read_bind())
        .await
        .expect("portal bind");
    axum::serve(listener, app).await.expect("portal serve");
}

fn read_bind() -> String {
    std::env::var("BRAIN_PORTAL_BIND")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:8766".to_string())
}

// ── the public surfaces ────────────────────────────────────────────────────

/// The portal's own liveness — the one public path (the kernel's
/// PUBLIC_PATHS law, portal-side: exact, explicit, nothing else).
async fn health() -> &'static str {
    "ok"
}

async fn login_page() -> Html<String> {
    Html(LoginPage { failed: false }.render().expect("template"))
}

#[derive(Debug, serde::Deserialize)]
struct LoginForm {
    passcode: String,
}

/// THE PORTAL'S OWN AUTH: the per-client passcode → an in-process session.
/// The kernel sees none of this (no login endpoint, no session store, no
/// password ever leaves this process).
async fn login_submit(State(st): State<Arc<PortalState>>, Form(form): Form<LoginForm>) -> Response {
    match st.sessions.login(&form.passcode, &st.cfg.passcode) {
        Some(session) => {
            let mut response = Redirect::to("/").into_response();
            // HttpOnly: the browser's script never reads the session id;
            // SameSite=Lax: the id rides only this portal's own navigation.
            response.headers_mut().insert(
                header::SET_COOKIE,
                format!("portal_session={session}; HttpOnly; SameSite=Lax; Path=/")
                    .parse()
                    .expect("cookie"),
            );
            response
        }
        None => Html(LoginPage { failed: true }.render().expect("template")).into_response(),
    }
}

async fn logout(State(st): State<Arc<PortalState>>, headers: axum::http::HeaderMap) -> Redirect {
    if let Some(session) = session_of(&headers) {
        st.sessions.logout(&session);
    }
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        "portal_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"
            .parse()
            .expect("cookie"),
    );
    Redirect::to("/login")
}

fn session_of(headers: &axum::http::HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for pair in cookie.split(';') {
        let pair = pair.trim();
        if let Some(id) = pair.strip_prefix("portal_session=") {
            return Some(id.to_string());
        }
    }
    None
}

/// THE SESSION GUARD: every non-public path verifies the in-process store.
/// A dead session redirects to the login page — the refusal is a redirect,
/// never a journey.
fn require_session(st: &PortalState, headers: &axum::http::HeaderMap) -> Option<()> {
    let session = session_of(headers)?;
    if st.sessions.verify(&session) {
        Some(())
    } else {
        None
    }
}

// ── the kernel proxy (the credential stays here) ──────────────────────────

async fn kernel_get(st: &PortalState, path: &str) -> Result<reqwest::Response, reqwest::Error> {
    st.http
        .get(format!("{}{path}", st.cfg.kernel_url))
        .bearer_auth(&st.cfg.kernel_token)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
}

async fn kernel_post(
    st: &PortalState,
    path: &str,
    body: serde_json::Value,
) -> Result<reqwest::Response, reqwest::Error> {
    st.http
        .post(format!("{}{path}", st.cfg.kernel_url))
        .bearer_auth(&st.cfg.kernel_token)
        .json(&body)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
}

fn html(page: impl askama::Template) -> Response {
    page.render().map_or(
        (StatusCode::INTERNAL_SERVER_ERROR, "template").into_response(),
        |body| Html(body).into_response(),
    )
}

// ── the journey (the offline law's home) ──────────────────────────────────

async fn journey(State(st): State<Arc<PortalState>>, headers: axum::http::HeaderMap) -> Response {
    if require_session(&st, &headers).is_none() {
        return Redirect::to("/login").into_response();
    }
    let path = format!("/clients/{}/journey", st.cfg.client);
    match kernel_get(&st, &path).await {
        Ok(resp) if resp.status() == StatusCode::OK => {
            let value = match resp.json::<serde_json::Value>().await {
                Ok(v) => v,
                Err(_) => {
                    return html(RefusedPage {
                        detail: "The service answered, but the answer could not be proven to be your journey.".to_string(),
                    });
                }
            };
            // The METADATA cache is written on every successful read (counts
            // only — the type carries no field that could hold a sentence).
            if let Some(meta) = to_metadata(&value, now_secs()) {
                st.cache.store(meta);
            }
            parse_journey(&value).map_or(
                html(RefusedPage {
                    detail: "The service answered, but the answer could not be proven to be your journey.".to_string(),
                }),
                |(client, cases, domain_events)| {
                    html(JourneyPage {
                        client,
                        domain_events,
                        cases,
                    })
                },
            )
        }
        Ok(resp) => {
            // A refusal is NAMED (the kernel's own law answers everyone but
            // the client's own principal + the operator with the same
            // existence-blind 404 — the portal renders the refusal, never an
            // empty journey).
            let detail = match resp.status() {
                StatusCode::UNAUTHORIZED => {
                    "The service rejected the portal's credential. Contact the operator (the portal's access needs re-arming)."
                }
                StatusCode::NOT_FOUND => "The service has no journey for this portal's client.",
                StatusCode::CONFLICT => {
                    "This client's contract is archived — the journey is closed."
                }
                _ => "The service refused the read.",
            };
            html(RefusedPage {
                detail: detail.to_string(),
            })
        }
        Err(_) => {
            // THE OFFLINE STATE: the named render, METADATA ONLY. The cache
            // holds counts and an age; there is nothing here that could
            // pretend to be your journey.
            html(st.cache.take().map_or(
                OfflinePage {
                    age: -1,
                    case_count: 0,
                    event_count: 0,
                    domain_count: 0,
                },
                |meta| OfflinePage {
                    age: now_secs() - meta.cached_at,
                    case_count: meta.cases.len(),
                    event_count: meta.cases.iter().map(|(_, n)| *n).sum(),
                    domain_count: meta.domain_event_count,
                },
            ))
        }
    }
}

// ── the propose door (the only two writes, both through the kernel) ───────

#[derive(Debug, serde::Deserialize)]
struct AppealForm {
    review_digest: String,
    reason: String,
}

async fn file_appeal(
    State(st): State<Arc<PortalState>>,
    headers: axum::http::HeaderMap,
    Form(form): Form<AppealForm>,
) -> Response {
    if require_session(&st, &headers).is_none() {
        return Redirect::to("/login").into_response();
    }
    let path = format!("/clients/{}/appeals", st.cfg.client);
    let case: Option<i64> = None; // the portal files client-level appeals
    let body = serde_json::json!({
        "review_digest": form.review_digest.trim(),
        "reason": form.reason,
        "case": case,
    });
    match kernel_post(&st, &path, body).await {
        Ok(resp) if resp.status() == StatusCode::OK => html(FiledPage {
            what: "appeal".to_string(),
        }),
        Ok(resp) => {
            let detail = if resp.status() == StatusCode::BAD_REQUEST {
                "The appeal needs the receipt (the digest you were shown beside the decision), and the reason must be under 2000 characters."
            } else {
                "The service refused the filing."
            };
            html(RefusedPage {
                detail: detail.to_string(),
            })
        }
        Err(_) => html(RefusedPage {
            detail: "The service is briefly unreachable — nothing was filed. Try again from your journey page.".to_string(),
        }),
    }
}

#[derive(Debug, serde::Deserialize)]
struct RequestForm {
    action: String,
    #[serde(default)]
    detail: String,
}

async fn file_request(
    State(st): State<Arc<PortalState>>,
    headers: axum::http::HeaderMap,
    Form(form): Form<RequestForm>,
) -> Response {
    if require_session(&st, &headers).is_none() {
        return Redirect::to("/login").into_response();
    }
    let action = form.action.trim().to_ascii_lowercase();
    if action != "dsar" && action != "erase" {
        return html(RefusedPage {
            detail: "The request must be a data request or an erasure request.".to_string(),
        });
    }
    let path = format!("/clients/{}/requests", st.cfg.client);
    let body = serde_json::json!({
        "action": action,
        "detail": form.detail,
    });
    match kernel_post(&st, &path, body).await {
        Ok(resp) if resp.status() == StatusCode::OK => html(FiledPage {
            what: "request".to_string(),
        }),
        Ok(_) => html(RefusedPage {
            detail: "The service refused the filing.".to_string(),
        }),
        Err(_) => html(RefusedPage {
            detail: "The service is briefly unreachable — nothing was filed. Try again from your journey page.".to_string(),
        }),
    }
}

// the pin's type-level witness: the render row types are referenced here so
// a rename cannot silently orphan the template inputs.
#[allow(dead_code)]
fn _template_input_types(_: CaseRow, _: EventRow) {}
