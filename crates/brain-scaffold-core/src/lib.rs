//! The scaffold registry's decision core (SI0): the closed vocabulary the
//! three registry tables store, the pure verdict a sign re-derives, and the
//! DML every registry read and write runs on the CALLER's connection. The
//! crate holds no DDL — the server's migration owns the tables — and never
//! opens a transaction of its own — the service core owns the boundaries —
//! and it has no model or network path. All of it is pinned by source in
//! `tests/scaffold_registry_pins.rs`.
//!
//! The rule it encodes (A-6, note 02 §7): the primary is file-level recall@10
//! on the fused arm; a candidate passes a paired sign test whose one-sided
//! tail at `b` of `b + c` must sit at or below 0.05 divided by the candidates
//! tried; fewer than ten changed entries cannot pass at any `k`; and the
//! guards (recall@1, recall@5, both strata) are non-inferior — a guard that
//! loses more entries than it gains is fail, never laundered as noise.

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Tests read as sibling cores' tests do (assert + unwrap on known-good
// fixtures); the production-code denies stay absolute.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod rule;
pub mod store;
pub mod validate;
pub mod vocab;
