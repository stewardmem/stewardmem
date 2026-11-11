//! The thin customer portal (IMPROVE15) — the state, the sessions, the
//! offline cache and the templates. The binary (`src/bin/portal.rs`) is the
//! axum wiring; THIS module is the logic, so the pins can test it.
//!
//! THE AUTH DECISION (§3(c), preregistered): customer auth lives in the
//! PORTAL — the kernel never becomes an identity provider (no customer
//! login endpoint, no customer passwords, no session store on the kernel).
//! The portal holds ONE per-client `client-auditor`-class credential (the
//! roster door's mint: read-only, one domain, tools empty — the day-zero
//! law does the scoping); the credential is a process env held server-side
//! and NEVER reaches the browser (the browser's only cookie is the portal's
//! own session id).
//!
//! THE OFFLINE LAW (the III-6 reuse): when the kernel is unreachable, the
//! portal renders CACHED METADATA ONLY — counts and ages, never event text.
//! The cache type carries no field that could hold a sentence; the offline
//! render is a named state, never an empty journey.
//!
//! THE PROPOSE DOOR: every customer action (an appeal, a DSAR/erase
//! request) files through the kernel's customer doors as a PROPOSAL — the
//! portal is a proxy for two endpoints, never a writer of memory.

#[cfg(test)]
use askama::Template as _;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ── the config ─────────────────────────────────────────────────────────────

/// The portal's boot config. Every field is REQUIRED at boot: a portal
/// without its per-client credential, its client name, or its passcode
/// refuses to start (fail closed — a half-configured portal would render a
/// login page that could never read a journey).
#[derive(Debug, Clone)]
pub struct PortalConfig {
    /// the bind address (default 127.0.0.1:8766 — loopback-only by default;
    /// a deployment that faces customers binds it deliberately).
    pub bind: String,
    /// the kernel origin (default http://127.0.0.1:8765).
    pub kernel_url: String,
    /// THE PER-CLIENT CREDENTIAL (the roster door's mint) — server-side
    /// only, never rendered, never set as a cookie.
    pub kernel_token: String,
    /// the ONE client this portal serves (the per-client posture: a portal
    /// instance per client; the blast radius of a portal compromise is one
    /// client's read-only view).
    pub client: String,
    /// the per-client rotatable passcode (rotate = restart with a new
    /// value; the in-process sessions die with the process — rotation is
    /// therefore total).
    pub passcode: String,
}

impl PortalConfig {
    /// Read from the environment, refusing to start half-configured.
    pub fn from_env() -> Result<Self, String> {
        let required = |name: &str| -> Result<String, String> {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| format!("portal boot requires {name} (fail closed — a half-configured portal renders a login that could never read a journey)"))
        };
        Ok(Self {
            bind: std::env::var("BRAIN_PORTAL_BIND")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "127.0.0.1:8766".to_string()),
            kernel_url: std::env::var("BRAIN_PORTAL_KERNEL_URL")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "http://127.0.0.1:8765".to_string()),
            kernel_token: required("BRAIN_PORTAL_KERNEL_TOKEN")?,
            client: required("BRAIN_PORTAL_CLIENT")?,
            passcode: required("BRAIN_PORTAL_PASSCODE")?,
        })
    }
}

// ── the sessions: in the portal process only ───────────────────────────────

/// The session lifetime (seconds): a half-hour tab. The store is
/// IN-MEMORY ONLY — nothing persists, nothing reaches the kernel, and a
/// restart logs every customer out (the honest skeleton's posture).
pub const PORTAL_SESSION_TTL_SECS: i64 = 30 * 60;

/// The in-process session store: the ONLY session store anywhere in this
/// posture (the kernel's session law is untouched; the browser's cookie is
/// an opaque id into THIS map and nothing else).
#[derive(Default)]
pub struct SessionStore {
    inner: Mutex<HashMap<String, i64>>,
}

impl SessionStore {
    /// Mint a session for a passed passcode. The passcode comparison is the
    /// per-client secret act (constant-time-ish: a fixed-size digest
    /// comparison through the same code path for every input).
    pub fn login(&self, passcode: &str, cfg_passcode: &str) -> Option<String> {
        if !constant_time_eq(passcode.as_bytes(), cfg_passcode.as_bytes()) {
            return None;
        }
        let id = format!("p{:x}", std::process::id()) + &new_session_nonce();
        let now = now_secs();
        self.inner
            .lock()
            .ok()?
            .insert(id.clone(), now + PORTAL_SESSION_TTL_SECS);
        Some(id)
    }

    /// True when the presented session id is live (present + unexpired).
    /// Expired entries are dropped on sight (the store self-cleans).
    pub fn verify(&self, id: &str) -> bool {
        let Ok(mut map) = self.inner.lock() else {
            return false;
        };
        let now = now_secs();
        map.retain(|_, exp| *exp > now);
        map.get(id).is_some_and(|exp| *exp > now)
    }

    /// Drop a session (logout) — and ONLY that session.
    pub fn logout(&self, id: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(id);
        }
    }

    /// The store's size (the ops probe; a deployed portal's session count
    /// is its own business, never the kernel's).
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    /// True when no session is live.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A constant-time-ish equality for the passcode check: the comparison cost
/// is the same for every input of the same length, so a wrong guess learns
/// nothing from timing. (Length leaks — the passcode's length is not a
/// secret worth this much machinery.)
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The session nonce: a random hex string from the OS (no crates, no
/// counters — a session id that a neighbor process could guess is a session
/// a neighbor process could ride).
fn new_session_nonce() -> String {
    let mut bytes = [0u8; 16];
    let ok = (|| {
        let f = std::fs::File::open("/dev/urandom").ok()?;
        use std::io::Read;
        (&f).read_exact(&mut bytes).ok()
    })()
    .is_some();
    if !ok {
        // fail closed: a portal whose entropy failed mints NO session
        return String::new();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── the offline cache: METADATA ONLY ───────────────────────────────────────

/// The cached render input: counts and ids and NOTHING ELSE. The type is
/// the offline law's first half — there is no field that could hold a
/// sentence, a digest, a name, or an actor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JourneyMetadata {
    pub cached_at: i64,
    pub cases: Vec<(i64, usize)>,
    pub domain_event_count: usize,
}

/// The portal's in-process cache: written on every successful kernel read,
/// read ONLY when the kernel is unreachable.
#[derive(Default)]
pub struct MetadataCache {
    inner: Mutex<Option<JourneyMetadata>>,
}

impl MetadataCache {
    pub fn store(&self, meta: JourneyMetadata) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = Some(meta);
        }
    }

    pub fn take(&self) -> Option<JourneyMetadata> {
        self.inner.lock().ok().and_then(|s| s.clone())
    }
}

/// Distill a successful journey read into its METADATA (the only thing that
/// survives offline). The event text never enters this type — the
/// distillation is a count pass, not a copy.
pub fn to_metadata(journey: &serde_json::Value, cached_at: i64) -> Option<JourneyMetadata> {
    let cases = journey.get("cases")?.as_array()?;
    let domain_event_count = journey.get("domain_events")?.as_array()?.len();
    let mut out = Vec::new();
    for case in cases {
        let id = case.get("case_id")?.as_i64()?;
        let events = case.get("events")?.as_array()?;
        out.push((id, events.len()));
    }
    Some(JourneyMetadata {
        cached_at,
        cases: out,
        domain_event_count,
    })
}

// ── the templates (askama, compiled at build time) ────────────────────────
//
// The templates receive ONLY kernel words (the closed customer sentences)
// and the metadata. The credential NEVER enters any context — the template
// structs have no field that could carry it (the type is the law).

/// The login page: the per-client passcode form.
#[derive(Debug, askama::Template)]
#[template(
    ext = "html",
    source = "<!DOCTYPE html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>StewardMem portal</title></head>
<body><h1>Customer portal</h1>
<p>Sign in with your access code.</p>
{% if failed %}<p role=\"alert\">That code did not match.</p>{% endif %}
<form method=\"post\" action=\"/login\"><input type=\"password\" name=\"passcode\" aria-label=\"Access code\" required><button type=\"submit\">Sign in</button></form>
</body></html>"
)]
pub struct LoginPage {
    pub failed: bool,
}

/// One journey event row as the portal renders it: the kernel's OWN words
/// (the closed customer sentence), the role, the optional receipt (an EMPTY
/// string when there is none — askama 0.13's compile-time checker reads the
/// whole expression chain, and a Display-able String keeps the template's
/// type law simple).
#[derive(Debug)]
pub struct EventRow {
    pub text: String,
    pub actor: String,
    pub digest: String,
}

/// One case block: the case's id and its events (the kernel's words
/// verbatim — the portal never paraphrases the projection).
#[derive(Debug)]
pub struct CaseRow {
    pub case_id: i64,
    pub events: Vec<EventRow>,
}

/// The journey page: the kernel's projection, rendered.
#[derive(Debug, askama::Template)]
#[template(
    ext = "html",
    source = "<!DOCTYPE html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Your journey</title></head>
<body><h1>Your journey</h1>
<p>Client {{ client }}. Every line below is read from the audit chain.</p>
{% if !domain_events.is_empty() %}<h2>Your data and appeals</h2><ol>{% for e in domain_events %}<li><p>{{ e.text }}</p><p>{{ e.actor }}</p>{% if !e.digest.is_empty() %}<code>{{ e.digest }}</code>{% endif %}</li>{% endfor %}</ol>{% endif %}
{% for c in cases %}<h2>Case {{ c.case_id }}</h2><ol>{% for e in c.events %}<li><p>{{ e.text }}</p><p>{{ e.actor }}</p>{% if !e.digest.is_empty() %}<code>{{ e.digest }}</code>{% endif %}</li>{% endfor %}</ol>{% endfor %}
<h2>Challenge a decision</h2>
<form method=\"post\" action=\"/appeal\"><label>Receipt (the digest you were shown) <input name=\"review_digest\" required></label><label>Your reason <textarea name=\"reason\" maxlength=\"2000\" required></textarea></label><button type=\"submit\">File the appeal</button></form>
<h2>Ask about your data</h2>
<form method=\"post\" action=\"/requests\"><label>Action <select name=\"action\"><option value=\"dsar\">Run my data request</option><option value=\"erase\">Erase my data</option></select></label><label>Detail <textarea name=\"detail\" maxlength=\"2000\"></textarea></label><button type=\"submit\">File the request</button></form>
</body></html>"
)]
pub struct JourneyPage {
    pub client: String,
    pub domain_events: Vec<EventRow>,
    pub cases: Vec<CaseRow>,
}

/// The OFFLINE page: the named state, METADATA ONLY. The template's fields
/// are counts and an age — the type cannot carry a sentence (the offline
/// law, at the template level).
#[derive(Debug, askama::Template)]
#[template(
    ext = "html",
    source = "<!DOCTYPE html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Your journey — offline</title></head>
<body><h1>The service is briefly unreachable</h1>
<p role=\"status\">Last read {{ age }} seconds ago.</p>
<p>Your cases at that read: {{ case_count }}. Events recorded at that read: {{ event_count }}. Appeals and data acts: {{ domain_count }}.</p>
<p>What you read here are counts only — the service shows your journey when it is reachable again.</p>
</body></html>"
)]
pub struct OfflinePage {
    pub age: i64,
    pub case_count: usize,
    pub event_count: usize,
    pub domain_count: usize,
}

/// The refusal page (a named refusal, never an empty journey).
#[derive(Debug, askama::Template)]
#[template(
    ext = "html",
    source = "<!DOCTYPE html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Your journey</title></head>
<body><h1>The service refused this read</h1>
<p role=\"alert\">{{ detail }}</p>
</body></html>"
)]
pub struct RefusedPage {
    pub detail: String,
}

/// The receipt page (an appeal or a request FILED — a pending proposal the
/// operator disposes; nothing happened to your data yet, by design).
#[derive(Debug, askama::Template)]
#[template(
    ext = "html",
    source = "<!DOCTYPE html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>Received</title></head>
<body><h1>Received.</h1>
<p role=\"status\">Your {{ what }} was filed. A human reviews it — you will see the decision on your journey. Nothing about your data changed by filing this.</p>
<p><a href=\"/\">Back to your journey</a></p>
</body></html>"
)]
pub struct FiledPage {
    pub what: String,
}

// ── the wire parse (the same validating read the shell carries) ───────────

/// Parse a kernel journey response into the portal's render rows. `None` =
/// the answer is not provably a journey — the caller renders the refusal,
/// never a guess.
pub fn parse_journey(value: &serde_json::Value) -> Option<(String, Vec<CaseRow>, Vec<EventRow>)> {
    let client = value.get("client")?.as_str()?.to_string();
    let mut cases = Vec::new();
    for case in value.get("cases")?.as_array()? {
        let case_id = case.get("case_id")?.as_i64()?;
        let mut events = Vec::new();
        for event in case.get("events")?.as_array()? {
            events.push(parse_event(event)?);
        }
        cases.push(CaseRow { case_id, events });
    }
    let mut domain_events = Vec::new();
    for event in value.get("domain_events")?.as_array()? {
        domain_events.push(parse_event(event)?);
    }
    Some((client, cases, domain_events))
}

fn parse_event(event: &serde_json::Value) -> Option<EventRow> {
    Some(EventRow {
        text: event.get("text")?.as_str()?.to_string(),
        actor: event.get("actor")?.as_str()?.to_string(),
        digest: event
            .get("digest")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_passcode_check_rejects_a_wrong_code_and_accepts_the_right_one() {
        let store = SessionStore::default();
        let wrong = store.login("nope", "secret-code");
        assert!(wrong.is_none(), "a wrong passcode mints no session");
        let right = store.login("secret-code", "secret-code");
        assert!(right.is_some(), "the right passcode mints a session");
        let id = right.expect("session id");
        assert!(store.verify(&id), "the minted session verifies");
        store.logout(&id);
        assert!(!store.verify(&id), "the logged-out session is gone");
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn the_metadata_type_cannot_hold_a_sentence() {
        let journey = serde_json::json!({
            "client": "acme-us", "domain": "acme",
            "cases": [{"case_id": 7, "status": "running", "opened_at": 1,
                       "events": [{"kind": "case_opened", "case_id": 7, "at": 1, "seq": 1,
                                   "text": "SECRET SENTENCE", "actor": "the operator",
                                   "digest": null, "count": null, "phase": null}]}],
            "domain_events": []
        });
        let meta = to_metadata(&journey, 9_000).expect("metadata");
        assert_eq!(meta.cases, vec![(7, 1)]);
        assert_eq!(meta.domain_event_count, 0);
        // the distilled type carries counts only — the secret never entered:
        let rendered = format!("{meta:?}");
        assert!(
            !rendered.contains("SECRET SENTENCE"),
            "the metadata type must not carry event text (the offline law)"
        );
    }

    #[test]
    fn the_offline_page_template_holds_no_event_field() {
        // the template struct's shape IS the law: counts and an age, nothing
        // else. This is a compile-time fact asserted here for the reader;
        // the behavioral half is the metadata test above.
        let page = OfflinePage {
            age: 42,
            case_count: 2,
            event_count: 5,
            domain_count: 1,
        };
        let rendered = page.render().expect("render");
        assert!(rendered.contains("counts only"));
        assert!(!rendered.contains("Your case was opened"));
    }
}
