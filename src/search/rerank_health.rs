//! The rerank tier's health echo — the state cell `/health/db` reads.
//!
//! Compiled in EVERY build (unlike the feature-gated [`crate::search::rerank`]
//! module) so the admin health shape is stable across binaries: on a
//! feature-less kernel nothing ever writes the cell and it reads `off`
//! forever, which is the truth about that binary — a kernel that cannot
//! rerank reports `off`, never a guess.
//!
//! Writer: `search::rerank`'s LazyLock initializer (feature-gated), through
//! the three named transitions — one call per transition so a reader never
//! sees a torn `{state, model}` pair, and [`RerankHealth::ready`] REQUIRES
//! the loaded model id, so `ready` with no live handle is unrepresentable
//! (P-6's "never ready while the LazyLock is None", enforced by the type).
//! Reader: `/health/db`'s admin body via [`RerankHealth::echo`] — a snapshot
//! of a boot/lazy decision, never a per-request pragma read, the same class
//! as the `durability`/`loom` echoes. The reduced public shape carries
//! nothing (the admin-telemetry rule).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// The process-wide cell. Written by the feature-gated reranker at load;
/// read by the admin `/health/db` body.
pub static RERANK_HEALTH: RerankHealth = RerankHealth::new();

/// The honest state vocabulary: `off` (feature absent or never armed) is the
/// zero value, so an untouched cell is already the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankState {
    Loading,
    Ready,
    Failed,
}

const OFF: u8 = 0;
const LOADING: u8 = 1;
const READY: u8 = 2;
const FAILED: u8 = 3;

fn state_byte(state: RerankState) -> u8 {
    match state {
        RerankState::Loading => LOADING,
        RerankState::Ready => READY,
        RerankState::Failed => FAILED,
    }
}

fn state_name(byte: u8) -> &'static str {
    match byte {
        LOADING => "loading",
        READY => "ready",
        FAILED => "failed",
        _ => "off",
    }
}

pub struct RerankHealth {
    state: AtomicU8,
    /// The model id ACTUALLY loaded (`byo:<dirname>` or the bge fallback
    /// id) — written only by [`RerankHealth::ready`], cleared by every other
    /// transition.
    model: Mutex<Option<String>>,
    /// The top-n the loader resolved — the knob actually in force — written
    /// with every transition so the echo reports the working value.
    top_n: AtomicUsize,
}

impl Default for RerankHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl RerankHealth {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(OFF),
            model: Mutex::new(None),
            top_n: AtomicUsize::new(crate::search::RERANK_TOP_N_DEFAULT),
        }
    }
    /// The lazy/warm load started (flag armed, `Reranker::new` in flight).
    pub fn loading(&self, top_n: usize) {
        self.store(RerankState::Loading, None, top_n);
    }

    /// The handle is live; the echo names the model that ACTUALLY loaded
    /// (`byo:<dirname>` or the bge fallback id) — a hardcoded golden label
    /// would lie the moment the seam loads a sibling.
    pub fn ready(&self, model: &str, top_n: usize) {
        self.store(RerankState::Ready, Some(model), top_n);
    }

    /// The load failed (recall falls back to the RRF order; the echo says
    /// so instead of leaving the tier silently dead).
    pub fn failed(&self, top_n: usize) {
        self.store(RerankState::Failed, None, top_n);
    }

    fn store(&self, state: RerankState, model: Option<&str>, top_n: usize) {
        self.state.store(state_byte(state), Ordering::Release);
        self.top_n.store(top_n, Ordering::Release);
        if let Ok(mut guard) = self.model.lock() {
            *guard = model.map(str::to_string);
        }
    }

    /// The `/health/db` admin echo: `{state, model, top_n}`. `ready` always
    /// names the model that actually loaded; every other state carries
    /// `model: null`.
    pub fn echo(&self) -> serde_json::Value {
        let byte = self.state.load(Ordering::Acquire);
        let model = self.model.lock().ok().and_then(|g| g.clone());
        serde_json::json!({
            "state": state_name(byte),
            "model": model,
            "top_n": self.top_n.load(Ordering::Acquire),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cell starts `off` with the default top-n and a null model — the
    /// shape a feature-less kernel reports forever.
    #[test]
    fn untouched_cell_reads_off() {
        let cell = RerankHealth::new();
        let echo = cell.echo();
        assert_eq!(echo["state"], "off");
        assert!(echo["model"].is_null());
        assert_eq!(echo["top_n"], crate::search::RERANK_TOP_N_DEFAULT);
    }

    /// The full vocabulary rides one cell; `ready` always names its model,
    /// every other transition clears it (P-6: never ready while None).
    #[test]
    fn transitions_are_honest() {
        let cell = RerankHealth::new();
        cell.loading(50);
        assert_eq!(cell.echo()["state"], "loading");
        assert!(cell.echo()["model"].is_null());

        cell.ready("byo:mxbai-rerank-large-v1", 50);
        assert_eq!(cell.echo()["state"], "ready");
        assert_eq!(cell.echo()["model"], "byo:mxbai-rerank-large-v1");

        // A load that follows a ready one reports honestly again.
        cell.failed(20);
        assert_eq!(cell.echo()["state"], "failed");
        assert!(cell.echo()["model"].is_null());
        assert_eq!(cell.echo()["top_n"], 20);
    }

    /// `ready` takes the model id by construction — there is no ready
    /// transition that could leave the model unnamed.
    #[test]
    fn ready_carries_its_model() {
        let cell = RerankHealth::new();
        cell.loading(50);
        cell.ready("BAAI/bge-reranker-v2-m3", 200);
        assert_eq!(cell.echo()["state"], "ready");
        assert_eq!(cell.echo()["model"], "BAAI/bge-reranker-v2-m3");
        assert_eq!(cell.echo()["top_n"], 200);
    }
}
