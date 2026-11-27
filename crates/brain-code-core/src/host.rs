//! The vec0 host port. Loading a SQLite extension is an FFI act; this crate
//! forbids `unsafe` outright (§R2), so the host that owns the audited
//! registration injects it here — the same seam shape as the encoder port
//! (P-5). The kernel fills the port with its own registration helper; the
//! crate's own tests fill it with the `brain-code-vec0` shim.

use crate::error::CodeIndexError;

pub trait Vec0Host {
    /// Register the vec0 SQLite extension process-wide, idempotently, before
    /// any connection is opened. If the extension cannot be hosted, the
    /// error must say so by name — the migration's `CREATE VIRTUAL TABLE …
    /// USING vec0` failing with "no such module: vec0" is the loud failure,
    /// never a silent skip.
    fn ensure_vec0_registered(&self) -> Result<(), CodeIndexError>;
}
