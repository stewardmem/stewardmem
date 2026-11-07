//! The Tier-0 extraction CHILD (IMPROVE04): the isolation boundary for the
//! untrusted-bytes parse.
//!
//! ## Why this process exists
//!
//! `pdf-extract` is `expect()`-heavy on hostile (and merely unusual) input —
//! measured 2026-11-07: a LEGITIMATE arXiv paper (2610.28425) panics it with
//! `missing unicode map and encoding`. The server's release profile runs
//! `panic = "abort"` (the F-50a posture: overflow fails stop, nothing
//! unwinds), so an in-process parse panic is a PROCESS DEATH — one request,
//! no recovery. RUSTSEC-2026-0187's class (uncatchable parser death on
//! untrusted bytes) is therefore NOT fully retired by the patched lock; the
//! patched bound only removes the ADVISORY's shape (unbounded recursion).
//! The complete answer inside this tree's constraints is isolation: the
//! parse runs in THIS child, whose abort kills only the child. The parent
//! (the server) sees a dead child and refuses NAMED — the hostile bytes
//! never reach the server's panic runtime.
//!
//! The contract: PDF bytes on STDIN (≤ the shared 1 MiB cap), one JSON
//! object on STDOUT (`{"pages": [...]}`), exit 0. Everything else — bad
//! bytes, a parser panic, an abort — is a non-zero exit the parent maps to
//! `invalid_document`. No DB, no config, no clock, no logging on stdout:
//! the child is a pure function of its stdin.

fn main() {
    // The cap rides with the bytes: the child re-enforces the shared cap so
    // a misconfigured parent cannot widen the parse surface.
    use std::io::{Read, Write};
    let mut bytes = Vec::new();
    if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
        eprintln!("stdin read failed: {e}");
        std::process::exit(2);
    }
    if bytes.len() > brain_server::config::MAX_REQUEST_SIZE {
        eprintln!("input exceeds the shared cap");
        std::process::exit(3);
    }
    match brain_server::ingest_document::extract_text_layer(&bytes) {
        Ok(doc) => {
            let payload = serde_json::json!({ "pages": doc.pages });
            let mut out = std::io::stdout().lock();
            if serde_json::to_writer(&mut out, &payload).is_err() {
                std::process::exit(4);
            }
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
        // A named no-text-layer is a DETERMINISTIC verdict, not a failure:
        // the parent distinguishes it from a parser death by exit code so
        // the route can answer `no_text_layer` (422) without re-parsing.
        Err(brain_server::ingest_document::DocumentExtractError::NoTextLayer) => {
            std::process::exit(9);
        }
        Err(e) => {
            eprintln!("extract failed: {e}");
            std::process::exit(5);
        }
    }
}
