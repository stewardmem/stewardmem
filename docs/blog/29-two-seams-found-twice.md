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

The pattern itself is older here than the paper. The governance thesis —
provenance and control fixed at ingestion — was written down on **September
11**. The write seam and read seam as separate, load-bearing structures —
screen at write because the payload becomes a resident, shape on read because
stored-verbatim and emitted-safe are different jobs — were written down on
**October 4**, along with the four-layer deletion path. The bi-temporal recall
mechanism behind both is the store's oldest documented mechanism. The paper was
released five weeks after that. What follows is the complete, accurate mapping
between the two systems — including the places they are ahead, because a
mapping that only flatters is not a mapping.

## The mapping

| DISCERN | StewardMem | Since | Status |
|---|---|---|---|
| Preserve at write: admit and organize, never collapse | The write seam: injection screen with reject / quarantine / clean verdicts; governance and provenance fixed at ingestion | Sep 11 / Oct 4 | Same principle; ours adds the security verdicts they do not carry |
| Immutable append-only event log as the sole source of truth | Tamper-evident hash chain over the store; store verbatim, shape on the way out | 2026 | Equivalent |
| Bi-temporal facts: superseded values soft-invalidated, retained; as-of queries resolve to history | Supersession on revise — old revision expired and *retained*; bi-temporal recall filter (`?at=`) | The store's oldest mechanism | Equivalent — their `check_temporal` is our `?at=` |
| Never merge confusable entities | Nothing in the store merges entities | — | We hold the invariant trivially; they build the *family* surface around it |
| `SIMILAR_NAME` collision families + read-time contrast (`find_similar_entities`) | None | — | **Theirs-only** — a mechanism gap we should adopt |
| Four-way terminal contract (answer / update / clarify / abstain), decidable from the graph | Binary zero-hit abstain gate; the citation law governs commit | — | **Theirs formalizes what we half-ship** — adoptable, and it is decidable without any LLM |
| Citation-or-abstain renderer: uncited answers bounce once, then abstain | The citation law: an answer cites its memory ids or does not ship | The citation law | Equivalent, independently stated |
| `EarnedAbstainGate`: abstention must be earned by demonstrated absence | The kernel's zero-hit abstain gate (keyed on retrieval emptiness) | — | Theirs is the stricter, better-calibrated form; adoptable |
| Read-time navigation loop (LLM-driven; 7.6 s/query on hosted backbones) | Reference-faithful deterministic retrieval — no LLM in the loop | 2026 | Deliberate divergence, see below |

And the rows the paper cannot have, because they describe problems it never
frames: write-time injection screening with a quarantine that preserves
evidence; read-time content shaping and PII masking at the recall boundary;
tamper-evident audit; two-principal approval quorums; human-in-the-loop
override; deletion with proof — four layers between deleting a row and deleting
the bytes. DISCERN's additive graph never deletes anything, which is correct
for resolution and silent on erasure. Their "governance" gates the *honesty*
of an answer; ours additionally gates what may enter, what may leave, and who
may decide. Neither subsumes the other. That is the diff.

## Where they are ahead, stated plainly

Three mechanisms in the paper formalize what we only half-built, and honesty
requires naming them as the paper's contribution:

**The terminal contract.** Their committed action is a decidable function of
store state: `clarify` when two or more candidate entities survive the
evidence-gathering, `abstain` on demonstrated absence, `update` when the
supersession chain is long, `answer` otherwise — with every gate enforced by
graph predicates rather than model judgment. Our abstain gate is binary and
keyed on retrieval emptiness; there is no `clarify`, no earned-answer check, no
near-miss guard. The paper's ablation puts numbers on the gap: removing the
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
corpora. The store's oldest mechanisms (bi-temporal recall, supersession, the
citation law, the abstain gate) are the paper's mechanisms under different
names, and the store's security seams remain the parts neither they nor the
rest of the memory literature frame.

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
