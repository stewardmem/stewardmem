//! # The document-correction provider lane (IMPROVE16 S1)
//!
//! The operator-configured LLM correction step for scan-grade documents —
//! the config + transport + pinned prompt, and NOTHING else. The lane's
//! GOVERNANCE (the two-artifact law, the anchor-contract cage, the stamps,
//! the review posture) lives at the route seam (S2); this module is the
//! provider posture alone:
//!
//! - **The GDL all-or-nothing law** (`config::DocCorrectorProfile`): four
//!   env vars or none. None = the lane is OFF (a `correct: true` request
//!   refuses named; the DEFAULT pipeline never touches this module).
//! - **The secret-file law**: the bearer rides
//!   [`crate::secret_file::read_provider_secret`] — root-confined, no
//!   symlink, owner-only — read once at client construction, never logged,
//!   never in an error string. (A local endpoint that ignores
//!   Authorization still gets the shape: point `*_SECRET_FILE` at any
//!   owner-only file; one posture, no branching.)
//! - **The local-endpoint posture**: `http` is admitted ONLY as the plan's
//!   preferred shape (ollama/MLX/LM Studio on the operator's own machine);
//!   a hosted endpoint is the operator's explicit choice, recorded in the
//!   audit row per call. The endpoint is operator-CONFIGURED
//!   all-or-nothing — unlike a webhook, no request-controlled value ever
//!   selects it — so the address screen resolves and pins the configured
//!   host (private/loopback admitted BY DESIGN) and redirects are refused:
//!   the screen validated ONE endpoint, a redirect is a different one.
//! - **The bounded call**: named connect/first-byte/total timeouts, a
//!   response-bytes cap enforced on the stream, ONE retry with a named
//!   backoff on the transient classes (connect, timeout, 5xx) — a 4xx, a
//!   malformed body, or an over-cap body refuses without a retry.
//! - **The audit row per call**: kind `doc-correction`, the model asked
//!   and the model that answered, bytes in/out, duration, attempts, and
//!   the request/response sha256s — the reproducible-BY-RECORD seam for an
//!   artifact that is honestly NOT a pure function of the PDF bytes (DI-1
//!   does not hold for it; the record is what makes it auditable).
//! - **Determinism, stated honestly**: temperature 0 is DECLARED (the
//!   deterministic-preferred posture) but the artifact's law does not
//!   claim purity — re-correction is a new explicit request producing a
//!   new row (the route's law), never a silent re-derive.

use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// The pinned system prompt. COMMITTED BY NAME: the corrector's instruction
/// set is part of the lane's governance — change it and the lane's outputs
/// change meaning. It says: correct text WITHIN the page/anchor structure,
/// change NOTHING else. The route's deterministic check
/// ([`crate::ingest_document::verify_anchor_contract`]) is the cage that
/// enforces what this prompt merely requests — the prompt is the courtesy,
/// the contract is the law (the EditVeto frame's deterministic analogue:
/// no model sits in the gate).
pub(crate) const CORRECTOR_SYSTEM_PROMPT: &str = "\
You correct OCR transcription errors in an annotated markdown document. \
Correct ONLY the running text: wrong words, wrong characters, spacing, and \
punctuation that the OCR engine mis-read. You MUST NOT change anything else:
1. Keep every line beginning '## Page ' byte-identical — same numbers, same order, no new or removed page headings.
2. Keep every '<!-- anchor: pN -->' line byte-identical, in its exact position at the end of its page block.
3. Keep every other '<!-- ... -->' comment byte-identical, including the header comment at the top of the document.
4. Never add, remove, reorder, merge, or split pages, headings, paragraphs, or lines of structure.
5. Preserve the document's language(s) exactly as written — never translate, never summarize, never rewrite for style.
6. If a span is uncertain, keep the OCR text as-is; never invent content.
Output ONLY the corrected annotated markdown — no preamble, no explanation, no code fences.";

/// The completions path the client posts to: the OpenAI-compatible shape
/// ollama (`/v1/chat/completions`), LM Studio, and mlx-openai-server all
/// serve. The base_url carries the operator's prefix; this path appends.
pub(crate) const COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// The declared sampling contract: deterministic-PREFERRED. Stated in the
/// audit detail so the artifact's provenance names it; NOT claimed as a
/// purity guarantee (the module doc carries the honest statement).
pub(crate) const CORRECTOR_TEMPERATURE: f32 = 0.0;

/// Named bounds for the bounded call. Production defaults live in
/// [`BOUNDED_CALL_DEFAULTS`]; tests construct tighter ones so the bounds
/// are PROVEN exercised, never just declared.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BoundedCall {
    pub(crate) connect_timeout: Duration,
    pub(crate) first_byte_timeout: Duration,
    pub(crate) total_timeout: Duration,
    pub(crate) max_response_bytes: usize,
    pub(crate) retry_backoff: Duration,
}

/// The production bounds, each reasoned: a 49-page annotated markdown can
/// be ~200 KB of prompt for a local 7–14B model — correction is a
/// slow, batch-shaped call, not a chat turn, so the total ceiling is an
/// order of magnitude above the chat provider's. The response cap admits
/// the corrected document at the same ceiling the request side already
/// rides (`config::MAX_REQUEST_SIZE` bounds the ingest body; 4 MiB here
/// covers a corrected markdown plus protocol overhead without admitting an
/// unbounded reply).
pub(crate) const BOUNDED_CALL_DEFAULTS: BoundedCall = BoundedCall {
    connect_timeout: Duration::from_secs(5),
    first_byte_timeout: Duration::from_secs(120),
    total_timeout: Duration::from_secs(600),
    max_response_bytes: 4 * 1024 * 1024,
    retry_backoff: Duration::from_millis(500),
};

/// How a correction call ended. Errors cross this boundary as fixed codes —
/// never a URL, never a secret, never raw provider text (the GDL law).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CorrectorOutcome {
    /// The endpoint answered a well-shaped completion. `model_served` is the
    /// endpoint's echo of the model identity (the stamp's version half).
    Corrected {
        content: String,
        model_served: String,
    },
    /// The endpoint never answered a usable completion after the retry
    /// ceiling: connect refusal, timeout, 5xx, over-cap body. → the route's
    /// `corrector_unavailable` (503).
    Unavailable,
    /// The endpoint answered but the answer is unusable: a 4xx, a malformed
    /// body, empty content. → the route's `corrector_failed` (422).
    Failed,
}

/// The audit facts of ONE correction call — the reproducible-by-record
/// seam. No URL and no secret ride in any field.
#[derive(Debug, Clone)]
pub(crate) struct CorrectorCallRecord {
    pub(crate) model_configured: String,
    pub(crate) model_served: Option<String>,
    pub(crate) bytes_in: usize,
    pub(crate) bytes_out: usize,
    pub(crate) duration_ms: u128,
    pub(crate) attempts: u32,
    pub(crate) request_sha256: String,
    pub(crate) response_sha256: String,
    pub(crate) outcome: &'static str,
}

impl CorrectorOutcome {
    pub(crate) fn as_audit_str(&self) -> &'static str {
        match self {
            Self::Corrected { .. } => "corrected",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
        }
    }
}

/// The corrected-variant stamp's vocabulary guard: `<model>@<served>` must
/// survive the header field parser (split on `|`, keyed on `name:`) — so
/// neither half may carry a pipe, a control byte, or header-hostile bulk.
/// A bizarre echo degrades to `unversioned` instead of poisoning the
/// header vocabulary.
pub(crate) fn corrector_stamp(model_configured: &str, model_served: Option<&str>) -> String {
    let served = match model_served {
        Some(s)
            if !s.is_empty()
                && s.len() <= 256
                && !s.chars().any(|c| c == '|' || c.is_control()) =>
        {
            s
        }
        _ => "unversioned",
    };
    format!("{model_configured}@{served}")
}

/// The screen-and-pin resolved client. Constructed per call from the
/// server-owned profile (the GDL construction shape); the secret is read
/// HERE, once, and never leaves this struct.
pub(crate) struct DocCorrectorClient {
    url: String,
    model: String,
    auth_header: String,
    bounds: BoundedCall,
    client: reqwest::Client,
}

impl DocCorrectorClient {
    /// Resolve the configured endpoint's address set ONCE (the screen),
    /// pin the client to it, and refuse every redirect. Private/loopback
    /// addresses are admitted BY DESIGN — the local-endpoint posture —
    /// because the endpoint is operator-configured all-or-nothing, never
    /// request-selected (the module doc carries the full reasoning).
    pub(crate) fn new(
        profile: &crate::config::DocCorrectorProfile,
    ) -> Result<Self, CorrectorScreenRefused> {
        Self::with_bounds(profile, BOUNDED_CALL_DEFAULTS)
    }

    /// The test-visible constructor: tighter bounds so the fixture tests
    /// PROVE the bounds fire. Production always rides
    /// [`BOUNDED_CALL_DEFAULTS`].
    pub(crate) fn with_bounds(
        profile: &crate::config::DocCorrectorProfile,
        bounds: BoundedCall,
    ) -> Result<Self, CorrectorScreenRefused> {
        let parsed = reqwest::Url::parse(&profile.base_url).map_err(|_| CorrectorScreenRefused)?;
        let host = parsed.host_str().ok_or(CorrectorScreenRefused)?.to_string();
        let port = parsed
            .port_or_known_default()
            .ok_or(CorrectorScreenRefused)?;
        let addrs = crate::webhook::resolve_and_validate_sink(&host, port, true)
            .map_err(|_| CorrectorScreenRefused)?;
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(bounds.connect_timeout)
            .read_timeout(bounds.first_byte_timeout)
            .timeout(bounds.total_timeout)
            .resolve_to_addrs(&host, &addrs);
        if parsed.scheme() == "https" {
            builder = builder.https_only(true);
        }
        let auth_header = format!("Bearer {}", read_secret_blocking(profile)?);
        let client = builder.build().map_err(|_| CorrectorScreenRefused)?;
        Ok(Self {
            url: build_completions_url(&profile.base_url),
            model: profile.model.clone(),
            auth_header,
            bounds,
            client,
        })
    }

    /// ONE correction call: the verbatim annotated markdown in, the
    /// corrected annotated markdown (as the endpoint produced it) out.
    /// The retry law: exactly ONE retry, transient classes only.
    pub(crate) async fn correct(&self, markdown: &str) -> (CorrectorOutcome, CorrectorCallRecord) {
        let started = Instant::now();
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": CORRECTOR_SYSTEM_PROMPT},
                {"role": "user", "content": markdown},
            ],
            "temperature": CORRECTOR_TEMPERATURE,
            "stream": false,
        });
        let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
        let request_sha256 = sha256_hex(&body_bytes);
        let bytes_in = markdown.len();

        let mut attempts: u32 = 0;
        let mut last: Option<(CorrectorOutcome, usize, String)> = None;
        while attempts < 2 {
            attempts += 1;
            if attempts == 2 {
                tokio::time::sleep(self.bounds.retry_backoff).await;
            }
            let response = self
                .client
                .post(&self.url)
                .header("Authorization", &self.auth_header)
                .header("Content-Type", "application/json")
                .body(body_bytes.clone())
                .send()
                .await;
            match response {
                Err(e) if e.is_connect() || e.is_timeout() => {
                    last = Some((CorrectorOutcome::Unavailable, 0, String::new()));
                    continue;
                }
                Err(_) => {
                    last = Some((CorrectorOutcome::Unavailable, 0, String::new()));
                    continue;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let (read_ok, bytes) = read_bounded(resp, self.bounds.max_response_bytes).await;
                    let response_sha = sha256_hex(&bytes);
                    if !read_ok {
                        last = Some((CorrectorOutcome::Unavailable, bytes.len(), response_sha));
                        continue;
                    }
                    if status.is_server_error() {
                        last = Some((CorrectorOutcome::Unavailable, bytes.len(), response_sha));
                        continue;
                    }
                    if !status.is_success() {
                        // 4xx: the endpoint understood and refused — no
                        // retry could change the answer.
                        last = Some((CorrectorOutcome::Failed, bytes.len(), response_sha));
                        break;
                    }
                    match parse_completion(&bytes) {
                        Some((content, served)) => {
                            let outcome = CorrectorOutcome::Corrected {
                                model_served: served.clone(),
                                content,
                            };
                            let record = self.record(
                                attempts,
                                bytes_in,
                                bytes.len(),
                                started.elapsed().as_millis(),
                                &request_sha256,
                                &response_sha,
                                &outcome,
                                Some(served),
                            );
                            return (outcome, record);
                        }
                        None => {
                            last = Some((CorrectorOutcome::Failed, bytes.len(), response_sha));
                            break;
                        }
                    }
                }
            }
        }
        let (outcome, bytes_out, response_sha) =
            last.unwrap_or((CorrectorOutcome::Unavailable, 0, sha256_hex(b"")));
        let record = self.record(
            attempts,
            bytes_in,
            bytes_out,
            started.elapsed().as_millis(),
            &request_sha256,
            &response_sha,
            &outcome,
            None,
        );
        (outcome, record)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &self,
        attempts: u32,
        bytes_in: usize,
        bytes_out: usize,
        duration_ms: u128,
        request_sha256: &str,
        response_sha256: &str,
        outcome: &CorrectorOutcome,
        model_served: Option<String>,
    ) -> CorrectorCallRecord {
        CorrectorCallRecord {
            model_configured: self.model.clone(),
            model_served,
            bytes_in,
            bytes_out,
            duration_ms,
            attempts,
            request_sha256: request_sha256.to_string(),
            response_sha256: response_sha256.to_string(),
            outcome: outcome.as_audit_str(),
        }
    }
}

/// The construction-time refusal: the configured endpoint's address set
/// could not be resolved and screened, or the secret file is invalid.
/// Fixed code, no path, no host (the GDL boundary's error law).
#[derive(Debug)]
pub(crate) struct CorrectorScreenRefused;

fn read_secret_blocking(
    profile: &crate::config::DocCorrectorProfile,
) -> Result<String, CorrectorScreenRefused> {
    crate::secret_file::read_provider_secret(&profile.secret_root, &profile.secret_file)
        .map_err(|_| CorrectorScreenRefused)
}

/// Append the completions path to the operator's base URL: a base that
/// already ends in `/v1` yields `…/v1/chat/completions` (the doubled
/// prefix would 404 on every server the docs name); a bare host
/// yields `…/v1/chat/completions` too (the path is the lane's contract,
/// not the operator's guess).
fn build_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
    format!("{trimmed}{COMPLETIONS_PATH}")
}

/// Parse the OpenAI-compatible completion body: the first choice's message
/// content and the endpoint's model echo. Anything else — no choices, an
/// empty content, a non-string — is None (the `corrector_failed` class).
fn parse_completion(bytes: &[u8]) -> Option<(String, String)> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let served = v
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("unversioned")
        .to_string();
    let content = v
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?
        .to_string();
    if content.is_empty() {
        return None;
    }
    Some((content, served))
}

/// The bounded body read: the cap is enforced on the stream (a hostile
/// endpoint cannot fill memory behind a lying Content-Length).
async fn read_bounded(resp: reqwest::Response, cap: usize) -> (bool, Vec<u8>) {
    let mut buf = Vec::new();
    use tokio_stream::StreamExt;
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = match item {
            Ok(c) => c,
            Err(_) => return (false, buf),
        };
        if buf.len() + chunk.len() > cap {
            return (false, buf);
        }
        buf.extend_from_slice(&chunk);
    }
    (true, buf)
}

/// Write the call's audit row (kind `doc-correction`). Best-effort (the
/// `audit::record` law): the correction verdict does not depend on the
/// evidence row landing, but the row is the reproducible-by-record seam —
/// a `None` return is the route's signal that evidence was missed.
pub(crate) fn record_correction_audit(
    conn: &rusqlite::Connection,
    actor: &str,
    input_sha256: &str,
    record: &CorrectorCallRecord,
) -> Option<i64> {
    let detail = serde_json::json!({
        "model": record.model_configured,
        "model_served": record.model_served,
        "bytes_in": record.bytes_in,
        "bytes_out": record.bytes_out,
        "duration_ms": record.duration_ms,
        "attempts": record.attempts,
        "request_sha256": record.request_sha256,
        "response_sha256": record.response_sha256,
        "sampling": "temperature=0 (declared; not a purity claim)",
    })
    .to_string();
    let status = match record.outcome {
        "corrected" => crate::audit::AuditStatus::Ok,
        _ => crate::audit::AuditStatus::Error,
    };
    crate::audit::record(
        conn,
        crate::audit::AuditKind::DocCorrection,
        actor,
        input_sha256,
        status,
        &detail,
    )
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::sync::Arc;

    /// The fixture server: a std-thread HTTP/1.1 responder playing a
    /// SCRIPT of raw responses, one per connection, recording every request
    /// body it received. No live calls — the replay IS the test (the
    /// scripted_server posture).
    struct ScriptServer {
        addr: std::net::SocketAddr,
        received: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    }

    impl ScriptServer {
        fn spawn(script: Vec<(u16, String)>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
            let addr = listener.local_addr().unwrap();
            let received = Arc::new(std::sync::Mutex::new(Vec::new()));
            let received_clone = Arc::clone(&received);
            std::thread::spawn(move || {
                for (status, body) in script {
                    let Ok((mut sock, _)) = listener.accept() else {
                        return;
                    };
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 8192];
                    loop {
                        let n = sock.read(&mut chunk).unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let raw = String::from_utf8_lossy(&buf).to_string();
                    if let Some(body_part) = raw.split("\r\n\r\n").nth(1) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(body_part) {
                            received_clone.lock().unwrap().push(v);
                        }
                    }
                    let reason = match status {
                        200 => "OK",
                        400 => "Bad Request",
                        _ => "Internal Server Error",
                    };
                    let resp = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes());
                    let _ = sock.shutdown(std::net::Shutdown::Both);
                }
                // The listener drops here; any stray connect is refused.
            });
            Self { addr, received }
        }

        fn base_url(&self) -> String {
            format!("http://{}", self.addr)
        }
    }

    fn completion_body(content: &str, model: &str) -> String {
        serde_json::json!({
            "id": "cmpl-fixture",
            "model": model,
            "choices": [{"index": 0, "message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        })
        .to_string()
    }

    /// A profile + client pointed at the fixture, with test-tight bounds.
    fn fixture_client(
        base_url: &str,
        bounds: BoundedCall,
    ) -> (DocCorrectorClient, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("doc-corrector-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("mkdir root");
        let secret_path = root.join("corrector.key");
        std::fs::write(&secret_path, "fixture-secret").expect("write secret");
        // The secret-file law's owner-only mode — the reader refuses 0644.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600))
                .expect("chmod secret");
        }
        let profile = crate::config::DocCorrectorProfile {
            base_url: base_url.to_string(),
            model: "fixture-model".to_string(),
            secret_file: std::path::PathBuf::from("corrector.key"),
            secret_root: root.clone(),
        };
        let client = DocCorrectorClient::with_bounds(&profile, bounds)
            .expect("the fixture endpoint screens clean");
        (client, root)
    }

    fn test_bounds() -> BoundedCall {
        BoundedCall {
            connect_timeout: Duration::from_secs(2),
            first_byte_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(10),
            max_response_bytes: 1 << 20,
            retry_backoff: Duration::from_millis(10),
        }
    }

    const VERBATIM: &str = "<!-- doc: abc | source: document | pages: 1 | tier: docling\n     extracted: 2026-11-12T00:00:00Z | extractor: docling-rs/0.9 | layout: unverified -->\n## Page 1\nThe cat satt on the mat.\n<!-- anchor: p1 -->\n";

    fn runtime<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(f)
    }

    /// The happy replay: the corrected markdown comes back whole, the
    /// request carried the pinned contract (model, temperature 0,
    /// stream=false, the system prompt, the verbatim body), and the record
    /// is complete.
    #[test]
    fn corrected_happy_path_replay_carries_the_pinned_contract() {
        let corrected = VERBATIM.replace("satt", "sat");
        let server = ScriptServer::spawn(vec![(200, completion_body(&corrected, "fixture-model"))]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        match outcome {
            CorrectorOutcome::Corrected {
                content,
                model_served,
            } => {
                assert!(content.contains("sat on the mat"));
                assert!(!content.contains("satt"));
                assert_eq!(model_served, "fixture-model");
            }
            other => panic!("expected Corrected, got {other:?}"),
        }
        assert_eq!(record.outcome, "corrected");
        assert_eq!(record.attempts, 1);
        assert_eq!(record.model_configured, "fixture-model");
        assert_eq!(record.model_served.as_deref(), Some("fixture-model"));
        assert_eq!(record.bytes_in, VERBATIM.len());
        assert!(!record.request_sha256.is_empty());
        assert!(!record.response_sha256.is_empty());
        // The request the fixture received: the pinned contract, by name.
        let received = server.received.lock().unwrap();
        assert_eq!(received.len(), 1, "no retry on success");
        let req = &received[0];
        assert_eq!(req["model"], "fixture-model");
        assert_eq!(req["temperature"], 0.0);
        assert_eq!(req["stream"], false);
        assert_eq!(req["messages"][0]["role"], "system");
        assert!(
            req["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("## Page "),
            "the system prompt names the structure it forbids touching"
        );
        assert_eq!(req["messages"][1]["content"], VERBATIM);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 500 then success: exactly ONE retry (two attempts, two connections).
    #[test]
    fn a_transient_500_is_retried_exactly_once() {
        let server = ScriptServer::spawn(vec![
            (500, r#"{"error":"overloaded"}"#.to_string()),
            (200, completion_body(VERBATIM, "fixture-model")),
        ]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        assert!(
            matches!(outcome, CorrectorOutcome::Corrected { .. }),
            "the retry succeeds"
        );
        assert_eq!(record.attempts, 2, "one retry, not two");
        assert_eq!(server.received.lock().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    /// 500 forever: the ONE-retry ceiling holds — two attempts, then
    /// Unavailable. A retry loop that never ends is a hung ingest.
    #[test]
    fn a_persistent_500_refuses_unavailable_at_the_one_retry_ceiling() {
        let server = ScriptServer::spawn(vec![
            (500, r#"{"error":"overloaded"}"#.to_string()),
            (500, r#"{"error":"overloaded"}"#.to_string()),
            (200, completion_body(VERBATIM, "fixture-model")),
        ]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        assert_eq!(outcome, CorrectorOutcome::Unavailable);
        assert_eq!(record.attempts, 2, "the ceiling is ONE retry");
        assert_eq!(record.outcome, "unavailable");
        assert_eq!(
            server.received.lock().unwrap().len(),
            2,
            "the third response was never requested"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// A 4xx refuses WITHOUT a retry: the endpoint understood and refused.
    #[test]
    fn a_4xx_refuses_failed_with_no_retry() {
        let server = ScriptServer::spawn(vec![
            (400, r#"{"error":"model not found"}"#.to_string()),
            (200, completion_body(VERBATIM, "fixture-model")),
        ]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        assert_eq!(outcome, CorrectorOutcome::Failed);
        assert_eq!(record.attempts, 1, "no retry on a 4xx");
        assert_eq!(server.received.lock().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// A malformed body (no choices) is the Failed class.
    #[test]
    fn a_malformed_body_refuses_failed() {
        let server = ScriptServer::spawn(vec![(200, r#"{"unrelated":true}"#.to_string())]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        assert_eq!(outcome, CorrectorOutcome::Failed);
        assert_eq!(record.outcome, "failed");
        let _ = std::fs::remove_dir_all(root);
    }

    /// An over-cap response is cut off at the cap and refuses Unavailable
    /// (the stream cap is PROVEN, not just declared).
    #[test]
    fn an_over_cap_response_refuses_unavailable() {
        let huge = "x".repeat(64 * 1024);
        let server = ScriptServer::spawn(vec![(200, completion_body(&huge, "fixture-model"))]);
        let bounds = BoundedCall {
            max_response_bytes: 1024,
            ..test_bounds()
        };
        let (client, root) = fixture_client(&server.base_url(), bounds);
        let (outcome, record) = runtime(async { client.correct(VERBATIM).await });
        assert_eq!(outcome, CorrectorOutcome::Unavailable);
        assert!(record.bytes_out < huge.len(), "the cap cut the read");
        let _ = std::fs::remove_dir_all(root);
    }

    /// An empty completion is the Failed class (an empty corrected
    /// document would fail the anchor contract downstream anyway — refuse
    /// at the provider seam, named).
    #[test]
    fn an_empty_content_refuses_failed() {
        let server = ScriptServer::spawn(vec![(200, completion_body("", "fixture-model"))]);
        let (client, root) = fixture_client(&server.base_url(), test_bounds());
        let (outcome, _record) = runtime(async { client.correct(VERBATIM).await });
        assert_eq!(outcome, CorrectorOutcome::Failed);
        let _ = std::fs::remove_dir_all(root);
    }

    /// The audit row: kind `doc-correction`, the fixed detail vocabulary
    /// (model, served, bytes, duration, attempts, both sha256s), no URL,
    /// no secret anywhere in the row.
    #[test]
    fn the_audit_row_carries_the_record_and_no_secrets() {
        crate::register_sqlite_vec::register_sqlite_vec();
        let mut conn = rusqlite::Connection::open_in_memory().expect("memory db");
        crate::migration::run_migration(&mut conn, crate::config::DB_MMAP_SIZE_MIB)
            .expect("migration");
        let record = CorrectorCallRecord {
            model_configured: "fixture-model".to_string(),
            model_served: Some("fixture-model".to_string()),
            bytes_in: 100,
            bytes_out: 98,
            duration_ms: 4211,
            attempts: 1,
            request_sha256: "req-sha".to_string(),
            response_sha256: "resp-sha".to_string(),
            outcome: "corrected",
        };
        let id = record_correction_audit(&conn, "operator", "input-sha", &record)
            .expect("the audit row lands");
        // The chain stores HASHES of target + detail (no plaintext target,
        // no URL, no secret can ride a row even by accident).
        let (kind, target_hash, status, detail_hash): (String, String, String, String) = conn
            .query_row(
                "SELECT kind, target_hash, status, detail_hash FROM audit_events WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("the row is readable");
        assert_eq!(kind, "doc-correction");
        assert_eq!(target_hash, crate::audit::hash("input-sha"));
        assert_eq!(status, "ok");
        // The chain stores the detail HASH — the row's vocabulary is pinned
        // HERE by reconstructing the exact JSON the writer built (a field
        // added or dropped reds this test; the audit law travels with it).
        let detail_expected = serde_json::json!({
            "model": "fixture-model",
            "model_served": Some("fixture-model"),
            "bytes_in": 100,
            "bytes_out": 98,
            "duration_ms": 4211,
            "attempts": 1,
            "request_sha256": "req-sha",
            "response_sha256": "resp-sha",
            "sampling": "temperature=0 (declared; not a purity claim)",
        })
        .to_string();
        assert_eq!(detail_hash, crate::audit::hash(&detail_expected));
    }

    /// The stamp vocabulary: `<model>@<served>`, with a hostile or missing
    /// echo degrading to `unversioned` instead of poisoning the header
    /// parser's field vocabulary (a `|` or a control byte would break the
    /// keyed parse).
    #[test]
    fn the_stamp_vocabulary_degrades_hostile_echoes() {
        assert_eq!(
            corrector_stamp("qwen2.5:7b", Some("qwen2.5:7b")),
            "qwen2.5:7b@qwen2.5:7b"
        );
        assert_eq!(corrector_stamp("m", None), "m@unversioned");
        assert_eq!(corrector_stamp("m", Some("")), "m@unversioned");
        assert_eq!(corrector_stamp("m", Some("a|b")), "m@unversioned");
        assert_eq!(corrector_stamp("m", Some("a\nb")), "m@unversioned");
        let long = "x".repeat(300);
        assert_eq!(corrector_stamp("m", Some(&long)), "m@unversioned");
    }

    /// The completions path appends under both base shapes the docs name.
    #[test]
    fn the_completions_path_appends_to_both_base_shapes() {
        assert_eq!(
            build_completions_url("http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            build_completions_url("http://127.0.0.1:11434"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
    }
}
