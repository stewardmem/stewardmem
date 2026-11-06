//! CG-12 P-8's disabled-path pin: hits carry `rerank_score` IFF the rerank ran.
//!
//! This binary NEVER arms the tier — it sets no env and touches no model — so
//! the first `maybe_rerank` call forces the LazyLock with the flag unset, the
//! handle is `None` for the life of this process, and the slots must stay
//! empty. A mutant that fills the slots while the tier is disabled REDS here
//! (damage-proofed in the evidence transcript: the `_tel`-ignoring no-op was
//! replaced by a slot-filling body and this file failed).
//!
//! The reranker-disabled state is also the DEFAULT-FEATURES build's permanent
//! state (the module is compiled out there), so this pin's law is exactly the
//! wire law a default kernel shows forever.
#![cfg(feature = "rerank-tier")]

use brain_server::search::rerank::maybe_rerank;
use brain_server::search::{SearchResult, SearchTelemetry};

fn candidates() -> Vec<SearchResult> {
    (1..=5)
        .map(|id| SearchResult {
            id,
            score: 1.0 / id as f32,
            title: Some(format!("hit-{id}")),
            content: format!("content-{id}"),
            ..Default::default()
        })
        .collect()
}

#[test]
fn slots_stay_empty_while_the_tier_is_disabled() {
    // The guard: if anything in this process armed the tier before this test
    // ran, the precondition is broken and the pin must say so loudly instead
    // of silently asserting nothing.
    let echo = brain_server::search::rerank_health::RERANK_HEALTH.echo();
    assert_eq!(
        echo["state"], "off",
        "this pin's process must never arm the tier (P-8's disabled path)"
    );

    let mut hits = candidates();
    let mut tel = SearchTelemetry::default();
    maybe_rerank("any query", &mut hits, &mut tel);

    for hit in &hits {
        assert!(
            hit.provenance.rerank_score.is_none(),
            "hit {} carries a rerank_score while the tier is DISABLED — provenance is lying",
            hit.id
        );
        assert!(
            !hit.provenance.rerank_truncated,
            "hit {} claims truncation while the tier is DISABLED",
            hit.id
        );
    }
    // The order the caller passed in comes back untouched: the no-op never
    // reorders (the RRF order stands — the fail-open contract's off state).
    let ids: Vec<i64> = hits.iter().map(|r| r.id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);
}
