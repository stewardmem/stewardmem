//! The middleware stack: request-id propagation, security headers
//! (CSP), and the outermost rate limiter. The stack ORDER lives in
//! `app()` below — the pins that hold it
//! `rate_limit_layer_is_outside_auth_layers`,
//! `serve_wires_connect_info_with_socket_addr`) travel with the
//! composition, not with these definitions.

pub mod auth;
pub mod chat;
pub(crate) mod compliance;
pub mod core;
pub mod memory;
pub(crate) mod ump;
pub(crate) mod workflow;
// the route-coverage + route-authz guard tables (the wire's data): re-homed
// beside the registrations they table at the Capstone flip; content
// unchanged. `spire_inventory` floors their row counts.
pub mod route_guards;

use axum::{
    Json, Router,
    body::Body,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::http_limit::RateLimiter;

use tower_http::{
    catch_panic::CatchPanicLayer, compression::CompressionLayer,
    request_id::PropagateRequestIdLayer, sensitive_headers::SetSensitiveHeadersLayer,
    set_header::SetResponseHeaderLayer, timeout::TimeoutLayer, trace::TraceLayer,
};

use crate::config;
use crate::server::bootstrap::AppState;
use auth::{auth_middleware, jwt_auth_middleware};
use std::time::Duration as StdDuration;

/// CSP for API routes — the strictest possible (JSON-only, no content executes).
pub const API_CSP: &str = "default-src 'none'; frame-ancestors 'none'; form-action 'none'";

/// CSP for client subresources (the retired Dioxus era's posture — allows
/// WASM compilation, same-origin API calls, self-hosted fonts/CSS; no CDN,
/// no inline scripts; `'unsafe-eval'` covered the wasm-bindgen glue's
/// `new Function()` — proven live 2026-09-14, and re-proven 2026-11-13:
/// do NOT drop it on a version-bump claim without a real browser boot).
/// ROUND 18 DECOMMISSION NOTE: the SPA's HTML documents no longer ride
/// THIS header — the served shell carries its OWN build-injected meta CSP
/// (per-build script shas, the D6 injection), and a header CSP here would
/// INTERSECT with it and kill the shell's inline payload scripts (the
/// blank /app/ seat, root-caused 2026-11-13). The header survives only on
/// the non-HTML client subresources until the Dioxus workspace's removal
/// lands; the API routes keep the strict API_CSP.
pub const CLIENT_CSP: &str = concat!(
    "default-src 'self'; ",
    "script-src 'self' 'wasm-unsafe-eval' 'unsafe-eval'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "connect-src 'self'; ",
    "img-src 'self' data:; ",
    "font-src 'self' data:; ",
    "frame-ancestors 'none'; ",
    "form-action 'self'; ",
    "base-uri 'self'"
);

/// Which paths ride the WASM-friendly CLIENT_CSP. Segment-exact: the seat
/// is `/app` itself, everything under `/app/`,
/// and the console root `/` — a future `/app-*` route (or its error body,
/// e.g. `/apple`) must NEVER inherit `wasm-unsafe-eval`; it gets the
/// strict API_CSP. The old `starts_with("/app")` shape over-matched and is
/// gone; the auth seat rule (`is_public_path`) already had this exactness.
pub(crate) fn is_client_path(path: &str) -> bool {
    path == "/" || path == "/app" || path.starts_with("/app/")
}

/// Request ID middleware - generates UUID v4 for tracing if not provided.
pub async fn request_id_middleware(mut req: Request<Body>, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    req.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap_or_else(|_| {
            axum::http::HeaderValue::from_str(&uuid::Uuid::new_v4().to_string())
                .expect("generated uuid is a valid header value")
        }),
    );
    next.run(req).await
}

/// Security headers middleware — applies standard hardening headers to every
/// response. Path-aware CSP (strict for API, WASM-friendly for client).
pub async fn security_headers_middleware(req: Request<Body>, next: Next) -> Response {
    // Read the path BEFORE next.run(req) consumes the request.
    let is_client = is_client_path(req.uri().path());
    let mut res = next.run(req).await;
    // The SPA-HTML probe reads the content type BEFORE the mutable header
    // borrow (the two cannot overlap).
    let is_spa_html = is_client
        && res
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("text/html"));
    let headers = res.headers_mut();
    headers.insert(
        axum::http::header::STRICT_TRANSPORT_SECURITY,
        axum::http::HeaderValue::from_static("max-age=63072000; includeSubDomains; preload"),
    );
    // Path-aware CSP: strict for API, WASM-friendly for client
    // subresources — EXCEPT the SPA's HTML documents: those carry the
    // BUILD-INJECTED meta CSP (per-build script shas, the D6 injection),
    // and a header CSP here would intersect with the meta and kill the
    // shell's inline payload scripts (the blank /app/ seat, root-caused
    // 2026-11-13). The document's meta is the more precise policy: sha-
    // pinned inline scripts, no static guessing.
    if !is_spa_html {
        let csp = if is_client { CLIENT_CSP } else { API_CSP };
        headers.insert(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(csp),
        );
    }
    headers.insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    );
    headers.insert(
        axum::http::header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "Permissions-Policy",
        axum::http::HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    res
}

/// Rate limiter middleware — per-IP sliding window (10 000 req/min default,
/// bounded key set via `RATE_LIMIT_MAX_KEYS`).
/// The peer `SocketAddr` extension (injected by
/// `into_make_service_with_connect_info`) is now guaranteed present, so each
/// remote address gets its own bucket. `X-Forwarded-For` is still honored
/// only under `BRAIN_TRUST_PROXY=1`.
pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    // only trust `X-Forwarded-For` when the operator has explicitly
    // opted in via `BRAIN_TRUST_PROXY=1`. Default uses the socket address — a
    // direct-connection attacker cannot spoof it, so the per-IP limiter actually
    // bounds them. When behind a reversing proxy that overwrites client XFF,
    // operators set the flag and the proxy-provided value is trusted instead.
    let ip = if config::brain_trust_proxy() {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            // Take the RIGHTMOST entry — the one the
            // trusted proxy APPENDED. The leftmost is client-controlled (an
            // attacker pre-seeds `X-Forwarded-For: 1.2.3.4` and the appending
            // proxy preserves it), so leftmost-trust allowed bucket evasion
            // and targeted cross-victim 429s under `BRAIN_TRUST_PROXY=1`.
            .and_then(|s| s.split(',').next_back())
            .map(|s| s.trim().to_string())
    } else {
        None
    }
    .or_else(|| {
        req.extensions()
            .get::<SocketAddr>()
            .map(|a| a.ip().to_string())
    })
    .unwrap_or_else(|| "unknown".to_string());

    if !rate_limiter.is_allowed(&ip) {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "rate_limited", "code": "rate_limited" })),
        )
            .into_response();
    }
    next.run(req).await
}

// ── the composed application (moved from main.rs verbatim at C3a) ──────
/// The composed application: every route family, the shared middleware
/// stack, and the state binding. THE WIRE: paths, methods, status
/// vocabulary, and the layer order below are frozen (openapi.yaml +
/// the route-authz table + the law-9 matrix pin them); the layer ORDER
/// is load-bearing — the rate limiter sits OUTSIDE both auth layers
/// (429 before token work), CORS outermost of all (a refusal the browser
/// cannot read is a refusal that never happened — the auth middleware's
/// 401 must pass outward through the CORS layer to carry the grant),
/// security headers outermost of the auth half, and the
/// 1 MiB body limit is applied BEFORE the 1 GiB import-router merge
/// (tower-http eager-application pitfall — see the import_router
/// comment below).
pub fn app(state: Arc<AppState>) -> Router {
    let base = core::router()
        .merge(memory::legacy_router())
        // Legacy contract markers: `/add` and GET `/search` are superseded by
        // `/ingest/memory` + `/recall`. The `Deprecation` header (RFC 8594)
        // signals clients to migrate; both still function. The layer sits
        // HERE so exactly the routes above it (the SPA seat + the health
        // family + the three legacy writes/reads) carry the header — the
        // original chain's application set, preserved byte-for-byte.
        .route_layer(SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("deprecation"),
            axum::http::HeaderValue::from_static("version=\"0.9.5\""),
        ))
        .merge(chat::router())
        .merge(memory::router())
        .merge(ump::router())
        .merge(workflow::router())
        .merge(compliance::router())
        .merge(auth::router());
    // the shared 1 MiB body limit FIRST, then the 1 GiB import dial merged
    // AFTER it (tower-http eager-application pitfall: an outer limit can
    // never be raised by an inner one — see memory::import_router).
    base.layer(tower_http::limit::RequestBodyLimitLayer::new(
        config::MAX_REQUEST_SIZE,
    ))
    .merge(memory::import_router())
    // the compliance pack merges after the shared cap like the chain did
    .merge(compliance::pack_router())
    // Inner layers (closest to handler)
    //
    // The duration now comes from the shared constant rather than an
    // inline literal, so the handler-side write budget (which refuses work it
    // cannot finish before this deadline) is derived from the SAME number. A
    // handler that guessed 30s and a middleware set to 20s would look right in
    // both files and be wrong at runtime.
    .layer(TimeoutLayer::with_status_code(
        axum::http::StatusCode::REQUEST_TIMEOUT,
        StdDuration::from_secs(crate::config::REQUEST_TIMEOUT_SECS),
    ))
    // The document-ingest route merges AFTER the shared timeout (the
    // import_router merge-order precedent): the Tier-2 converter's honest
    // work is minutes-scale and the shared 30 s wall fired a generic 408
    // before the kernel's named converter_timeout could answer (measured
    // 2026-11-08). The route carries its own budget
    // (config::DOCUMENT_INGEST_TIMEOUT_SECS); everything layered BELOW this
    // point in source (CatchPanic, the RBAC route_layer, auth) still covers
    // it — `.layer()` applies to routes registered so far, and this merge
    // sits before them.
    .merge(memory::document_ingest_router())
    .layer(CatchPanicLayer::new())
    // the RBAC evaluation middleware.
    //
    // PLACEMENT, and this is the part that is easy to get exactly wrong:
    // `.layer()`/`.route_layer()` apply BOTTOM-TO-TOP, so a LATER source line
    // runs EARLIER at request time. To run AFTER authentication, this call
    // must sit at a source line STRICTLY BETWEEN the CatchPanic layer and the
    // opaque-auth layer — i.e. ABOVE `auth_middleware` in source. The first
    // draft of this round put it below the JWT layer, which made it OUTER to
    // authentication: it would have run before any `Principal` existed and
    // decided on every request without one.
    // `r47_the_rbac_layer_sits_between_auth_and_the_handler_layers` pins this
    // ordering by LINE NUMBER, not by comment, and it caught exactly that.
    //
    // `route_layer`, not `layer`: a bare `.layer()` also wraps UNMATCHED
    // paths, which would convert this repo's probe-blind 404s into 403s across
    // the whole surface — an unreviewed wire-contract change. Both forms apply
    // only to routes registered SO FAR, and the last `.merge()` above is well
    // before this point, so every registered route is covered.
    .route_layer(middleware::from_fn_with_state(
        auth::RbacMiddlewareState {
            db_path: state.db_path.clone(),
        },
        auth::rbac_middleware,
    ))
    .layer(CompressionLayer::new())
    .layer(middleware::from_fn(request_id_middleware))
    .layer(PropagateRequestIdLayer::new(
        axum::http::HeaderName::from_static("x-request-id"),
    ))
    .layer(TraceLayer::new_for_http())
    // Sensitive-header marking is registered AFTER (so OUTSIDE, so BEFORE)
    // TraceLayer: requests flow outermost→inward, and the marking must
    // already be on the request when tracing first observes it — otherwise
    // any header-logging span prints the bearer in the clear. The order is
    // pinned by `r109_sensitive_headers_mark_before_trace_layer_sees_the_
    // request` (tests/rbac_evaluation_pins.rs).
    .layer(SetSensitiveHeadersLayer::new([
        axum::http::header::AUTHORIZATION,
        axum::http::header::COOKIE,
        axum::http::header::SET_COOKIE,
    ]))
    // The auth/refusal layers. The CORS layer used to sit HERE — between
    // the sensitive-header marking and the opaque auth, i.e. INSIDE the
    // auth middleware — so the middleware's 401 returned from a layer
    // outside CORS and never passed outward through it: the response
    // carried no access-control-allow-origin and the webview CORS-blocked
    // every real refusal (measured on the wire in the credential-heal
    // round's drill: twelve 401s, every one unreadable by the shell).
    // CORS now sits at the outermost seat (below, before `.with_state`),
    // where the pin `an_auth_refusal_carries_the_cors_headers`
    // (tests/cors_wire_pins.rs) holds it.
    .layer(middleware::from_fn_with_state(
        auth::OpaqueAuthState {
            tokens: state.token_store.clone(),
            pool: state.pool.clone(),
            db_path: state.db_path.clone(),
        },
        auth_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        state.jwt_middleware_state.clone(),
        jwt_auth_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        state.rate_limiter.clone(),
        rate_limit_middleware,
    ))
    .layer(middleware::from_fn(security_headers_middleware))
    // Response headers
    .layer(SetResponseHeaderLayer::overriding(
        axum::http::header::SERVER,
        axum::http::HeaderValue::from_static("brain-server"),
    ))
    .layer(SetResponseHeaderLayer::overriding(
        axum::http::HeaderName::from_static("x-api-version"),
        axum::http::HeaderValue::from_static(env!("CARGO_PKG_VERSION")),
    ))
    // CORS OUTERMOST — the last layer before `.with_state`, so a
    // LATER source line runs OUTER at request time and every response
    // (401/403/404 alike) passes outward through the predicate:
    //   (a) preflights (OPTIONS, no credentials by spec) are answered by
    //       CorsLayer BEFORE auth/jwt/rate-limit/security-headers — a
    //       preflight carries no x-request-id, no security headers, and is
    //       not rate-limited (standard CORS posture);
    //   (b) refusals raised by any inner layer now carry the grant for
    //       allowed origins — the shell can READ its 401s;
    //   (c) foreign origins are unchanged — the predicate still refuses to
    //       add ACAO at any layer; the browser remains the enforcer.
    // The r47 RBAC pin reads call sites (`auth_middleware,`), not line
    // numbers, and asserts only rbac < auth && rbac > catchpanic — the
    // move shifts neither.
    .layer(state.cors.clone())
    .with_state(state.clone())
}
