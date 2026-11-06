//! CG-12 P-7: a corrupt model dir degrades fail-soft with honest health.
//!
//! Own test binary (own process) because it ARMS the tier — the reranker's
//! LazyLock loads once per process, so this file must never share a process
//! with a disabled-path pin (`tests/rerank_tier_pins.rs`).
//!
//! The corruption: a model dir whose `onnx/model_quantized.onnx` exists but
//! whose tokenizer files are missing — the BYO-ONNX seam refuses it
//! (`missing tokenizer.json`). The designed degradation is the in-registry
//! bge fallback; the pin makes THAT fail too, hermetically and without
//! touching the network for real: `HF_ENDPOINT` points at a dead loopback
//! port (fastembed's `pull_from_hf` reads it, `common.rs:269`) and `HF_HOME`
//! redirects the cache into the temp dir. The bge attempt fails in
//! milliseconds, `Reranker::new` errs, the LazyLock settles on `None`, and
//! the whole chain must hold:
//!
//! - the recall-path call does not panic and does not stall,
//! - the RRF order stands untouched,
//! - `rerank_score` is absent on every hit (provenance honesty),
//! - the health echo says `failed` with no model — never a silent dead tier.
#![cfg(feature = "rerank-tier")]

use brain_server::search::rerank::maybe_rerank;
use brain_server::search::rerank_health::RERANK_HEALTH;
use brain_server::search::{SearchResult, SearchTelemetry};

#[test]
fn corrupt_model_dir_fails_soft_with_honest_health() {
    let corrupt = std::env::temp_dir().join(format!("cg12-corrupt-{}", std::process::id()));
    std::fs::create_dir_all(corrupt.join("onnx")).expect("corrupt dir scaffold");
    std::fs::write(
        corrupt.join("onnx").join("model_quantized.onnx"),
        b"NOT AN ONNX FILE (the seam refuses this dir via the missing tokenizer files)",
    )
    .expect("corrupt onnx bytes");

    // SAFETY: single-test binary; the env is written before anything in this
    // process reads it (the LazyLock forces inside the call below).
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("BRAIN_RERANK_ENABLED", "1");
        std::env::set_var("BRAIN_RERANK_MODEL_DIR", &corrupt);
        std::env::set_var("HF_ENDPOINT", "http://127.0.0.1:1");
        std::env::set_var("HF_HOME", corrupt.join("hf-cache"));
    }

    let mut hits: Vec<SearchResult> = (1..=5)
        .map(|id| SearchResult {
            id,
            score: 1.0 / id as f32,
            title: Some(format!("hit-{id}")),
            content: format!("content-{id}"),
            ..Default::default()
        })
        .collect();
    let incoming: Vec<i64> = hits.iter().map(|r| r.id).collect();

    let mut tel = SearchTelemetry::default();
    // The whole point: this call survives a corrupt model dir + an unreachable
    // fallback source — no panic, no stall (the first-query-503 class).
    maybe_rerank("does the fail-soft chain hold?", &mut hits, &mut tel);

    let after: Vec<i64> = hits.iter().map(|r| r.id).collect();
    assert_eq!(
        after, incoming,
        "the RRF order stands untouched through a corrupt model dir"
    );
    for hit in &hits {
        assert!(
            hit.provenance.rerank_score.is_none(),
            "hit {} carries a rerank_score from a tier that FAILED to load",
            hit.id
        );
    }

    let echo = RERANK_HEALTH.echo();
    assert_eq!(
        echo["state"], "failed",
        "the health echo names the failure instead of leaving the tier silently dead"
    );
    assert!(
        echo["model"].is_null(),
        "`failed` never names a model (nothing loaded)"
    );

    let _ = std::fs::remove_dir_all(&corrupt);
}
