//! The near-miss collision surface (round 20, S3 — the DISCERN adoption's
//! conditional mechanism, landed because the trap probe MEASURED the failure
//! class here: contamination 1.000, 4/4 near-miss queries served the
//! confusable twin's chunk, `docs/TRAP_ROWS_2026-11-14.jsonl`).
//!
//! DISCERN's shape, in this store's deterministic form: confusable entities
//! stay co-equal, linked, NEVER merged — and a query that names an entity
//! the store never stored routes to the clarify-or-abstain vocabulary
//! instead of committing a confusable neighbor's chunk. The LLM parts of
//! the paper's mechanism (the EAV entity extraction, the navigation loop)
//! are refused by name; what transfers is exactly the graph-decidable part:
//!
//! - THE NAME-EDGE FORMULA (preregistered, verbatim from the plan and
//!   research note 25): `0.6·jaccard_lex(names) + 0.4·cos(embeddings)`,
//!   threshold-gated, mechanical, no LLM. The "names" surface is extracted
//!   WITHOUT any entity layer — capitalized-token runs (the mechanical
//!   proxy; this store's entity names are client-declared text, and the
//!   trap corpus's names live in the content).
//! - THE NEAR-MISS DETECTOR: a query NAMES an entity the served set
//!   provably lacks when it carries a capitalized mid-query token (a
//!   name-like token) that appears in NO served row, while sharing ≥2
//!   tokens with the top row (the confusable family the retriever served
//!   instead) and the top row sits in a real collision family (≥1
//!   above-threshold name-edge to another served row — the formula gates
//!   the declaration, so a lone hit cannot manufacture a near-miss).
//!
//! The ROUTE (the recall seam, `BRAIN_RECALL_NEARMISS=1`, OFF at rest — no
//! default moves) drops the whole served set for a routed query and records
//! the witness on the telemetry: every served row is wrong-entity evidence,
//! and the kernel's own zero-hit abstain gate composes downstream (the
//! established empty `low_confidence` envelope — the ClarifyQuery mapping,
//! unchanged). The rows stay in the store untouched — never merged, never
//! rewritten. The false-decline guard is the store-grounded presence check
//! (the FTS leg, in `search`): a token PRESENT in the domain's corpus
//! means the real entity may exist — the contrast stands down and the
//! retrieval's own ranking stands.

/// The name-edge formula's weights (preregistered: DISCERN's
/// `0.6·jaccard_lex(names) + 0.4·cos(embeddings)`).
pub const EDGE_W_JACCARD: f64 = 0.6;
pub const EDGE_W_COS: f64 = 0.4;

/// The name-edge threshold (the round's own preregistered pin: the paper
/// publishes the formula but not the threshold value; 0.70 follows the
/// Graphiti duplicate-probability line the store's lineage already sits on).
pub const EDGE_THRESHOLD: f64 = 0.70;

/// One normalized name surface: the lowercase tokens of a capitalized run.
pub type Surface = Vec<String>;

/// The deterministic name-surface extractor: runs of ≥2 consecutive
/// capitalized alphanumeric tokens (the mechanical proxy for entity names —
/// no EAV layer, no LLM). Sentence-internal lowercase words break runs;
/// a lone capitalized token (a sentence start with no capitalized neighbor,
/// an acronym standing alone) forms no surface. The heuristic's known noise
/// (a sentence-initial "The" riding an adjacent acronym into a run) is
/// gated downstream: only above-threshold name-edges become family.
pub fn name_surfaces(text: &str) -> Vec<Surface> {
    let mut out = Vec::new();
    let mut run: Vec<String> = Vec::new();
    for raw in text.split_whitespace() {
        let tok: String = raw.chars().filter(|c| c.is_alphanumeric()).collect();
        let name_like = tok.len() >= 2
            && tok.chars().next().is_some_and(|c| c.is_uppercase())
            && tok.chars().all(|c| c.is_alphanumeric());
        if name_like {
            run.push(tok.to_lowercase());
        } else if run.len() >= 2 {
            out.push(std::mem::take(&mut run));
        } else {
            run.clear();
        }
    }
    if run.len() >= 2 {
        out.push(run);
    }
    out
}

/// THE PREREGISTERED NAME-EDGE FORMULA: `0.6·jaccard_lex + 0.4·cos`.
/// The jaccard leg REUSES the packing module's token-set jaccard (the dup
/// guard's law — no second copy); the cos leg rides [`cosine_sim`]'s
/// embedding similarity. Both widen into the f64 formula unchanged.
pub fn name_edge_score(a: &Surface, b: &Surface, cos: f64) -> f64 {
    EDGE_W_JACCARD * crate::search::packing::jaccard(a, b) as f64 + EDGE_W_COS * cos
}

/// The one normalizer for this module's token math — the SAME declared
/// shape the trap probe grades with (lowercase, every non-alphanumeric
/// char a space, whitespace collapsed), so the detector's tokens and the
/// probe's spans read the same strings.
pub(crate) fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The near-miss witness (telemetry): the distinguishing token plus the
/// served collision family's row ids. Never a merge — the rows stay in the
/// store untouched.
pub struct NearMiss {
    pub token: String,
    pub family: Vec<i64>,
}

/// The telemetry-serializable form of the witness (SearchTelemetry carries
/// it; `near_miss` is `None` unless the contrast routed).
#[derive(Debug, Clone, serde::Serialize)]
pub struct NearMissWitness {
    pub token: String,
    pub family: Vec<i64>,
}

impl From<NearMiss> for NearMissWitness {
    fn from(n: NearMiss) -> Self {
        Self {
            token: n.token,
            family: n.family,
        }
    }
}

/// The near-miss detector over one domain's served results (pure except the
/// embedding call, which is the store's own deterministic vectorizer).
/// Returns the witness when the query NAMES an entity the served set
/// provably lacks. `None` otherwise — the retrieval's own ranking stands.
pub fn detect(
    query: &str,
    results: &[crate::SearchResult],
    model: &dyn crate::embed::Embedder,
) -> Option<NearMiss> {
    if results.is_empty() {
        return None;
    }
    // 1. The distinguishing name-token: capitalized mid-query, absent from
    //    EVERY served row's content (normalized token match). The top row's
    //    normalized content leads the token set (`normed[0]` below).
    let normed: Vec<String> = results.iter().map(|r| norm(&r.content)).collect();
    let mut token: Option<String> = None;
    for (i, raw) in query.split_whitespace().enumerate() {
        if i == 0 {
            continue; // sentence-initial caps are grammar, not names
        }
        let tok: String = raw.chars().filter(|c| c.is_alphanumeric()).collect();
        if tok.len() < 3 || !tok.chars().next().is_some_and(|c| c.is_uppercase()) {
            continue;
        }
        let low = tok.to_lowercase();
        if !normed.iter().any(|n| n.split(' ').any(|w| w == low)) {
            token = Some(low);
            break;
        }
    }
    let token = token?;
    // 2. The confusable family was served: ≥2 normalized query tokens in
    //    the top row's content.
    let top_tokens: std::collections::HashSet<&str> = normed[0].split(' ').collect();
    let q_tokens: Vec<String> = norm(query).split(' ').map(str::to_string).collect();
    let shared = q_tokens
        .iter()
        .filter(|t| top_tokens.contains(t.as_str()))
        .count();
    if shared < 2 {
        return None;
    }
    // 3. The collision family per the preregistered formula: the top row
    //    must sit in a REAL family (≥1 above-threshold name-edge to another
    //    served row) — a lone hit cannot manufacture a near-miss.
    let mut surfaces: Vec<Surface> = Vec::new();
    let mut idx: Vec<Vec<usize>> = Vec::new();
    for r in results.iter().take(8) {
        let mut ii = Vec::new();
        for s in name_surfaces(&r.content).into_iter().take(2) {
            ii.push(surfaces.len());
            surfaces.push(s);
        }
        idx.push(ii);
    }
    if idx.first().is_none_or(|ii| ii.is_empty()) {
        return None;
    }
    let flat: Vec<String> = surfaces.iter().map(|s| s.join(" ")).collect();
    let vecs = model.encode(&flat.iter().map(String::as_str).collect::<Vec<&str>>());
    let cos_of = |i: usize, j: usize| -> f64 {
        vecs.get(i)
            .and_then(|v| vecs.get(j).map(|w| crate::search::cosine_sim(v, w) as f64))
            .unwrap_or(0.0)
    };
    let top_idx = &idx[0];
    let mut family = Vec::new();
    for (ri, r) in results.iter().enumerate().take(8).skip(1) {
        let in_family = idx.get(ri).is_some_and(|r_idx| {
            top_idx.iter().any(|ti| {
                r_idx.iter().any(|si| {
                    let (t, s) = (&surfaces[*ti], &surfaces[*si]);
                    name_edge_score(t, s, cos_of(*ti, *si)) >= EDGE_THRESHOLD
                })
            })
        });
        if in_family {
            family.push(r.id);
        }
    }
    if family.is_empty() {
        return None;
    }
    Some(NearMiss { token, family })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: i64, content: &str) -> crate::SearchResult {
        crate::SearchResult {
            id,
            content: content.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn name_surfaces_extract_capitalized_runs_only() {
        let s = name_surfaces("Zephyron Cloud Solutions was founded in 2014 by Acme.");
        assert_eq!(s, vec![vec!["zephyron", "cloud", "solutions"]]);
        // a run broken by a lowercase word does not span it; the known
        // noise — a sentence-initial "The" riding an adjacent acronym —
        // forms a run the threshold gates downstream
        let s2 = name_surfaces("The CEO of Meridian Robotics is Lena Hartmann.");
        assert_eq!(
            s2,
            vec![
                vec!["the", "ceo"],
                vec!["meridian", "robotics"],
                vec!["lena", "hartmann"]
            ]
        );
        // sentence-initial caps and single tokens form no surface
        assert!(name_surfaces("Where is the market?").is_empty());
    }

    #[test]
    fn the_preregistered_formula_weights() {
        // jaccard 1.0, cos 1.0 → 0.6 + 0.4 = 1.0
        let a = vec!["zephyron".to_string(), "cloud".to_string()];
        assert!((name_edge_score(&a, &a, 1.0) - 1.0).abs() < 1e-9);
        // jaccard 0, cos 0 → 0.0
        let b = vec!["northwind".to_string()];
        assert!((name_edge_score(&a, &b, 0.0)).abs() < 1e-9);
        // the threshold gate at the pinned 0.70
        assert!(name_edge_score(&a, &a, 0.25) >= EDGE_THRESHOLD);
        assert!(name_edge_score(&a, &b, 0.5) < EDGE_THRESHOLD);
    }

    #[test]
    fn the_reused_similarity_legs_edge_cases() {
        // the jaccard leg is the packing module's own helper (no second copy)
        let a = vec!["a".to_string()];
        assert_eq!(
            crate::search::packing::jaccard(&[], &["x".to_string()]),
            0.0
        );
        assert!((crate::search::packing::jaccard(&a, &a) - 1.0).abs() < 1e-9);
        // the cos leg is the search module's own helper (it zips shorter on
        // a length mismatch — a case same-dim embedders never produce)
        assert!((crate::search::cosine_sim(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((crate::search::cosine_sim(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert_eq!(crate::search::cosine_sim(&[], &[1.0, 2.0]), 0.0); // empty guard
        assert_eq!(crate::search::cosine_sim(&[0.0, 0.0], &[1.0, 1.0]), 0.0); // zero norm
        // and the formula widens them unchanged: jaccard 0, cos 0 → 0.0
        let b = vec!["northwind".to_string()];
        assert!(name_edge_score(&a, &b, 0.0).abs() < 1e-9);
    }

    /// A stub embedder: identical strings → identical vectors, distinct
    /// strings → far-apart ones. Deterministic, dependency-free.
    struct StubEmbedder;
    impl crate::embed::Embedder for StubEmbedder {
        fn encode(&self, texts: &[&str]) -> Vec<Vec<f32>> {
            texts
                .iter()
                .map(|t| {
                    // hash the text into a unit-ish vector, identical text →
                    // identical vector, different text → far apart
                    let h = t
                        .bytes()
                        .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
                    vec![
                        (h % 1000) as f32 / 1000.0,
                        ((h / 1000) % 1000) as f32 / 1000.0 + 0.5,
                    ]
                })
                .collect()
        }
        fn model_id(&self) -> &str {
            "stub-nearmiss-test"
        }
    }

    #[test]
    fn detect_fires_on_the_near_miss_shape() {
        let results = vec![
            result(3, "Zephyron Cloud Solutions was founded in 2014."),
            result(2, "Zephyron Cloud Solutions is headquartered in Seattle."),
            result(
                1,
                "Zephyron Cloud Solutions reported annual revenue of 5.1 billion dollars for fiscal 2025.",
            ),
            result(7, "Northwind Analytics is headquartered in Rotterdam."),
        ];
        let nm = detect(
            "What is the revenue of Zephyron Cloud Systems?",
            &results,
            &StubEmbedder,
        )
        .expect("the near-miss shape fires");
        assert_eq!(nm.token, "systems");
        // the family witness: the twin rows above the name-edge threshold
        // with the top row — the Northwind row (different name, no edge) is
        // NOT family
        assert!(nm.family.contains(&2));
        assert!(nm.family.contains(&1));
        assert!(!nm.family.contains(&7));
    }

    #[test]
    fn detect_stands_down_on_the_real_entity() {
        let results = vec![
            result(2, "Zephyron Cloud Solutions is headquartered in Seattle."),
            result(3, "Zephyron Cloud Solutions was founded in 2014."),
        ];
        // every query token is served — no distinguishing token, no near-miss
        assert!(
            detect(
                "Where is Zephyron Cloud Solutions headquartered?",
                &results,
                &StubEmbedder,
            )
            .is_none()
        );
    }

    #[test]
    fn detect_requires_a_real_collision_family() {
        // the distinguishing token is absent, but the served top row has NO
        // above-threshold neighbor — a lone hit cannot manufacture a near-miss
        let results = vec![
            result(3, "Zephyron Cloud Solutions was founded in 2014."),
            result(7, "Northwind Analytics is headquartered in Rotterdam."),
        ];
        assert!(
            detect(
                "What is the revenue of Zephyron Cloud Systems?",
                &results,
                &StubEmbedder,
            )
            .is_none()
        );
    }

    #[test]
    fn detect_stands_down_without_two_shared_tokens() {
        let results = vec![result(
            2,
            "Zephyron Cloud Solutions is headquartered in Seattle.",
        )];
        // the distinguishing token is absent, but the query shares only ONE
        // content token with the served top row — no confusable family was
        // served, and the family gate stands the contrast down
        assert!(
            detect(
                "Frobnitz Cloud Systems uptime record",
                &results,
                &StubEmbedder,
            )
            .is_none()
        );
    }
}
