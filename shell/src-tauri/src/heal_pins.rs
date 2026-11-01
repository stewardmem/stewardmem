//! The self-heal's pins. Every pin drives the REAL `heal()` with a REAL
//! loopback stub kernel and REAL temp-dir token files — only the keychain is
//! a mock, exactly because the platform keychain is the one edge a test must
//! never touch (the token-module law). The stub kernel is the same shape the
//! sidecar pins use: a `TcpListener` that answers `/health` and `/stats` as
//! the kernel would.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::heal::{self, HealReport, HealSkip};
use crate::sidecar::{APP_ORIGIN, WireOutcome};

/// What the stub kernel accepts and what it has seen.
struct Stub {
    /// The bearer the kernel accepts; `None` = the kernel requires a bearer
    /// and rejects every one it is offered.
    accepted: Option<String>,
    /// How many AUTHENTICATED probes reached the kernel.
    stats_hits: AtomicUsize,
}

impl Stub {
    fn spawn(accepted: Option<String>) -> (u16, Arc<Stub>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        let stub = Arc::new(Stub {
            accepted,
            stats_hits: AtomicUsize::new(0),
        });
        let outer = stub.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let share = outer.clone();
                std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let request = String::from_utf8_lossy(&buf);
                    let ok = |body: &str| {
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\naccess-control-allow-origin: {APP_ORIGIN}\r\n\r\n{body}"
                        )
                    };
                    if request.starts_with("GET /health") {
                        let _ = stream.write_all(
                            ok("{\"status\":\"ok\",\"version\":\"1.29.99\"}").as_bytes(),
                        );
                    } else if request.starts_with("GET /stats") {
                        share.stats_hits.fetch_add(1, Ordering::SeqCst);
                        let bearer_ok = match (&share.accepted, extract_bearer(&request)) {
                            (Some(accepted), Some(bearer)) => bearer == accepted.as_str(),
                            (None, _) => false,
                            (Some(_), None) => false,
                        };
                        if bearer_ok {
                            let _ = stream.write_all(ok("{\"rows\":1}").as_bytes());
                        } else {
                            let _ = stream.write_all(
                                b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\n\r\n",
                            );
                        }
                    } else {
                        let _ = stream
                            .write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n");
                    }
                    let _ = stream.flush();
                });
            }
        });
        (port, stub)
    }
}

fn extract_bearer(request: &str) -> Option<String> {
    for line in request.lines() {
        if let Some(rest) = line.strip_prefix("Authorization: Bearer ") {
            return Some(rest.trim().trim_end_matches('\u{0}').to_string());
        }
    }
    None
}

/// The real `probe_wire` against the stub — the same transport production
/// uses, so a pin's proof IS the wire's proof.
fn prove(port: u16) -> impl Fn(&str) -> WireOutcome {
    move |bearer: &str| {
        crate::sidecar::probe_wire(
            port,
            std::time::Duration::from_millis(500),
            APP_ORIGIN,
            if bearer.is_empty() {
                None
            } else {
                Some(bearer)
            },
        )
    }
}

/// A 0600 token file with the given content.
fn token_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write token file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("chmod 0600");
    }
    path
}

#[test]
fn heal_adopts_a_proven_candidate_into_the_keychain() {
    let dir = tempfile_dir();
    let kernel_token = "kerneltoken-aabbccddeeff00112233".to_string();
    let (port, _stub) = Stub::spawn(Some(kernel_token.clone()));
    let source = token_file(&dir, "auth-token", &format!("{kernel_token}\n"));

    let written = Arc::new(AtomicUsize::new(0));
    let stored: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let probe = prove(port);
    let writer = stored.clone();
    let writer_hits = written.clone();
    let mut keychain = move |token: &str| -> Result<(), ()> {
        writer_hits.fetch_add(1, Ordering::SeqCst);
        *writer.lock().expect("slot") = Some(token.to_string());
        Ok(())
    };
    let report = heal::heal(
        Some("stale-bearer-the-kernel-rejects"),
        std::slice::from_ref(&source),
        &probe,
        &mut keychain,
    );
    assert_eq!(
        report,
        HealReport::Healed { source },
        "the proven candidate must be adopted"
    );
    assert_eq!(
        written.load(Ordering::SeqCst),
        1,
        "exactly one keychain write"
    );
    assert_eq!(
        stored.lock().expect("slot").as_deref(),
        Some(kernel_token.as_str()),
        "the keychain holds the KERNEL's token, not the stale one"
    );
}

#[test]
fn heal_never_adopts_a_candidate_the_kernel_rejects() {
    let dir = tempfile_dir();
    let (port, stub) = Stub::spawn(Some("the-real-token".to_string()));
    let source = token_file(&dir, "auth-token", "not-the-kernels-token\n");

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> { Ok(()) };
    let report = heal::heal(
        Some("current"),
        std::slice::from_ref(&source),
        &probe,
        &mut keychain,
    );
    match &report {
        HealReport::NoSource { tried } => {
            assert_eq!(
                tried.as_slice(),
                &[(source, HealSkip::AlreadyRejected)],
                "the rejected candidate is named, not adopted"
            );
        }
        other => panic!("a rejected candidate must not heal, got {other:?}"),
    }
    assert_eq!(
        stub.stats_hits.load(Ordering::SeqCst),
        2,
        "the proof probe ran once for the diagnosis and once for the candidate"
    );
}

#[test]
fn heal_refuses_a_world_readable_source_and_names_it() {
    let dir = tempfile_dir();
    let (port, _stub) = Stub::spawn(Some("good-token-0102030405".to_string()));
    let source = dir.join("auth-token");
    std::fs::write(&source, "good-token-0102030405\n").expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
            .expect("chmod 0644");
    }

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> {
        panic!("a world-readable source must never reach the keychain");
    };
    let report = heal::heal(
        Some("current"),
        std::slice::from_ref(&source),
        &probe,
        &mut keychain,
    );
    match &report {
        HealReport::NoSource { tried } => {
            assert_eq!(
                tried.as_slice(),
                &[(source, HealSkip::InsecureMode { mode: 0o644 })],
                "the mode law refusal names the path AND the mode"
            );
        }
        other => panic!("a world-readable source must be refused, got {other:?}"),
    }
}

#[test]
fn heal_skips_a_fragment_that_cannot_travel_the_wire() {
    let dir = tempfile_dir();
    let kernel_token = "clean-token-1122334455".to_string();
    let (port, _stub) = Stub::spawn(Some(kernel_token));
    // The pathological file: its first whitespace-split fragment carries an
    // embedded NUL. The kernel's middleware would hold that fragment as its
    // bearer, and NO Authorization header can carry a NUL — so the fragment
    // can never prove, and the heal refuses it by name without offering it
    // to the kernel at all.
    let source = token_file(&dir, "auth-token", "first\u{0}frag\nsecond-frag\n");

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> { Ok(()) };
    let report = heal::heal(
        Some("current"),
        std::slice::from_ref(&source),
        &probe,
        &mut keychain,
    );
    match &report {
        HealReport::NoSource { tried } => {
            assert_eq!(
                tried.as_slice(),
                &[(source, HealSkip::NotWireSafe)],
                "a control-carrying fragment is refused by name"
            );
        }
        other => panic!("an unwireable fragment must not heal, got {other:?}"),
    }
}

#[test]
fn heal_adopts_the_first_whitespace_fragment_of_a_multi_token_file() {
    let dir = tempfile_dir();
    let operator = "operator-token-aaaa1111bbbb2222".to_string();
    let agent = "agent-token-cccc3333dddd4444".to_string();
    // THE LIVE SHAPE: the kernel's file holds BOTH tokens on two lines; the
    // kernel's middleware splits on whitespace and accepts each. The shell
    // adopts the FIRST — the operator credential, the one its surfaces need.
    let (port, _stub) = Stub::spawn(Some(operator.clone()));
    let source = token_file(&dir, "auth-token", &format!("{operator}\n{agent}\n"));

    let probe = prove(port);
    let stored: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
    let writer = stored.clone();
    let mut keychain = move |token: &str| -> Result<(), ()> {
        *writer.lock().expect("slot") = Some(token.to_string());
        Ok(())
    };
    let report = heal::heal(Some("stale"), &[source], &probe, &mut keychain);
    assert!(matches!(report, HealReport::Healed { .. }), "heals");
    assert_eq!(
        stored.lock().expect("slot").as_deref(),
        Some(operator.as_str()),
        "the FIRST fragment — the operator token — is what the shell adopts"
    );
}

#[test]
fn heal_does_nothing_when_the_wire_already_works() {
    let dir = tempfile_dir();
    let (port, stub) = Stub::spawn(Some("current".to_string()));
    let source = token_file(&dir, "auth-token", "other-token\n");

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> {
        panic!("a healthy wire must not be touched");
    };
    let report = heal::heal(Some("current"), &[source], &probe, &mut keychain);
    assert_eq!(report, HealReport::NotNeeded);
    assert_eq!(
        stub.stats_hits.load(Ordering::SeqCst),
        1,
        "exactly one diagnostic probe: no candidate is read"
    );
}

#[test]
fn heal_reports_kernel_down_and_reads_nothing() {
    let dir = tempfile_dir();
    let source = token_file(&dir, "auth-token", "token-abcdefabcdefabcdef\n");
    // Bind and DROP a listener: nothing answers.
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    let dead_port = listener.local_addr().expect("addr").port();

    let probe = prove(dead_port);
    let mut keychain = |_token: &str| -> Result<(), ()> {
        panic!("nothing is listening; nothing may be written");
    };
    let report = heal::heal(Some("current"), &[source], &probe, &mut keychain);
    assert_eq!(
        report,
        HealReport::KernelDown,
        "a dead port is the gate's start path, never a heal"
    );
}

#[test]
fn heal_refuses_to_reeat_the_bearer_the_kernel_just_rejected() {
    let dir = tempfile_dir();
    let (port, stub) = Stub::spawn(Some("different".to_string()));
    // The file holds EXACTLY the bearer the kernel just rejected.
    let source = token_file(&dir, "auth-token", "stale-and-rejected\n");

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> { Ok(()) };
    let report = heal::heal(
        Some("stale-and-rejected"),
        std::slice::from_ref(&source),
        &probe,
        &mut keychain,
    );
    match &report {
        HealReport::NoSource { tried } => {
            assert_eq!(
                tried.as_slice(),
                &[(source, HealSkip::AlreadyRejected)],
                "the identical bearer is skipped WITHOUT a proof probe"
            );
        }
        other => panic!("the identical bearer must be skipped, got {other:?}"),
    }
    // The skip happened WITHOUT offering the identical bearer to the kernel
    // again: only the diagnosis probe reached /stats.
    assert_eq!(
        stub.stats_hits.load(Ordering::SeqCst),
        1,
        "no proof probe may run for a bearer the kernel just rejected"
    );
}

#[test]
fn heal_names_an_absent_source_and_tries_the_next_candidate() {
    let dir = tempfile_dir();
    let kernel_token = "real-token-9988776655".to_string();
    let (port, _stub) = Stub::spawn(Some(kernel_token.clone()));
    let missing = dir.join("does-not-exist");
    let present = token_file(&dir, "auth-token", &format!("{kernel_token}\n"));

    let probe = prove(port);
    let mut keychain = |_token: &str| -> Result<(), ()> { Ok(()) };
    let report = heal::heal(
        Some("stale"),
        &[missing.clone(), present.clone()],
        &probe,
        &mut keychain,
    );
    assert_eq!(
        report,
        HealReport::Healed { source: present },
        "the first candidate's absence does not stop the second"
    );
}

#[test]
fn the_candidate_order_puts_our_roots_before_the_operators_files() {
    let home = tempfile_dir();
    let data_root = home.join("data");
    let service_root = home.join("service");
    let paths = heal::candidate_paths(&data_root, &service_root, &home);
    assert_eq!(
        paths,
        vec![
            data_root.join("auth-token"),
            service_root.join("auth-token"),
            home.join(".config/brain-server/auth-token"),
        ],
        "our posture-A root, our posture-B root, then the documented default"
    );
}

#[test]
fn the_d1_declared_token_file_joins_the_candidates_when_the_plist_declares_one() {
    let home = tempfile_dir();
    let plist_dir = home.join("Library/LaunchAgents");
    std::fs::create_dir_all(&plist_dir).expect("mkdir");
    std::fs::write(
        plist_dir.join("com.brain.server.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>EnvironmentVariables</key>
  <dict>
    <key>AUTH_TOKEN_FILE</key>
    <string>/Users/op/declared/auth-token</string>
  </dict>
</dict>
</plist>
"#,
    )
    .expect("write plist");
    let paths = heal::candidate_paths(&home.join("data"), &home.join("service"), &home);
    assert_eq!(
        paths,
        vec![
            home.join("data/auth-token"),
            home.join("service/auth-token"),
            PathBuf::from("/Users/op/declared/auth-token"),
            home.join(".config/brain-server/auth-token"),
        ],
        "the DECLARED file rides before the documented default; no path is duplicated"
    );
}

#[test]
fn the_first_wire_safe_token_refuses_control_characters() {
    assert_eq!(
        heal::first_wire_safe_token("clean-token-1\n"),
        Some("clean-token-1".to_string()),
        "a trailing newline is outer whitespace, not part of the token"
    );
    assert_eq!(
        heal::first_wire_safe_token("\r\ntoken-2\r\n"),
        Some("token-2".to_string())
    );
    assert_eq!(
        heal::first_wire_safe_token("broken\u{0}token x"),
        None,
        "an embedded NUL makes the fragment unwireable"
    );
    assert_eq!(
        heal::first_wire_safe_token("   \n  \n"),
        None,
        "an empty file yields no fragment"
    );
}

// ── the harness ─────────────────────────────────────────────────────────

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "heal-pin-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}
