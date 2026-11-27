//! The sqlite-vec FFI registration shim for the code-index substrate.
//!
//! `brain-code-core` copies the six linted crates' policy verbatim, including
//! `unsafe_code = "forbid"` — and `forbid` cannot be escaped by a scoped
//! allow. Loading sqlite-vec 0.1.9 is an FFI act with no safe path (the
//! crate's Rust entrypoint is a no-arg stub; the real C symbol takes three
//! arguments). So the audited escape lives HERE, in its own sixty-line
//! crate, as a verbatim copy of the kernel's `src/register_sqlite_vec.rs`
//! with its per-site SAFETY proofs intact. This crate's policy is the
//! kernel lib.rs law — production `unsafe` denied, escapes carry per-site
//! SAFETY proofs — not the six-crate forbid block, and that difference is
//! the whole point of the shim.

#![cfg_attr(not(test), deny(unsafe_code))]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Tests read as sibling cores' tests do (assert + unwrap on known-good
// fixtures); the production-code denies stay absolute.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::ffi::{c_char, c_int};

// See the [dependencies] comment in Cargo.toml: the link comes from the
// build script; this import marks the dependency used.
use sqlite_vec_rs as _;

#[link(name = "sqlite_vec0")]
// SAFETY: this block only DECLARES the C symbols the `sqlite_vec0`
// library exports; declaring an extern block performs no unsafety by
// itself and the signatures mirror the sqlite3 extension ABI exactly
// (entry point + api-routines pointer), which is what makes the pointer
// hand-off in `register_sqlite_vec` sound.
#[allow(unsafe_code)]
unsafe extern "C" {
    #[link_name = "sqlite3_vec_init"]
    fn sqlite3_vec_init_typed(
        _db: *mut rusqlite::ffi::sqlite3,
        _pz_err_msg: *mut *mut c_char,
        _: *const rusqlite::ffi::sqlite3_api_routines,
    ) -> c_int;
}

/// Register the sqlite-vec extension for every connection opened after this
/// call, process-wide. Idempotent: SQLite refuses duplicate registration of
/// the same entry point harmlessly. Call before opening the index file.
pub fn register_sqlite_vec() {
    // SAFETY: sqlite3_auto_extension stores the entry-point pointer for
    // every future connection. The pointer is `sqlite3_vec_init_typed`,
    // whose declaration (above) mirrors the sqlite3 extension ABI; it is
    // the library's OWN exported symbol resolved at link time, so SQLite
    // calls it with exactly the arguments it expects. Idempotent at the
    // SQLite layer (duplicate registration is refused by the engine), and
    // it runs before any connection is opened.
    #[allow(unsafe_code)]
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(sqlite3_vec_init_typed));
    }
}

#[cfg(test)]
mod tests {
    /// The shim's proof: after registration, a plain in-memory connection
    /// hosts vec0 tables and answers a KNN query.
    #[test]
    fn registration_hosts_vec0_tables() {
        super::register_sqlite_vec();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE demo USING vec0(chunk_id INTEGER PRIMARY KEY, embedding float[4]);",
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO demo(chunk_id, embedding) VALUES (1, '[1.0,0.0,0.0,0.0]'), (2, '[0.0,1.0,0.0,0.0]');",
        )
        .unwrap();
        let hit: i64 = conn
            .query_row(
                "SELECT chunk_id FROM demo WHERE embedding MATCH ? AND k = ?",
                rusqlite::params!["[0.9,0.1,0.0,0.0]", 1],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hit, 1, "the nearest neighbour must win the KNN");
    }
}
