//! The code-index substrate core (C0): the per-repo SQLite index — schema,
//! tree-sitter ingest, symbol chunks, the lexical tables, the vector table —
//! behind an encoder port the kernel fills. The crate loads no model and
//! makes no network call. NOTHING consumes the index yet; C0 produces the
//! substrate and the frozen baseline, no route, no wire change.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Tests read as sibling cores' tests do (assert + unwrap on known-good
// fixtures); the production-code denies stay absolute.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod encoder;
pub mod error;
pub(crate) mod hashing;
pub mod host;
pub mod ingest;
pub mod inventory;
pub mod parse;
pub mod recall_set;
pub mod schema;
pub mod snapshot;
