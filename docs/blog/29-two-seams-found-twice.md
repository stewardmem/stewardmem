# Two seams, found twice

*2026-11-14. A paper released on November 11th carries our architecture's
principle in its title, with numbers behind it. We had shipped and written down
the same seams months earlier. Independent arrival is the strongest validation
a design gets — and the diff is where the work is.*

Here is the sentence that stopped the morning research watch: **"DISCERN:
Preserve at Write, Resolve at Read for Persistent LLM-Agent Memory"** (Amazon,
arXiv:2611.15729, released November 11, 2026). That is not an architecture
similar to the one this store was built on. It is that architecture, stated as
the design's single rule, defended with a benchmark, eight baselines, and a
four-judge council.

We found it on November 14th, in the routine research sweep, the way you find
anything: flagged, read, checked. There was no contact, no shared lineage, and
none is implied — the authors and we reached the same division of labor from
different starting problems. That is precisely why the finding is valuable.
When one team ships a design, it might be a taste. When two teams arrive
independently, and the second one brought experiments, the design has been
validated by the field.

The pattern itself is older here than the paper, and every date below is a
commit in this repository's history, not a claim. Tombstones landed **July 21**.
The per-tenant tamper-evident audit chain landed **July 28**. The bi-temporal
edges — `valid_at`/`invalid_at`, the Graphiti model — landed **July 30**.
Calibrated abstention and atomic supersession landed **August 1**. PII masking
in the gate landed **August 7**. The read seam covering every text field
landed **August 13**. The injection screen — the one that sees what the model
sees — landed **September 8**. The two-principal approval quorum landed
**September 11**. The cite-or-abstain verdict, stored per turn, landed
**October 28**. DISCERN was released **November 11**. Every mechanism in the
mapping table below predates the paper by weeks to months, with one exception
dated honestly below. What follows is the complete, accurate mapping between
the two systems — including the places they are ahead, because a mapping that
only flatters is not a mapping.

## The mapping

| DISCERN | StewardMem | In the tree since (commit) | Status |
|---|---|---|---|
| Preserve at write: admit and organize, never collapse | The write seam: the injection screen gating ingest, quarantined rows excluded from retrieval; provenance columns at the store | `screen.rs` · Sep 8, `da9e65a8` | Same principle; ours adds the security verdicts they do not carry |
| Immutable append-only event log as the sole source of truth | Tamper-evident per-tenant audit chain (`prev_hash` columns; the `brain_audit_chain_ok` gauge) | Jul 28, `42947fa0` | Equivalent |
| Bi-temporal facts: superseded values soft-invalidated, retained; as-of queries resolve to history | Bi-temporal edges (`valid_at`/`invalid_at`); atomic supersession (`resolve_supersession`, held evidence refuses destruction); as-of recall | Jul 30 / Aug 1, `bf854296`, `e1f6e486` | Equivalent — their `check_temporal` is our bi-temporal filter |
| Never merge confusable entities | No entity-merge function exists in the store | verified by absence | We hold the invariant trivially; they build the *family* surface around it |
| `SIMILAR_NAME` collision families + read-time contrast (`find_similar_entities`) | None | — | **Theirs-only** — a mechanism gap we should adopt |
| Four-way terminal contract (answer / update / clarify / abstain), decidable from the graph | Calibrated abstention + the cite-or-abstain verdict stored per turn; no `clarify`, no `update` action | Aug 1, `c189bb8e` · Oct 28, `75daf898` | **Theirs formalizes what we half-ship** — adoptable, and it is decidable without any LLM |
| Citation-or-abstain renderer: uncited answers bounce once, then abstain | The cite-or-abstain verdict, STORED per turn (`chat_turns.support_decision`); an unsupported claim is shown as unsupported, never silently kept as an answer | Oct 28, `75daf898` | Equivalent, independently stated |
| `EarnedAbstainGate`: abstention must be earned by demonstrated absence | The calibrated low-confidence abstention gate: an empty hit set with a `low_confidence`/`degraded` recall decision abstains deterministically — the kernel answers from fixed text and never asks the model for prose | Aug 1, `c189bb8e` | Theirs earns absence against a *collision family* — the stricter form; adoptable |
| Read-time navigation loop (LLM-driven; 7.6 s/query on hosted backbones) | Reference-faithful deterministic retrieval — the grounding query is built in code, "zero model tokens are spent… no LLM query rewriting" | 2026 (`handlers/chat.rs`) | Deliberate divergence, see below |

And the rows the paper cannot have, because they describe problems it never
frames: write-time injection screening with a quarantine that preserves
evidence; read-time content shaping and PII masking (`pii_mask` landed August
7; the read seam over every text field August 13; the recall-outcome-boundary
extraction landed November 14 — the one piece of this row that postdates the
paper, and it is an extension of the August seams, not a new one);
tamper-evident audit; two-principal approval quorums (`BRAIN_APPROVAL_QUORUM=2`
requires two distinct principals, recorded as hash-chained audit rows);
human-in-the-loop override; deletion with proof — tombstones carrying content
digests, purge behind a role gate, DSAR erase proposals. DISCERN's additive
graph never deletes anything, which is correct for resolution and silent on
erasure. Their "governance" gates the *honesty* of an answer; ours additionally
gates what may enter, what may leave, and who may decide. Neither subsumes the
other. That is the diff.

## Where they are ahead, stated plainly

Three mechanisms in the paper formalize what we only half-built, and honesty
requires naming them as the paper's contribution:

**The terminal contract.** Their committed action is a decidable function of
store state: `clarify` when two or more candidate entities survive the
evidence-gathering, `abstain` on demonstrated absence, `update` when the
supersession chain is long, `answer` otherwise — with every gate enforced by
graph predicates rather than model judgment. Our abstention is real and
calibrated — an empty hit set with a low-confidence or degraded recall decision
abstains deterministically, from fixed kernel text, and the tiered cascade
abstains when zero sentences verify against evidence — but it is a *one-way*
gate: there is no `clarify`, no `update` action, no earned-answer check, and
no near-miss guard, because there is no collision family for them to be earned
against. The paper's ablation puts numbers on the gap: removing the
collision check collapses abstention calibration (over-commitment 0.21 → 0.28),
and a commercial memory system built without this discipline refused 87% of
its answerable questions. Calibration, not volume, is the skill.

**The entity-collision surface.** Confusable entities linked into families,
never merged, contrasted at read time — with the contamination failure class
they measure it against (answering about the target without leaking a
confusable twin's fact). We hold the never-merge invariant but offer no
disambiguation surface; a near-match query retrieves whichever chunk ranks
highest.

**The abstention evaluation axis.** They score the answer-versus-abstain
decision as a classification problem — precision, recall, F1, false-decline —
against attribute-swap and near-miss traps, so that reflexive declining cannot
score well. Our probe battery measures recall, citation, and honest refusal
anecdotally; it has no calibrated abstention axis. Their trap shape is
directly reusable against our store.

One more number deserves its own sentence: their read-time loop costs **7.6
seconds per query** and degrades from 0.887 to 0.610 as the backbone weakens.
That is a hosted-model posture. Our retrieval is deterministic and
LLM-free — not because we could not build the loop, but because the on-device
tiers cannot afford it, and the paper's own backbone table is the evidence.
The parts of DISCERN that transfer to this architecture are exactly the parts
that are graph-decidable: the gates, the contract, the collision families.
The part that does not transfer is the thing their accuracy is built on.

## What this means

The architecture holds. The two-seam division of labor — preserve and never
collapse at write, resolve and commit at read — now has independent
confirmation from a team that tested it against eight baselines on two
corpora. The store's mechanisms under the paper's names — bi-temporal recall
(July 30), supersession (August 1), calibrated abstention (August 1), the
citation verdict (October 28) — are this paper's mechanisms with different
names and earlier commit dates, and the store's security seams remain the
parts neither they nor the rest of the memory literature frame.

**Provenance.** Every date above is a first-appearance commit, verifiable with
`git log --reverse --format="%ad %h %s" --date=short -S "PATTERN" -- src` on
the repository: `prev_hash` (Jul 28, `42947fa0`), `valid_at` (Jul 30,
`bf854296`), `resolve_supersession` (Aug 1, `e1f6e486`), `abstain` (Aug 1,
`c189bb8e`), `redacted:email` (Aug 7, `306eb16c`), `sanitize_read` (Aug 13,
`c469671a`), `typoglycemia` (Sep 8, `da9e65a8`), `BRAIN_APPROVAL_QUORUM` (Sep
11, `a5d411ec`), `cite-or-abstain` (Oct 28, `75daf898`),
`sanitize_search_result` (Nov 14, `b637a0f1`), and the tombstone schema in the
initial public tree (Jul 21, `7aace8f8`). The code anchors are named where
each row needs them: `src/gate.rs` (`sanitize_read`,
`sanitize_search_result`), `src/pii_mask.rs`, `src/screen.rs`, `src/temporal.rs`,
`src/consolidate.rs` (`resolve_supersession`),
`src/service/review.rs` (the quorum), `src/handlers/chat.rs` (the abstain
gate and the grounding query), `src/storage_layout.rs` (the verdict columns).

The work this sets up is equally concrete: their trap taxonomy — attribute
swaps, near-miss entities, and the case the paper's own benchmark cannot test,
two independently-written contradictory facts with no explicit revise — turned
into a probe against *our* store, scored as classification, run before any
mechanism is adopted. Four mechanisms are candidates: the collision families,
the terminal contract, the earned-abstention gate, the evaluation axis. The
paper earns a mechanism review, not a fork, and the probe decides which of the
four are real gaps in this tree rather than gaps in a different one.

Full analysis, with the equations and the honest structure of their results
table — including the part where the simple baseline beats them on three of
four pillars — is in the research note on
[DISCERN and the two seams](./../research/25-discern-two-seams.md). The
write-seam reasoning it independently confirms is in
[the injection screen post](./27-catch-it-on-the-way-in.md); the erasure story
its additive graph does not attempt is in
[delete is a verb](./26-delete-is-a-verb-not-a-promise.md).
