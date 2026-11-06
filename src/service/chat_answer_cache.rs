//! The greedy answer cache (CG-11 E3): the chat spine's bridge-turn path
//! answers a repeated `(model, prompt)` from the stored answer instead of a
//! second decode. Sound BY LAW, not by assumption: both serving tiers are
//! greedy-locked (the MLX tier is `temperature: 0` by construction in
//! `MLXBridge`; the AFM tier is greedy-locked by CG-09 S5), so the answer
//! is a pure function of the model and the prompt — which is ALSO why the
//! key must be EXACT (the wire model id included, so a tier flip never
//! serves another tier's answer) and why nothing sampled can ever enter:
//! the spine's request carries no sampling vocabulary at all (pinned).
//!
//! Boundaries, each named:
//! - **GDL propose never rides it.** The cache lives behind the chat
//!   spine's turn handler ONLY — the case machine's `BridgeTurn`s go
//!   through `agentloop::provider_afm` (fresh `gdl-` sessions, `model:
//!   None`, M-4's isolation law) and never touch this module (pinned).
//! - **In-memory, process-lifetime.** No new table (the lane's law). A
//!   process restart invalidates the whole cache — which is also the
//!   model-revision posture: the weights' revision crosses neither the
//!   wire nor the DB, but the revision a process loaded lives and dies
//!   with that process (the bridge's containers are cached per process),
//!   so a cache that never outlives the process can never serve an answer
//!   from weights that are no longer loaded.
//! - **Bounded, FIFO.** Each entry is one final's text; the bound is a
//!   floor on the honest workload (the chat spine's prompts are bounded at
//!   2,000 bytes + the grounding budget), not a tuning knob.
//! - **The prompt version is INSIDE the hash.** The instructions are part
//!   of the effective input; a `PROMPT_VERSION` bump is an input change
//!   and invalidates every cached answer — the same reasoning that puts
//!   the model id in the key.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use crate::agentloop::bridge_afm::BridgeClaim;

/// One cached answer: everything the spine's final event and its
/// persistence need from the turn that produced it. `tier` is the
/// producing tier (the answer's provenance — a cached `mlx_native` answer
/// is still an `mlx_native` answer); the token counts are NOT cached
/// because a served hit generates zero tokens this turn and the wire says
/// so honestly (0/0).
#[derive(Debug, Clone)]
pub struct CachedAnswer {
    pub text: String,
    pub tier: String,
    pub abstain: bool,
    pub claims: Vec<BridgeClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// The wire model id (`BRAIN_CHAT_MODEL`), empty string for the AFM
    /// default — the same vocabulary the turn op transports.
    model: String,
    /// SHA-256 over (prompt version, NUL, prompt bytes).
    prompt: [u8; 32],
}

/// The bound. A chat answer is bounded by the response clamp (≤ 1,024
/// tokens) and the prompts by the spine's own laws; 1,024 entries is years
/// of local-first chat at the product's scale, and the eviction is FIFO —
/// an overfull cache drops its oldest answers, never its correctness.
const CACHE_CAP: usize = 1_024;

/// The process-lifetime cache. Lock discipline mirrors the kernel's other
/// small mutexes: clone-out under the guard, no I/O, no re-entrancy.
pub struct ChatAnswerCache {
    inner: Mutex<ChatAnswerCacheInner>,
}

#[derive(Default)]
struct ChatAnswerCacheInner {
    map: HashMap<CacheKey, CachedAnswer>,
    order: VecDeque<CacheKey>,
}

impl Default for ChatAnswerCache {
    fn default() -> Self {
        Self {
            inner: Mutex::new(ChatAnswerCacheInner::default()),
        }
    }
}

impl ChatAnswerCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The EXACT key: (wire model id, prompt hash). The prompt hash covers
    /// the prompt version and the full prompt the bridge would see — the
    /// fenced context and the message; two turns with the same prompt and
    /// the same model are the same greedy computation by law.
    pub fn key(model: Option<&str>, prompt_version: &str, prompt: &str) -> CacheKey {
        let mut hasher = Sha256::new();
        hasher.update(prompt_version.as_bytes());
        hasher.update([0u8]);
        hasher.update(prompt.as_bytes());
        CacheKey {
            model: model.unwrap_or_default().to_owned(),
            prompt: hasher.finalize().into(),
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<CachedAnswer> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map
            .get(key)
            .cloned()
    }

    pub fn store(&self, key: CacheKey, answer: CachedAnswer) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if inner.map.contains_key(&key) {
            return;
        }
        if inner.map.len() >= CACHE_CAP
            && let Some(oldest) = inner.order.pop_front()
        {
            inner.map.remove(&oldest);
        }
        inner.order.push_back(key.clone());
        inner.map.insert(key, answer);
    }

    /// The count — the cache witness arm's secondary signal (a repeat turn
    /// must not grow the cache).
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map
            .len()
    }

    /// True when nothing is cached yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(text: &str) -> CachedAnswer {
        CachedAnswer {
            text: text.to_owned(),
            tier: "mlx_native".to_owned(),
            abstain: false,
            claims: vec![],
        }
    }

    /// THE KEY IS EXACT: the model id binds (a tier flip never serves
    /// another tier's answer), the prompt version binds (an instructions
    /// bump invalidates), and equal inputs give equal keys.
    #[test]
    fn the_cache_key_binds_the_model_and_the_prompt_version() {
        let afm = key_of(None, "chat-27v2", "hello");
        let mlx = key_of(Some("qwen3-8b-4bit"), "chat-27v2", "hello");
        assert_ne!(
            afm, mlx,
            "the model id is part of the key — a tier flip cannot hit"
        );
        let v2 = key_of(Some("qwen3-8b-4bit"), "chat-27v3", "hello");
        assert_ne!(
            mlx, v2,
            "the prompt version is inside the hash — a bump invalidates"
        );
        assert_eq!(
            key_of(Some("qwen3-8b-4bit"), "chat-27v2", "hello"),
            mlx,
            "equal (model, version, prompt) is the same computation"
        );
        assert_eq!(
            key_of(None, "chat-27v2", "hello"),
            key_of(Some(""), "chat-27v2", "hello"),
            "no configured tier and an empty id are the same key (the AFM default)"
        );
    }

    fn key_of(model: Option<&str>, version: &str, prompt: &str) -> CacheKey {
        ChatAnswerCache::key(model, version, prompt)
    }

    /// THE ROUND TRIP: store → get → Some; an unknown key → None; a repeat
    /// store does not grow the cache; the bound evicts FIFO.
    #[test]
    fn the_cache_round_trips_and_stays_bounded() {
        let cache = ChatAnswerCache::new();
        let k1 = key_of(Some("qwen3-8b-4bit"), "v", "prompt one");
        cache.store(k1.clone(), answer("answer one"));
        assert_eq!(cache.get(&k1).unwrap().text, "answer one");
        assert_eq!(cache.len(), 1);

        let miss = key_of(Some("qwen3-8b-4bit"), "v", "prompt TWO");
        assert!(cache.get(&miss).is_none(), "an unknown prompt is a miss");

        cache.store(k1.clone(), answer("answer one AGAIN"));
        assert_eq!(cache.len(), 1, "a repeat store does not grow the cache");
        assert_eq!(
            cache.get(&k1).unwrap().text,
            "answer one",
            "the FIRST answer stands (the hit already served it)"
        );

        for i in 0..CACHE_CAP + 8 {
            cache.store(
                key_of(Some("qwen3-8b-4bit"), "v", &format!("prompt {i}")),
                answer(&format!("answer {i}")),
            );
        }
        assert!(
            cache.len() <= CACHE_CAP,
            "the cache never exceeds its bound: {}",
            cache.len()
        );
    }
}
