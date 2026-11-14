# DISCERN and the two seams: what the field's independent arrival at our architecture gets right, what it lacks, and what we should adopt

*2026-11-14 · sources: DISCERN (arXiv:2611.15729v1, Amazon, TTCL Workshop 2026, full text), Zep/Graphiti (arXiv:2501.13956), HippoRAG2 (arXiv:2502.14802), LongMemEval (arXiv:2410.10813) — abstracts verified; the paper read in full.*

## The problem the paper solves

Enterprise memory accumulates facts that **contradict** (revenue restated across
sessions), **supersede** (a deal stage that changes), and **collide** (Zephyron
Cloud *Solutions* at $5.1B vs Zephyron Cloud *Systems* at $18M — two real
distinct entities that share every name token). DISCERN's measured claim is that
the field has been solving the wrong problem: on their ENT-310 benchmark,
**retention and currency are near-saturated (0.93–0.98 for every competent
system) — recall is essentially solved. What remains unsolved is resolution**:
which entity a query names, which time-state is current, and whether to commit
an answer at all. Two failure classes sit in the gap between "the right evidence
was retrieved" and "the committed answer was right": one-shot disambiguation (a
single post-retrieval enrichment call, no evidence loop), and the absence of a
**terminal-state contract** — the system always answers, so blended, stale, and
hallucinated answers are invisible to its metrics.

This is the sharpest statement yet of the two-seam architecture brain-server was
built on — preserve at write, resolve at read — and it arrives from Amazon with
numbers behind it. That makes it worth a full comparison rather than a citation.

## The reference implementation

**Write time — preserve and organize, never collapse.** An immutable,
append-only per-actor event log is the sole source of truth; over it, derived
views are materialized as *reversible projections* (rebuildable from the log,
"agnostic to the physical backend — one SQLite file per actor"). Two safety
rules do the real work:

1. **Confusable entities stay co-equal, linked, never merged.** A
   `SIMILAR_NAME` edge (drawn at `0.6·jaccard_lex(names) + 0.4·cos(embeddings)`
   above a threshold) joins the twins into a family the reader can contrast —
   the $18M is never written onto the $5.1B entity. Their stated failure mode
   is *extra nodes* (drift), which is the safe direction: an extra sibling the
   reader can rule out, versus write-time merging, which "would fold Systems
   into Solutions, misattribute the $18M, and destroy the entity irreversibly."
2. **Changed values become bi-temporal edges** — stamped with both the
   event-log time and a valid-from/valid-to interval; superseded values are
   soft-invalidated (`expired_at` set) and *retained*, so "what was the revenue
   before the acquisition?" stays answerable.

**Read time — navigate and commit, per query.** Resolution is an agentic loop
descending a memory hierarchy (profile → entity/session → fact → transcript)
through a deterministic, read-only, zero-LLM tool surface:
`read_profile_summary`, `find_similar_entities` (the collision family),
`check_temporal` (walk the bi-temporal chain), `search_facts` /
`enumerate_facts`, `search_memory` (dense+BM25), `search_conversation` /
`get_session`. Depth adapts to what earlier calls returned.

**Commitment is graph-decidable, not a judgment over prose** — this is the
paper's most important move. The committed action is a decidable function of
store state (their Eq. 1), with `C` the candidate-entity set surviving evidence
gathering, `active(e,attr)` the non-superseded values (at most one by the
additive invariant), and `ℓ(e,attr)` the supersession-chain length:

```
clarify  | |C| ≥ 2
abstain  | |C| = 0, or |C| = 1 ∧ active(e,attr) = ∅
update   | |C| = 1, active(e,attr) ≠ ∅, ℓ(e,attr) ≥ 2
answer   | |C| = 1, active(e,attr) ≠ ∅, ℓ(e,attr) = 1
```

The governance gates are **LLM-free** because the store is a labeled graph: an
`EarnedAnswerGate` forces the ambiguity check for underspecified queries before
any commit (it forces the *check*, never the decision); an `EarnedAbstainGate`
rejects abstention without demonstrated absence; a `DeclineRouter` makes the
decline type deterministic (clarify when confusable candidates survive, abstain
when none); a near-miss guard routes an answer whose exact entity is absent but
has a confusable neighbor to clarify; and a **citation-or-abstain renderer**
bounces an uncited answer once for grounding before the citation floor abstains.

**Evaluation.** ENT-310: 10 actors, 50 companies, 31 sessions, 310 queries —
210 answerable across four pillars (retention n=84, currency n=40,
**contamination n=71** — answer about the target without leaking a confusable
entity's fact, **entity resolution n=15**) and 100 unanswerable of two trap
types (**attribute-swap**: a real entity asked for an attribute it never had;
**near-miss entity**: a confusable entity never actually discussed). Judged by a
four-model council (Sonnet 4.6, Opus 4.8, Qwen3-235B, GPT-oss-120B; Cohen's κ
0.61–0.80), with answer-versus-abstain scored as a *classification* problem
(precision/recall/F1/false-decline) — so reflexive declining cannot win.

**Results, read honestly.** DISCERN leads macro correctness (0.861 vs 0.806 for
the strongest baseline) and abstention (F1 0.924 vs 0.802), with entity
resolution 0.800 where no baseline exceeds 0.40. But the honest structure of
their own table matters: the simple extract-retrieve baseline **beats DISCERN on
three of the four pillars** (retention 0.976 vs 0.929, currency 0.975 vs 0.900,
contamination 0.873 vs 0.817) — the macro win is carried entirely by
entity-resolution on n=15, which the authors themselves flag as directional.
The ablation that does isolate the mechanism: navigation lifts accuracy from a
0.33 tools-off floor to 0.89, and removing `find_similar_entities` collapses
abstention calibration (over-commit 0.21→0.28) — the earned-answer check is the
load-bearing gate. Backbone sensitivity is steep (Sonnet 0.887 → Qwen3-235B
0.707 → GPT-oss 0.610): the navigation loop needs hosted-model tool-use. And
read latency is **7.6 s mean, 4.1 turns and 5.7 tool calls per query**.

## The deterministic way brain-server implements it — the diff, both directions

**Already implemented (the paper validates existing choices):**

| DISCERN mechanism | brain-server's standing shape |
|---|---|
| Immutable append-only event log, sole source of truth | the append-only audit chain + the store-verbatim law (store verbatim, shape on the way out) |
| Soft-invalidated superseded values, retained | UMP supersession: a revise expires the old revision and *retains* it; default recall serves the survivor |
| As-of queries resolve to history | the bi-temporal recall filter (`valid_at`) — `check_temporal`'s capability exists as a recall filter |
| Citation-or-abstain renderer | the CG-11 citation law: an answer cites its memory ids or does not ship |
| Never-merge entities | trivially true — nothing in the store merges entities; the paper's contribution is the *family* + *contrast* surface, not the never-merge rule |
| Abstain on absence | the kernel's zero-hit abstain gate — a coarse `EarnedAbstainGate` keyed on retrieval emptiness rather than demonstrated absence |

**The gaps DISCERN names (adoptable, in priority order):**

1. **The entity-collision disambiguation surface.** We have never-merge but no
   *family*: no `SIMILAR_NAME`-style collision edges, no read-time contrast
   tool, no near-miss guard. A query that names a near-match today retrieves
   whichever chunk ranks highest — the exact contamination failure their
   contamination pillar (n=71) measures, and our ingest path's entity names are
   client-declared with no collision surface at all.
2. **The four-way terminal contract.** Our abstain gate is binary and
   retrieval-keyed; there is no `clarify` (surviving ambiguity → ask), no
   `update` action, and no earned-*answer* gate forcing an ambiguity check
   before commit. The paper's point is that these are **decidable functions of
   the store** — implementable without any LLM judgment, which is precisely the
   on-device-friendly shape.
3. **Demonstrated-absence abstention.** Zero-hit abstention fires on retrieval
   emptiness; DISCERN requires the absence to be *demonstrated* (searched for).
   Their ablation quantifies the failure mode of the coarse version from the
   other side (over-commit when the collision check is absent), and Mem0's
   collapse (false-decline 0.867 — refusing 87% of answerable queries) shows
   the reflexive-decline version failing from the opposite direction.
4. **The terminal-decision evaluation axis.** Nothing in our probe battery
   scores answer-vs-abstain as classification. The ENT-310 trap shape
   (attribute-swap + near-miss, gold answers derived from the seeded state) is
   directly adoptable as a probe against our store — the same shape
   LongMemEval (ICLR 2025) pioneered as knowledge-update and abstention
   abilities, which DISCERN then industrialized.

**What brain-server has that DISCERN does not (the direction their paper
cannot see):** every *security* seam. Their write path screens nothing — no
injection screen, no quarantine verdicts; their read path shapes nothing — no
sanitization, no PII masking at the boundary, no hostile-element/URL drops.
Their "governance" is answer-honesty gating; the compliance stack (tamper-evident
audit, two-principal quorum, human-in-the-loop approvals, the read seam's
masking law) is absent from their frame entirely. And their resolution posture
is *hosted-model-shaped*: 7.6 s per query of LLM navigation that degrades to
0.610–0.707 on weaker backbones. Brain-server's on-device lanes cannot buy that
loop on local tiers — the deterministic, graph-decidable form of their gates is
the part that transfers; the LLM navigation loop is the part that does not.

## The ceiling

ENT-310 is synthetic, generated by one model family that also sits on the judge
council and among the backbones (disclosed, mitigated by the per-judge
breakdown — the lead holds under both non-Anthropic judges — but still
self-referential). The macro-correctness win rests on a 15-question pillar; the
simple lossless reader wins the three saturated pillars. The 7.6 s read latency
is a hosted-posture number, not an on-device one. And the paper's own framing —
"the decision rule given that graph rather than a claim of ground-truth
correctness" — concedes that everything downstream inherits the LLM
extraction's errors.

**The adoptable next step (preregistered):** an ENT-310-shaped probe against
*our* store — attribute-swap traps, near-miss entity traps, and a
contradiction-at-recall pair (two independently-written conflicting facts, no
explicit revise) — scored answer-vs-abstain as classification. It measures
whether the gaps named above are real in our tree before any mechanism is
adopted, and it merges naturally with the real-store round already
preregistered. The paper earns a mechanism review, not a fork: the never-merge
family, the earned gates, and the terminal contract are the parts worth
building; the navigation loop is the part our on-device posture replaces with
deterministic recall plus the gates.
