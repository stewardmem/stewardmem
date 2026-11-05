//! ONE comment-stripper for the source-reading pins, replacing four copies.
//!
//! # Why this file exists
//!
//! A pin that greps raw source text passes on a **comment that merely names the
//! symbol it forbids** — the false-pass `r51_gate_law_pins` was written to close,
//! and it has fired more than once since. Every source-reading pin therefore
//! strips comments first, and every one of them grew its own stripper. Four
//! copies drifted, and two of the four were **broken in ways that let comments
//! through**: a pin built on one of them scans comments while believing it has
//! not.
//!
//! # The three hazards, each measured
//!
//! These are not hypothetical. Each was reproduced against this tree before the
//! fix was written.
//!
//! **1 · The lifetime.** `'` outside a string opens a char literal. Rust
//! lifetimes use the same character, so `&'static str` opens a literal that
//! never closes and the machine treats **the rest of the file** as string
//! interior. Every `//` after that stops being a comment opener.
//! **Measured:** 12 leaked comment lines on `src/workflow/drift_census.rs`,
//! 7 on `src/screen.rs`, 12 on `src/embed.rs`.
//!
//! **2 · The line continuation.** `"… \` + newline is legal Rust. Measured
//! across this tree: **this does not leak.** The continuation fix alone cures
//! **0** of the leaks above; the lifetime fix alone cures **all** of them.
//! The arm is kept because it is harmless and because a scanner that does not
//! model a legal Rust form is one refactor away from being wrong — but it is
//! **not** the fix, and this comment says so rather than letting the next round
//! credit it.
//!
//! **3 · The escaped quote.** `"a\"b"` — a `\"` inside a string. A machine that
//! closes on the first `"` ends the string early, and the comment after it
//! survives. This is the hazard the previous fix **still carried**, found by
//! using it as the measuring reference.
//!
//! # What this deliberately does NOT model
//!
//! **Raw strings** (`r#"…"#`). Modelling them would change output on 127 leaked
//! comment lines across 7 files under `src/` that are currently *inside* raw
//! strings — a behaviour change with no measured pin benefit. The ceiling is
//! named here instead of silently paid for.
//!
//! # The rules this obeys
//!
//! - **Never remove code.** A stripper that deletes a non-comment line is worse
//!   than one that leaves a comment. Every change is verified against the real
//!   files its callers scan.
//! - **Keep string and char literals.** A literal is code, and a pin matching a
//!   key has to see the key.
//! - **Comment-stripping only.** No brace matching, no parsing, no refactor
//!   target. This is a scanner, not a lexer, and `main_suite.rs`'s comment guard
//!   is the instrument for anything stronger.

/// Strip `//` and `/* */` comments, preserving string and char literals.
///
/// The shared implementation for the source-reading pins. `r63a`'s copy is the
/// form; `r66b`'s is the previous fix this file supersedes and extends with the
/// escaped-quote arm.
pub fn code_only(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(src.len());
    let (mut in_str, mut in_block) = (false, false);
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if in_block {
            if c == '*' && next == Some('/') {
                in_block = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if !in_str && c == '/' && next == Some('/') {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if !in_str && c == '/' && next == Some('*') {
            in_block = true;
            i += 2;
            continue;
        }
        if in_str {
            // An ESCAPED character is not a terminator: `\"` must not close the
            // string, and `\\` must not escape the character after it.
            //
            // This single arm ALSO covers the line continuation (`"… \` +
            // newline), because the escaped character there is the newline
            // itself: both bytes are emitted and the string stays open across
            // the line. Writing the continuation as a separate arm after this
            // one made it unreachable, so it is not here — one arm, one
            // behaviour. It is NOT the fix for any measured leak (see the
            // module doc); it keeps a legal Rust form modelled rather than
            // accidentally right.
            if c == '\\' {
                out.push(c);
                if let Some(&e) = chars.get(i + 1) {
                    out.push(e);
                }
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            out.push(c);
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            i += 1;
            continue;
        }
        if c == '\'' {
            // ATOMIC char-literal match: a `'` is a literal only if a closing
            // `'` appears within a short lookahead. Otherwise it is a LIFETIME,
            // which is code and passes through as one.
            let mut j = i + 1;
            while j < n && j <= i + 10 {
                match chars[j] {
                    '\\' => j += 2,
                    '\'' => break,
                    _ => j += 1,
                }
            }
            if j < n && chars[j] == '\'' {
                out.extend(chars[i..=j].iter());
                i = j + 1;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────
// The chat-tier env seam (CG-10 E4)
// ─────────────────────────────────────────────────────────────────────────

/// The chat tier (`BRAIN_CHAT_MODEL`) is read PER REQUEST by the chat spine,
/// and cargo runs a crate's tests concurrently — an MLX-tier test that set
/// the key while an AFM-tier test asserted `on_device` would be a
/// scheduling flake, not a pin. Every test that asserts on the tier holds
/// the lock for its whole body; the guard restores whatever the process had
/// on drop, so a panic cannot leak the key into later tests (the posture
/// `case_run.rs`'s ProfileEnvRestore established, now shared across the
/// chat test crates).
#[allow(dead_code)] // used by the chat-tier crates; inert elsewhere
pub mod chat_tier {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    pub const CHAT_MODEL_KEY: &str = "BRAIN_CHAT_MODEL";

    fn lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Acquire the tier lock and set `BRAIN_CHAT_MODEL` to `model` (None
    /// removes the key). The returned guard restores the previous value —
    /// or the key's absence — on drop, releasing the lock at the same
    /// moment: the env window is exactly the critical section.
    pub fn set_chat_model(model: Option<&str>) -> (MutexGuard<'static, ()>, ChatModelEnv) {
        let guard = lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let saved = std::env::var(CHAT_MODEL_KEY).ok();
        match model {
            Some(value) => unsafe { std::env::set_var(CHAT_MODEL_KEY, value) },
            None => unsafe { std::env::remove_var(CHAT_MODEL_KEY) },
        }
        (guard, ChatModelEnv { saved })
    }

    /// Restores the pre-test value of `BRAIN_CHAT_MODEL` on drop.
    pub struct ChatModelEnv {
        saved: Option<String>,
    }

    impl Drop for ChatModelEnv {
        fn drop(&mut self) {
            match &self.saved {
                Some(value) => unsafe { std::env::set_var(CHAT_MODEL_KEY, value) },
                None => unsafe { std::env::remove_var(CHAT_MODEL_KEY) },
            }
        }
    }
}

/// Locate the built `brain-afm` the way every live chat pin does: the
/// suite's own Swift build output. `None` means the macOS lane owns the
/// binary and the calling crate SKIPS with a stated reason.
#[allow(dead_code)] // used by the chat-tier crates; inert elsewhere
pub fn bridge_binary() -> Option<std::path::PathBuf> {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("shell-macos/.build/out/Products/Debug/brain-afm"),
        manifest.join("shell-macos/.build/arm64-apple-darwin/debug/brain-afm"),
    ];
    candidates.iter().find(|p| p.is_file()).cloned()
}
