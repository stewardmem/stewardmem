//! The support verdict — the chat spine's cite-or-abstain gate (LL-04
//! §1.2, tightened by 22A: the cite-TRUTH verdict).
//!
//! THE LAW THIS MODULE EXISTS TO HOLD: "Never fabricate a citation" stopped
//! being an unverified instruction and became a WIRE FACT (LL-04), and 22A
//! carried it one step further — `supported` means VERIFIED-QUOTED. The
//! model's typed final (`BridgeEvent::Final{abstain, claims}`) names the
//! grounding hits it drew from; this module judges those names with PURE
//! INDEX ARITHMETIC plus a pure TEXT MATCH — no LLM in the gate, no
//! threshold, no model call. A claim citing an in-range hit whose
//! normalized text the hit does not carry is disclosed `paraphrase`; an
//! answer with claims but zero quoted claims is `partial` ("cited, not
//! verbatim"), never `supported`. The kernel labels what the model
//! asserted; it never edits the model's prose.
//!
//! Rules (pinned, each one a test below):
//! - every `cites[i]` must index a hit the kernel ACTUALLY sent (the packed
//!   set — `evidence.len()` IS the universe); any out-of-range index flags
//!   that claim `unknown_refs`;
//! - `Abstained`  = `abstain ∧ claims empty` — the honest "I don't know";
//! - `Unsupported` = `unknown_refs > 0 ∨ (cited = 0 ∧ claims non-empty)` —
//!   the fabricated-citation label;
//! - `Partial`    = `uncited > 0 ∨ (quoted = 0 ∧ claims non-empty)` — some
//!   claims prove nothing, or nothing was quoted verbatim (22A);
//! - `Degraded`   = grounding was degraded (the recall error arm — the wire
//!   already said so; this carries it onto the turn of record).
//!
//! Precedence when several facts hold: Abstained, Unsupported, Partial,
//! Degraded, Supported. A fabrication label outranks a degraded-evidence
//! label — the stronger fact is the one a reader must not glide past; and
//! a fully-cited answer on degraded grounding is still `Degraded`, because
//! the turn's evidence never became sound.
//!
//! The verdict rides the wire as the `support` SSE event (after `final`,
//! exactly once) and persists on the `chat_turns` row (`support_decision`);
//! `Unsupported` turns are STORED FLAGGED — visible, never silently an
//! answer.

use crate::agentloop::bridge_afm::BridgeClaim;

/// The verdict's wire vocabulary — snake_case, like the grounding
/// decisions beside it. Stored verbatim in `chat_turns.support_decision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportDecision {
    Supported,
    Partial,
    Unsupported,
    Abstained,
    Degraded,
}

impl SupportDecision {
    /// The wire/storage name.
    pub fn as_str(&self) -> &'static str {
        match self {
            SupportDecision::Supported => "supported",
            SupportDecision::Partial => "partial",
            SupportDecision::Unsupported => "unsupported",
            SupportDecision::Abstained => "abstained",
            SupportDecision::Degraded => "degraded",
        }
    }

    /// Parse a stored/wire name back. Unknown values are storage damage and
    /// refuse rather than inventing a sixth decision.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "supported" => Some(SupportDecision::Supported),
            "partial" => Some(SupportDecision::Partial),
            "unsupported" => Some(SupportDecision::Unsupported),
            "abstained" => Some(SupportDecision::Abstained),
            "degraded" => Some(SupportDecision::Degraded),
            _ => None,
        }
    }
}

/// The support verdict over one turn's claims. Counts are CLAIM counts:
/// `cited` claims index at least one hit the kernel sent, `uncited` claims
/// index none, `unknown_refs` claims named an index outside the packed
/// set; of the cited claims, `quoted` are carried by the hit's own text
/// and `paraphrase` are the answerer's own words (22A — the disclosure
/// the counts ride the wire additively beside the decision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    pub cited: u16,
    pub uncited: u16,
    pub unknown_refs: u16,
    pub quoted: u16,
    pub paraphrase: u16,
    pub decision: SupportDecision,
}

/// The span law's n-gram window (S0's preregistered metric device, now the
/// kernel's citation law) — MOVED VERBATIM from cascade.rs (22A): one span
/// law, two callers.
const SPAN_NGRAM: usize = 6;

/// THE ONE NORMALIZER (22A): lowercase; `[n]` cite markers stripped first
/// (the answerer's prose carries them, evidence text never does — for
/// evidence the strip is a no-op, and a literal bracketed number in memory
/// content strips symmetrically on both sides, benign for span purposes);
/// every non-alphanumeric char becomes a space; whitespace collapsed.
/// Byte-identical to cascade's historical internal form for
/// cite-marker-free input — the judge's byte-identity law — and the ONLY
/// normalizer in the tree: cascade delegates to it.
pub fn normalize_claim_span(text: &str) -> String {
    let stripped = strip_cite_markers(text);
    let mut out = String::with_capacity(stripped.len());
    for c in stripped.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove `[123]`-style cite markers (one or more ASCII digits inside
/// square brackets). A `[` not followed by digits-then-`]` stays — the
/// scan only skips a well-formed marker, so ordinary prose is untouched.
fn strip_cite_markers(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b']' {
                i = j + 1;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("i sits on a char boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// THE SPAN LAW (S0's preregistered 6-gram), moved VERBATIM from
/// cascade.rs: the claim and the evidence share a contiguous run of
/// `SPAN_NGRAM` consecutive normalized words; an evidence text shorter
/// than the n-gram degrades to whole-evidence containment (the
/// preregistered degenerate case). Byte-identical semantics — a cascade
/// verdict change is a wrong answer (delegation must be a no-op).
pub fn spans_verified(claim_norm: &str, evidence_norm: &str) -> bool {
    let cw: Vec<&str> = claim_norm.split(' ').collect();
    let ew: Vec<&str> = evidence_norm.split(' ').collect();
    if ew.len() < SPAN_NGRAM {
        return !evidence_norm.is_empty() && claim_norm.contains(evidence_norm);
    }
    // EVERY 6-gram window of the evidence is a span to find in the claim's
    // own 6-gram windows — both sides slide (the S0 harness's law verbatim).
    ew.windows(SPAN_NGRAM).any(|g| {
        let g = g.join(" ");
        cw.windows(SPAN_NGRAM).any(|w| w.join(" ") == g)
    })
}

/// THE MATCHER (22A): is the claim QUOTED by any in-range cited evidence?
/// Two sufficient shapes, both pure text — the verbatim quote (the claim's
/// normalized words, in sequence, inside the hit) or the shared six-word
/// span (cascade's own law). A judge-derived cite satisfies the span arm
/// by construction, which is why kernel-derived claims are quoted exactly
/// when cited: the delegation no-op, pinned in cascade's suite.
pub fn claim_is_quoted(text: &str, cites: &[i64], evidence: &[&str]) -> bool {
    let key = normalize_claim_span(text);
    if key.is_empty() {
        return false;
    }
    cites.iter().any(|&i| {
        usize::try_from(i)
            .ok()
            .filter(|&u| u < evidence.len())
            .is_some_and(|u| {
                let ev = normalize_claim_span(evidence[u]);
                ev.contains(&key) || spans_verified(&key, &ev)
            })
    })
}

/// The verdict, pure and total: any (abstain, claims, evidence, grounding
/// posture) maps to exactly one label. Totality is the gate's point — there
/// is no input shape that escapes labeling. The evidence slice IS the cite
/// universe: `evidence.len()` bounds every index check, and its texts feed
/// the quote matcher — one slice, one truth.
pub fn support_verdict(
    abstain: bool,
    claims: &[BridgeClaim],
    evidence: &[&str],
    degraded_grounding: bool,
) -> Support {
    let mut cited = 0;
    let mut uncited = 0;
    let mut unknown_refs = 0;
    let mut quoted = 0;
    let mut paraphrase = 0;
    for claim in claims {
        let mut known = false;
        let mut named_unknown = false;
        for &index in &claim.cites {
            // Negative indices cannot name a hit either: usize cast folds
            // them far out of range, which is exactly how they must count.
            if (index as usize) < evidence.len() {
                known = true;
            } else {
                named_unknown = true;
            }
        }
        if named_unknown {
            unknown_refs += 1;
        }
        if known {
            cited += 1;
            if claim_is_quoted(&claim.text, &claim.cites, evidence) {
                quoted += 1;
            } else {
                paraphrase += 1;
            }
        } else {
            uncited += 1;
        }
    }
    let decision = if abstain && claims.is_empty() {
        SupportDecision::Abstained
    } else if unknown_refs > 0 || (cited == 0 && !claims.is_empty()) {
        SupportDecision::Unsupported
    } else if uncited > 0 || (quoted == 0 && !claims.is_empty()) {
        SupportDecision::Partial
    } else if degraded_grounding {
        SupportDecision::Degraded
    } else {
        SupportDecision::Supported
    };
    Support {
        cited,
        uncited,
        unknown_refs,
        quoted,
        paraphrase,
        decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(text: &str, cites: &[i64]) -> BridgeClaim {
        BridgeClaim {
            text: text.to_owned(),
            cites: cites.to_vec(),
        }
    }

    /// THE FLAGSHIP RED-PROOF (LL-04 §5), FLIPPED FORM. The first form —
    /// `unknown_citation_index_passes_through_unflagged` — ran GREEN at
    /// e1a77296 before any LL-04 edit: a final whose only claim cited index
    /// 7 of the 5 hits the kernel sent left the wire with no claims, no
    /// abstain, no verdict. THE DEFECT. This is the assertion that replaces
    /// it: the same shape now flags the turn.
    #[test]
    fn unknown_citation_index_flags_the_turn() {
        let support = support_verdict(
            false,
            &[claim("Green tea contains EGCG [8].", &[7])],
            &["hit"; 5],
            false,
        );
        assert_eq!(support.decision, SupportDecision::Unsupported);
        assert_eq!(support.unknown_refs, 1, "the out-of-range claim is counted");
        assert_eq!(
            support.uncited, 1,
            "a claim naming only unknowns is uncited"
        );
        assert_eq!(support.cited, 0);
        assert_eq!(support.quoted, 0, "an out-of-range cite is never a quote");
    }

    /// THE §1.2 RULE SET, each rule its own arm. Totality is the gate's
    /// point: every shape lands on exactly one label.
    #[test]
    fn the_support_verdict_rules_hold() {
        // Abstained: abstain ∧ claims empty — on sound AND on degraded
        // grounding (the kernel's deterministic abstain emits exactly this shape).
        let a = support_verdict(true, &[], &[], false);
        assert_eq!(a.decision, SupportDecision::Abstained);
        assert_eq!(a.cited + a.uncited + a.unknown_refs, 0);
        let a_degraded = support_verdict(true, &[], &[], true);
        assert_eq!(a_degraded.decision, SupportDecision::Abstained);

        // Unsupported: cited == 0 with claims non-empty — a claim wall with
        // nothing behind it, even when every index is in range.
        let u = support_verdict(
            false,
            &[claim("x", &[]), claim("y", &[])],
            &["hit"; 5],
            false,
        );
        assert_eq!(u.decision, SupportDecision::Unsupported);
        assert_eq!(u.uncited, 2);

        // Unsupported outranks Degraded: a fabrication on degraded
        // grounding is labeled by the STRONGER fact.
        let ud = support_verdict(false, &[claim("x", &[9])], &["hit"; 2], true);
        assert_eq!(ud.decision, SupportDecision::Unsupported);

        // Partial: uncited > 0 while at least one claim holds (and the
        // held claim is genuinely quoted — the cited/uncited axis alone).
        let p = support_verdict(
            false,
            &[claim("grounded", &[0]), claim("floating", &[])],
            &["grounded", "hit two", "hit three"],
            false,
        );
        assert_eq!(p.decision, SupportDecision::Partial);
        assert_eq!(p.cited, 1);
        assert_eq!(p.uncited, 1);
        assert_eq!(p.quoted, 1);
        assert_eq!(p.paraphrase, 0);

        // Degraded: grounding was degraded and nothing stronger applies —
        // even a fully-cited answer stays Degraded (the evidence never
        // became sound).
        let d = support_verdict(false, &[], &[], true);
        assert_eq!(d.decision, SupportDecision::Degraded);
        let d_cited = support_verdict(false, &[claim("g", &[0])], &["g"], true);
        assert_eq!(d_cited.decision, SupportDecision::Degraded);

        // Supported: claims non-empty, every claim in range (and quoted —
        // under the 22A law an all-paraphrase wall is Partial, so the
        // evidence here carries each claim's words verbatim).
        let s = support_verdict(
            false,
            &[claim("a", &[0]), claim("b", &[2, 4])],
            &["a", "x", "b", "x", "x"],
            false,
        );
        assert_eq!(s.decision, SupportDecision::Supported);
        assert_eq!(s.cited, 2);
        assert_eq!(s.quoted, 2);

        // A claim citing NOTHING indexes nothing: uncited, never a crash —
        // and an empty claims list with no abstain is a Degraded-shaped
        // turn only when grounding was degraded; on sound grounding it is
        // the Unsupported wall (cited == 0 with claims non-empty is the
        // rule; zero CLAIMS is labeled by the grounding posture).
        let zero = support_verdict(false, &[], &["hit"; 5], false);
        assert_eq!(zero.decision, SupportDecision::Supported);
    }

    /// THE WIRE/STORAGE VOCABULARY round-trips, and an unknown value is
    /// storage damage, not a sixth decision.
    #[test]
    fn the_decision_names_round_trip_and_refuse_unknowns() {
        for decision in [
            SupportDecision::Supported,
            SupportDecision::Partial,
            SupportDecision::Unsupported,
            SupportDecision::Abstained,
            SupportDecision::Degraded,
        ] {
            assert_eq!(
                SupportDecision::parse(decision.as_str()),
                Some(decision),
                "{} round-trips",
                decision.as_str()
            );
        }
        assert_eq!(SupportDecision::parse("hallucinated"), None);
        assert_eq!(SupportDecision::parse(""), None);
        assert_eq!(
            SupportDecision::parse("Supported"),
            None,
            "the vocabulary is lowercase snake_case"
        );
    }

    /// S0e (22A) — ONE MATCHER, TWO CALLERS (the `cors_layer_from_config`
    /// precedent). cascade's `judge_answer` must DELEGATE its span law to
    /// this module — the ONE normalizer and the ONE span predicate — and
    /// must host no second matcher of its own. Two span laws drift, and
    /// the drift is silent: the verdict would say `quoted` while the judge
    /// said `cited` over differently-normalized text. Red on the pre-round
    /// tree (cascade.rs carried its own `normalize` + `spans_verified`);
    /// a planted second matcher in cascade re-reds it.
    #[test]
    fn cascade_and_support_share_one_matcher() {
        let cascade_src =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agentloop/cascade.rs");
        let src = std::fs::read_to_string(&cascade_src)
            .expect("cascade.rs must be readable from the crate root");
        assert!(
            src.contains("chat_support::normalize_claim_span")
                && src.contains("chat_support::spans_verified"),
            "cascade.rs must delegate the matcher to service::chat_support \
             (the ONE normalizer and the ONE span law) — found no delegation"
        );
        assert!(
            !src.contains("fn normalize(") && !src.contains("fn spans_verified("),
            "cascade.rs hosts a SECOND matcher — the span law lives in \
             service::chat_support and cascade delegates (one matcher, \
             two callers)"
        );
    }

    /// S0a (22A) — THE ROUND'S LAW: a citation that names an in-range hit
    /// is TOLD whether the hit's text actually contains the claim. One
    /// claim, cited in-range, whose words the evidence does NOT carry →
    /// Partial, never Supported: `supported` is earned by the quote.
    #[test]
    fn paraphrase_only_answer_is_partial_not_supported() {
        let support = support_verdict(
            false,
            &[claim("Green tea is processed by steaming.", &[0])],
            &["green tea leaves are steamed"],
            false,
        );
        assert_eq!(support.decision, SupportDecision::Partial);
        assert_eq!(support.cited, 1, "the claim names an in-range hit");
        assert_eq!(
            support.quoted, 0,
            "the evidence does not carry the claim's words"
        );
        assert_eq!(support.paraphrase, 1, "cited, not verbatim — disclosed");
        assert_eq!(support.uncited, 0);
        assert_eq!(support.unknown_refs, 0);
    }

    /// S0b (22A) — one verbatim quote UPLIFTS the answer: a mix of quoted
    /// and paraphrased claims with zero uncited is Supported (quoted >= 1),
    /// with both counts disclosed on the verdict.
    #[test]
    fn quoted_and_paraphrase_mix_with_no_uncited_is_supported() {
        let support = support_verdict(
            false,
            &[
                claim("Green tea leaves are steamed [0].", &[0]),
                claim("Matcha is whisked into a bright green drink [1].", &[1]),
            ],
            &[
                "green tea leaves are steamed after picking",
                "matcha is shaded before harvest",
            ],
            false,
        );
        assert_eq!(support.decision, SupportDecision::Supported);
        assert_eq!(support.quoted, 1, "the first claim is carried verbatim");
        assert_eq!(support.paraphrase, 1, "the second is the model's own words");
        assert_eq!(support.cited, 2);
        assert_eq!(support.uncited, 0);
    }

    /// S0c (22A) — span judging ignores out-of-range cites: a cite past
    /// the evidence's length is an unknown ref even when the claim's text
    /// is verbatim evidence — the quote cannot rescue a fabricated index.
    #[test]
    fn span_judging_ignores_out_of_range_cites() {
        let support = support_verdict(
            false,
            &[claim("Green tea leaves are steamed after picking.", &[7])],
            &[
                "green tea leaves are steamed after picking",
                "matcha is shaded before harvest",
            ],
            false,
        );
        assert_eq!(support.decision, SupportDecision::Unsupported);
        assert_eq!(support.unknown_refs, 1);
        assert_eq!(support.quoted, 0, "an out-of-range cite is never a quote");
        assert_eq!(support.cited, 0);
    }

    /// S0d (22A) — THE REGRESSION LAW: the zero-claim case (no claims, no
    /// abstain, sound grounding) stays Supported. The Partial disjunct
    /// requires claims non-empty — an answer with nothing to judge is not
    /// "cited, not verbatim"; it is the shape the grounding posture
    /// labels.
    #[test]
    fn the_zero_claim_case_stays_supported() {
        let support = support_verdict(false, &[], &["evidence that exists"], false);
        assert_eq!(support.decision, SupportDecision::Supported);
        assert_eq!(support.quoted, 0);
        assert_eq!(support.paraphrase, 0);
    }

    /// S2's NO-OP THEOREM (22A): judge_answer derives cites BY the shared
    /// span law, and claim_is_quoted's span arm IS that law — so every
    /// kernel-derived claim is quoted exactly when cited, and the cascade
    /// arms' support verdicts are unchanged by the cite-truth law. The
    /// prose here is the adversarial shape: a sentence that shares the
    /// six-word span but is NOT contained in the evidence (the containment
    /// arm alone would call it paraphrase — this pin reds if the span arm
    /// is ever removed from the matcher).
    #[test]
    fn judge_derived_claims_are_quoted_exactly_when_cited() {
        let evidence = vec![
            crate::agentloop::cascade::EvidenceChunk {
                hit_index: 0,
                chunk_id: 1,
                score: 1.0,
                title: None,
                content: "Green tea is made from Camellia sinensis leaves that are steamed soon after picking, which preserves catechins like EGCG.".to_owned(),
                provenance: None,
            },
            crate::agentloop::cascade::EvidenceChunk {
                hit_index: 1,
                chunk_id: 2,
                score: 0.9,
                title: None,
                content: "Its benefits include antioxidant support, improved metabolic markers, and a milder caffeine lift than coffee.".to_owned(),
                provenance: None,
            },
        ];
        // The first sentence extends the evidence in both directions —
        // span-covered, never contained; the second is pure synthesis and
        // the judge drops it (no cites, not a claim).
        let prose = "Indeed, green tea is made from Camellia sinensis leaves that are steamed soon after picking, and this preserves the good compounds. The writer's own synthesis names no evidence at all here.";
        let judged = crate::agentloop::cascade::judge_answer(prose, &evidence);
        assert_eq!(
            judged.claims.len(),
            1,
            "only the span-covered sentence becomes a claim"
        );
        let evidence_texts: Vec<&str> = evidence.iter().map(|c| c.content.as_str()).collect();
        let claims: Vec<BridgeClaim> = judged
            .claims
            .iter()
            .map(|c| BridgeClaim {
                text: c.text.clone(),
                cites: c.cites.iter().map(|&i| i as i64).collect(),
            })
            .collect();
        let support = support_verdict(judged.abstained, &claims, &evidence_texts, false);
        assert_eq!(
            support.quoted, support.cited,
            "judge-derived cites satisfy the span arm by construction — the delegation no-op"
        );
        assert_eq!(support.paraphrase, 0);
        assert_eq!(
            support.decision,
            SupportDecision::Supported,
            "the cascade arm's verdict is unchanged by the cite-truth law"
        );
    }
}
