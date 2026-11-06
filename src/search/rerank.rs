//! The profile-gated cross-encoder reranker.
//!
//! Sits after `rrf_fuse` (the rank-fusion stage in [`crate::search`]) and writes
//! into the reserved `rerank_score` / `rerank_truncated` slots on
//! [`crate::search::SearchResult`] provenance.
//!
//! Model resolution (highest quality first):
//!   1. `mixedbread-ai/mxbai-rerank-large-v1` — the golden pick (Apache-2.0,
//!      DeBERTa-v3-large single-label cross-encoder → `logits[:, 0]` score, which is the
//!      exact contract fastembed's `UserDefinedRerankingModel` seam expects). It is
//!      NOT in the FastEmbed registry enum (which only ships bge-reranker-base /
//!      bge-reranker-v2-m3 / jina v1-turbo / jina v2), so it's loaded through the
//!      BYO-ONNX seam from a local dir. Default location: `models/mxbai-rerank-large-v1/`
//!      (override with `BRAIN_RERANK_MODEL_DIR`). Uses the official int8
//!      `onnx/model_quantized.onnx` for CPU/footprint-friendly inference.
//!   2. Fallback: `bge-reranker-v2-m3` (FastEmbed-rs `BGERerankerV2M3`, in-enum,
//!      auto-downloads) when the mxbai files are absent or fail to load.
//!
//! Qwen3-Reranker-0.6B is deliberately NOT wired here: it is `Qwen3ForCausalLM`
//! (ChatML template, last-token logit scoring) — architecturally incompatible with
//! fastembed's rerank seam, which feeds bare `(query, doc)` pairs and reads
//! `logits[:, 0]`. It would load, run, and return meaningless scores. Using it
//! requires a real LLM runtime (llama.cpp / vLLM / TEI), out of scope for the seam.
//!
//! Profile gate: active only on `enterprise`/`desktop`. The Jetson/edge path
//! stays rerank-free (the tier was removed for the 8 s
//! recall timeout — re-added here only where the hardware can afford it).
//!
//! Fail-open contract: a model load failure or a per-call ONNX error leaves the
//! RRF order untouched (provenance stays `rerank_score = None`) — a recall never
//! panics or stalls on a reranker fault. This mirrors the `screen.rs` layer-2
//! fail-soft posture.
#![cfg(feature = "rerank-tier")]

use std::sync::{LazyLock, Mutex};

use fastembed::{
    OnnxSource, RerankInitOptions, RerankInitOptionsUserDefined, RerankerModel, TextRerank,
    TokenizerFiles, UserDefinedRerankingModel,
};

use crate::search::{SearchResult, SearchTelemetry};

/// Directory (relative to CWD or absolute) containing the mxbai-rerank-large-v1
/// files fastembed's BYO-ONNX seam needs: `onnx/model_quantized.onnx`,
/// `tokenizer.json`, `config.json`, `special_tokens_map.json`,
/// `tokenizer_config.json`. Override with `BRAIN_RERANK_MODEL_DIR`.
const DEFAULT_MXBAI_DIR: &str = "models/mxbai-rerank-large-v1";

/// Resolve + VALIDATE the model dir: an absolute path or an explicit env
/// override only. A CWD-relative default is refused — a writable working
/// directory would silently swap the model (supply-chain).
fn resolve_model_dir() -> Option<std::path::PathBuf> {
    let explicit = std::env::var("BRAIN_RERANK_MODEL_DIR")
        .ok()
        .map(std::path::PathBuf::from);
    let dir = explicit.unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_MXBAI_DIR));
    if dir.is_absolute() {
        Some(dir)
    } else {
        tracing::warn!(
            dir = %dir.display(),
            "rerank model dir is CWD-relative; refusing to load from a writable              CWD — set BRAIN_RERANK_MODEL_DIR to an absolute path"
        );
        None
    }
}

/// The process-wide reranker handle, loaded lazily on first recall iff
/// `BRAIN_RERANK_ENABLED=1`. The server boot sets that env var when the active
/// profile is `enterprise`/`desktop` (search/mod.rs is lib code and must not
/// depend on the server-private `config` module, so the gate is an env flag the
/// server owns). Mirrors `screen::CLASSIFIER`'s LazyLock<Option<...>> posture:
/// load-on-first-use, fail-soft to None on a model fault (recall falls back to
/// the RRF order — never stalls on a reranker fault).
static RERANKER: LazyLock<Option<Reranker>> = LazyLock::new(|| {
    if std::env::var("BRAIN_RERANK_ENABLED").as_deref() != Ok("1") {
        return None;
    }
    let top_n =
        crate::search::parse_rerank_top_n(std::env::var("BRAIN_RERANK_TOP_N").ok().as_deref());
    crate::search::rerank_health::RERANK_HEALTH.loading(top_n);
    match Reranker::new(top_n) {
        Ok(r) => {
            tracing::info!("reranker loaded: {} (top_n={top_n})", r.model_id());
            crate::search::rerank_health::RERANK_HEALTH.ready(r.model_id(), top_n);
            Some(r)
        }
        Err(e) => {
            tracing::warn!("reranker load failed (recall falls back to RRF order): {e}");
            crate::search::rerank_health::RERANK_HEALTH.failed(top_n);
            None
        }
    }
});

/// The seam `perform_search_with_prf` calls after fusion + PRF. No-op (preserves
/// the RRF order, leaves `rerank_score = None`) when the reranker is off (edge
/// profile) or failed to load. The CALL SITE owns the clock: it times this call
/// and records `rerank_ms` into telemetry whether or not a rerank ran, so the
/// provenance stays honest about whether a rerank stage ran (this function
/// takes `_tel` and never writes it — the ranking probe's misreading warning).
pub fn maybe_rerank(query: &str, candidates: &mut [SearchResult], _tel: &mut SearchTelemetry) {
    let Some(r) = RERANKER.as_ref() else {
        return; // edge profile / load failure — RRF order stands
    };
    let _ = r.rerank(query, candidates); // fail-open inside
}

/// Force the lazy load at boot (the server calls this when it arms the tier).
/// Without it, the FIRST recall pays the model download inside the request
/// path — observed live: a 503 `recall timed out` on the first query while
/// bge-reranker-v2-m3 downloaded. Warm it before serving.
pub fn warmup() {
    LazyLock::force(&RERANKER);
}

/// One cross-encoder verdict: the window index it scored and the score it gave.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScoredIndex {
    pub index: usize,
    pub score: f32,
}

/// The pure reorder law (the rerank round's derivation, pinned without ONNX in the
/// module tests below): given the incoming window — RRF fusion order, already
/// freshness-tie-broken (`observed_at` → `authority` → trust) — and the
/// cross-encoder's verdicts, return the window reordered so that
///
/// 1. the rerank score strictly orders the window: for reranked candidates
///    the reranker REPLACES the candidate order (the CG-02 tie-break law
///    ordered EQUAL FUSED scores; that order has done its job by the time the
///    window is scored — it decided which candidates made the window);
/// 2. EQUAL rerank scores defer to the incoming order: the total sort key is
///    (score DESC, index ASC), so a tie is deterministic instead of depending
///    on the ONNX session's emission order — the trust tier stays a LABEL
///    riding provenance, never a post-rerank ordering force;
/// 3. a hit carries `rerank_score` IFF the rerank placed it (P-8's honesty
///    law), and `rerank_truncated` rides exactly the scored window;
/// 4. a candidate the scorer didn't return keeps its incoming order with no
///    rerank score — nothing silently dropped (the placed-guard law).
pub(crate) fn apply_rerank_order(
    window: Vec<SearchResult>,
    scored: Vec<ScoredIndex>,
    truncated: bool,
) -> Vec<SearchResult> {
    let n = window.len();
    let mut scored = scored;
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.index.cmp(&b.index))
    });
    let mut placed = vec![false; n];
    let mut reordered: Vec<SearchResult> = Vec::with_capacity(n);
    for s in &scored {
        if s.index < n && !placed[s.index] {
            placed[s.index] = true;
            let mut item = window[s.index].clone();
            item.provenance.rerank_score = Some(s.score);
            item.provenance.rerank_truncated = truncated;
            reordered.push(item);
        }
    }
    for (i, item) in window.into_iter().enumerate() {
        if !placed[i] {
            reordered.push(item);
        }
    }
    reordered
}

/// The reranker handle, loaded once (the ONNX session is the expensive part).
/// `TextRerank::rerank` takes `&mut self` (ONNX scratch buffers), so the
/// `Mutex` lets the shared handle stay `&self` for the call sites — the lock is
/// uncontended under brain-server's pooled-connection, one-embed-per-task model
/// (same pattern as `embed::NeuralEmbedder` and `screen::onnx::OnnxScorer`).
pub struct Reranker {
    /// Lock bounds (Headroom): the critical section is the cross-encoder
    /// scoring pass — CPU-bound, long hold, no I/O, no nesting, no SQL. The
    /// acquire wait is the rerank-tier's model-queue signal. Poison:
    /// fail-OPEN (RRF order stands — a reranker fault never propagates to
    /// recall). Request-path holder, wait-measured.
    inner: Mutex<TextRerank>,
    /// The model id actually loaded (mxbai-rerank-large-v1 or bge-reranker-v2-m3).
    model_id: std::sync::Arc<str>,
    /// The max candidates scored per call. Reranking more than ~50–100 costs
    /// latency for diminishing rank-quality gain (IronCurtain's own cascade
    /// discipline uses ~50). Larger candidate sets are truncated and the
    /// `rerank_truncated` provenance flag reports it honestly.
    top_n: usize,
}

impl Reranker {
    /// `model_id() -> &str` reports which reranker actually loaded.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Try the golden `mxbai-rerank-large-v1` (BYO-ONNX seam, dir from
    /// `BRAIN_RERANK_MODEL_DIR` or the default), falling back to the in-registry
    /// `bge-reranker-v2-m3` when the XML files are absent or the load fails.
    /// Fail-open sits with the caller (`LazyLock` → `None`), so a missing model
    /// dir degrades to the in-enum model, never to a boot failure. The model id
    /// reports the dir ACTUALLY loaded (`byo:<dirname>`) — a hardcoded golden
    /// label would lie the moment an operator points the seam at a sibling
    /// (the base-tier swap does exactly that).
    pub fn new(top_n: usize) -> anyhow::Result<Self> {
        // Prefer the user-defined model via the BYO-ONNX seam.
        match Self::new_mxbai_user_defined(top_n) {
            Ok(inner) => {
                let byo_label = resolve_model_dir()
                    .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "model".to_string());
                Ok(Self {
                    inner: Mutex::new(inner),
                    model_id: std::sync::Arc::from(format!("byo:{byo_label}").as_str()),
                    top_n,
                })
            }
            Err(e) => {
                tracing::warn!(
                    "BYO rerank model unavailable ({e}); falling back to bge-reranker-v2-m3"
                );
                let options = RerankInitOptions::new(RerankerModel::BGERerankerV2M3)
                    .with_show_download_progress(true);
                let inner = TextRerank::try_new(options)?;
                Ok(Self {
                    inner: Mutex::new(inner),
                    model_id: std::sync::Arc::from("BAAI/bge-reranker-v2-m3"),
                    top_n,
                })
            }
        }
    }

    /// Load mxbai-rerank-large-v1 (int8) through fastembed's BYO-ONNX seam.
    /// Requires `onnx/model_quantized.onnx` + the 4 tokenizer files in the model
    /// dir. Errors (rather than panics) if any file is missing — the caller falls
    /// back to bge-reranker-v2-m3. `_top_n` is reserved for a future per-call
    /// max_length cap; model_max_length (512) bounds the tokenizer today.
    fn new_mxbai_user_defined(_top_n: usize) -> anyhow::Result<TextRerank> {
        // Fail-closed on a CWD-relative dir (writable-CWD model swap).
        let Some(dir) = resolve_model_dir() else {
            anyhow::bail!("rerank model dir must be absolute or BRAIN_RERANK_MODEL_DIR set");
        };
        let model = UserDefinedRerankingModel::new(
            OnnxSource::File(dir.join("onnx").join("model_quantized.onnx")),
            TokenizerFiles {
                tokenizer_file: std::fs::read(dir.join("tokenizer.json"))
                    .map_err(|e| anyhow::anyhow!("missing tokenizer.json: {e}"))?,
                config_file: std::fs::read(dir.join("config.json"))
                    .map_err(|e| anyhow::anyhow!("missing config.json: {e}"))?,
                special_tokens_map_file: std::fs::read(dir.join("special_tokens_map.json"))
                    .map_err(|e| anyhow::anyhow!("missing special_tokens_map.json: {e}"))?,
                tokenizer_config_file: std::fs::read(dir.join("tokenizer_config.json"))
                    .map_err(|e| anyhow::anyhow!("missing tokenizer_config.json: {e}"))?,
            },
        );
        let options = RerankInitOptionsUserDefined::new();
        TextRerank::try_new_from_user_defined(model, options)
            .map_err(|e| anyhow::anyhow!("fastembed rerank init failed: {e}"))
    }

    /// Rerank the fused candidates in place. Sets `rerank_score` on each
    /// survivor; sets `rerank_truncated = true` when the input exceeded
    /// `top_n` (so provenance honestly reports low-rank candidates were
    /// dropped before scoring).
    ///
    /// Fail-open: on any ONNX/lock error, leaves `fused` in its RRF order and
    /// returns `Ok(())` — never propagates a reranker fault to the recall path.
    /// The caller times the call and records `rerank_ms` into telemetry.
    pub fn rerank(&self, query: &str, fused: &mut [SearchResult]) -> anyhow::Result<()> {
        if fused.len() <= 1 {
            return Ok(()); // nothing to order
        }
        let truncated = fused.len() > self.top_n;
        let n = fused.len().min(self.top_n);

        let mut guard = match crate::concurrency::mutex_guard_measured(&self.inner) {
            Ok(g) => g,
            Err(_) => return Ok(()), // poisoned — fail open
        };
        // Score the top-n by RRF order.
        let docs: Vec<&str> = fused[..n].iter().map(|r| r.content.as_str()).collect();
        let results = match guard.rerank(query, &docs, false, None) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("reranker call failed (fail-open to RRF order): {e}");
                return Ok(());
            }
        };
        // The window is pulled out and reordered by the PURE law
        // (`apply_rerank_order` — unit-pinned without ONNX in the module
        // tests, and from `tests/rerank_tier_pins.rs`).
        let window: Vec<SearchResult> = fused[..n].to_vec();
        let scored: Vec<ScoredIndex> = results
            .into_iter()
            .map(|r| ScoredIndex {
                index: r.index,
                score: r.score,
            })
            .collect();
        let reordered = apply_rerank_order(window, scored, truncated);
        // Splice the reordered top-n back; the un-reranked tail (n..) keeps its
        // RRF order, flagged truncated so the consumer sees they were dropped
        // from scoring.
        fused[..n].clone_from_slice(&reordered);
        if truncated {
            for r in &mut fused[n..] {
                r.provenance.rerank_truncated = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod rerank_order_law_tests {
    use super::{ScoredIndex, apply_rerank_order};
    use crate::search::{SearchResult, TrustTier};

    fn hit(id: i64, trust: Option<TrustTier>) -> SearchResult {
        SearchResult {
            id,
            score: 1.0 / (id as f32 + 1.0), // the RRF-ish incoming distinctness
            title: Some(format!("hit-{id}")),
            content: format!("content-{id}"),
            trust,
            ..Default::default()
        }
    }

    fn window(trusts: &[Option<TrustTier>]) -> Vec<SearchResult> {
        trusts
            .iter()
            .enumerate()
            .map(|(i, t)| hit(i as i64, *t))
            .collect()
    }

    /// The reranker's order replaces the incoming order for distinct scores.
    #[test]
    fn rerank_order_replaces_incoming_order() {
        let w = window(&[None, None, None]);
        let scored = vec![
            ScoredIndex {
                index: 2,
                score: 0.9,
            },
            ScoredIndex {
                index: 0,
                score: 0.5,
            },
            ScoredIndex {
                index: 1,
                score: 0.1,
            },
        ];
        let out = apply_rerank_order(w, scored, false);
        let ids: Vec<i64> = out.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 0, 1]);
        assert_eq!(out[0].provenance.rerank_score, Some(0.9));
        assert_eq!(out[1].provenance.rerank_score, Some(0.5));
        assert_eq!(out[2].provenance.rerank_score, Some(0.1));
        assert!(!out.iter().any(|r| r.provenance.rerank_truncated));
    }

    /// THE COMPOSITION LAW: equal rerank scores defer to the incoming order —
    /// regardless of the order the scorer emitted its ties in.
    #[test]
    fn equal_rerank_scores_defer_to_incoming_order() {
        let w = window(&[None, None, None]);
        // Scorer emits the tie BACKWARDS (index 2 before index 0) — the law
        // must still keep 0 ahead of 1 inside the tie bucket.
        let scored = vec![
            ScoredIndex {
                index: 2,
                score: 0.9,
            },
            ScoredIndex {
                index: 1,
                score: 0.5,
            },
            ScoredIndex {
                index: 0,
                score: 0.5,
            },
        ];
        let out = apply_rerank_order(w, scored, false);
        let ids: Vec<i64> = out.iter().map(|r| r.id).collect();
        assert_eq!(
            ids,
            vec![2, 0, 1],
            "tie {{0,1}} resolves to the incoming order"
        );
    }

    /// The trust tier is a LABEL riding provenance: it travels with its hit
    /// through the reorder and is never a post-rerank ordering force.
    #[test]
    fn trust_labels_ride_their_hits_through_the_reorder() {
        let w = window(&[
            Some(TrustTier::Published),
            Some(TrustTier::Raw),
            Some(TrustTier::Approved),
        ]);
        let scored = vec![
            ScoredIndex {
                index: 1,
                score: 0.9,
            }, // Raw wins the rerank
            ScoredIndex {
                index: 0,
                score: 0.2,
            }, // Published demoted
            ScoredIndex {
                index: 2,
                score: 0.1,
            },
        ];
        let out = apply_rerank_order(w, scored, false);
        assert_eq!(out[0].trust, Some(TrustTier::Raw), "the label rides");
        assert_eq!(out[1].trust, Some(TrustTier::Published));
        assert_eq!(out[2].trust, Some(TrustTier::Approved));
        // And the ordering that produced this is the RERANK score, not the
        // trust tier: Raw > Published > Approved in the output order.
        let ids: Vec<i64> = out.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 0, 2]);
    }

    /// Provenance honesty (P-8's construction half): a hit carries
    /// rerank_score IFF the rerank placed it; unscored candidates keep their
    /// incoming order unscored, and nothing is silently dropped.
    #[test]
    fn unscored_candidates_keep_order_and_stay_unscored() {
        let w = window(&[None, None, None, None]);
        let scored = vec![ScoredIndex {
            index: 3,
            score: 0.7,
        }];
        let out = apply_rerank_order(w, scored, true);
        assert_eq!(out[0].id, 3);
        assert_eq!(out[0].provenance.rerank_score, Some(0.7));
        assert!(out[0].provenance.rerank_truncated);
        for r in &out[1..] {
            assert!(
                r.provenance.rerank_score.is_none(),
                "unscored stays unscored"
            );
            // NOT truncated: the flag's contract is "length-capped before
            // reranking" — these were IN the scored window, the scorer simply
            // failed to return them. Claiming truncation would be a guess.
            assert!(!r.provenance.rerank_truncated);
        }
        let ids: Vec<i64> = out.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![3, 0, 1, 2], "unscored keep incoming order");
    }

    /// Duplicate verdicts and out-of-range indices are absorbed: first
    /// (highest-score) wins, garbage is ignored, the window never changes size.
    #[test]
    fn duplicate_and_out_of_range_verdicts_are_absorbed() {
        let w = window(&[None, None]);
        let scored = vec![
            ScoredIndex {
                index: 1,
                score: 0.9,
            },
            ScoredIndex {
                index: 1,
                score: 0.8,
            }, // duplicate — absorbed
            ScoredIndex {
                index: 9,
                score: 0.99,
            }, // out of range — ignored
            ScoredIndex {
                index: 0,
                score: 0.1,
            },
        ];
        let out = apply_rerank_order(w, scored, false);
        assert_eq!(out.len(), 2, "the window never grows");
        assert_eq!(out[0].id, 1);
        assert_eq!(out[0].provenance.rerank_score, Some(0.9), "first wins");
        assert_eq!(out[1].id, 0);
    }

    /// NaN verdicts cannot poison the order (partial_cmp falls back to the
    /// index key, so the tie law absorbs the garbage deterministically).
    #[test]
    fn nan_scores_fall_back_to_the_index_key() {
        let w = window(&[None, None]);
        let scored = vec![
            ScoredIndex {
                index: 1,
                score: f32::NAN,
            },
            ScoredIndex {
                index: 0,
                score: 0.4,
            },
        ];
        let out = apply_rerank_order(w, scored, false);
        let ids: Vec<i64> = out.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![0, 1]);
    }
}
