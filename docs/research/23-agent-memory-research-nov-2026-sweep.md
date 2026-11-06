# 23 · The Agent-Memory Research Sweep (November 2026): the subfield crystallizes, and one attack ships its counter in three days

*The problem.* October's sweep ([20](./20-agent-memory-oct-2026-sweep.md)) caught
the memory-governance wave forming. November caught it crystallizing. The
daily radar logged sixty-plus relevant papers in three weeks, and the shape
of the batch changed: papers stopped arriving as single data points and
started arriving as *clusters* — ownership, provenance-carrying, admission
control, point-of-use evaluation — independent groups re-deriving the same
governance questions within days of each other. One of those questions was
answered here in shipped code before the research file could triage it.

*The shipped answer.* Governance-state corruption
([arXiv:2610.30698](https://arxiv.org/abs/2610.30698), PREPRINT,
2026-10-22) stress-tests what happens when an agent's long-term memory has
its governance state — owner, scope — corrupted while the content stays
intact: content-integrity does not cover governance-integrity. brain-server's
counter **shipped 2026-10-25/26** (release 1.29.32): a row-level
`governance_hash` over the governance tuple, maintained in-transaction by
every write path, with a `brain verify-governance` scan, red-first pins,
and four recorded drills — three days from preprint to shipped
counter-design, red-first throughout (the round record is the AGENTS.md
R138 entry). The honest boundary ships with it: out-of-band tampering
requires local DB access, the stated local-attacker class — tamper-EVIDENT,
not tamper-proof.

## What validated

- **Ownership and provenance.**
  [Whose Credential Is It?](https://arxiv.org/abs/2610.25707) (preprint):
  owner attribution in agent memory shapes cross-user account misuse — the
  `owner`/`access_scope` design and server-side identity labeling, argued
  from measurement. [Compaction
  Laundering](https://arxiv.org/abs/2610.29869) (preprint, by an
  application-security researcher): context compaction strips trust labels,
  laundering poisoned instructions into clean-looking summaries — the
  provenance-rides-the-summary rule, recorded as a design law before the
  surface exists.
- **Admission as a design variable.**
  [Accept Changes, Reject Attacks](https://arxiv.org/abs/2610.41056)
  (preprint): robust memory management *is* the discrimination problem —
  accept the correction, reject the attack. The human gate solves it by
  construction. [I-HITL](https://arxiv.org/abs/2610.35835) (preprint):
  the human checkpoint as an *inalienable* component.
- **Memory over context, measured.**
  [Agent Memory as Rate-Limited Filtering](https://arxiv.org/abs/2611.02361)
  (preprint): explicit state outlasts long context beyond a reader-dependent
  threshold — the memory-layer-over-context-window argument, quantified.
  [FlyHash](https://arxiv.org/abs/2611.05791) (preprint): deterministic
  hashing validated for agent memory — the zero-token family, independently.

## What threatens

- [Log-To-Leak](https://arxiv.org/abs/2610.36108) (preprint): injection
  entering through logs on tool-using MCP agents — the audit trail checked
  as an attack surface. The kernel's chain stores hashes, not content; the
  paper defines the audit question every log pipeline should answer.
- [The disclosure ratchet](https://arxiv.org/abs/2610.28315) (preprint):
  memory-conditioned answers elicit more disclosure, which the memory
  accumulates — leakage widening per session, in ordinary assistance, no
  adversary. The accumulator is the store; provenance, scoped reads, and
  retention throttle it.
- [After the Attacker Leaves](https://arxiv.org/abs/2610.28425) (preprint):
  carryover attacks benchmarked — what keeps working after the attacker is
  gone. [HABIT-Bench](https://arxiv.org/abs/2610.24520) (preprint): habits
  induced from weak longitudinal evidence, no attacker required.
  [Sticky collusion](https://arxiv.org/abs/2610.39660) (preprint) and
  [intervention-history → sabotage](https://arxiv.org/abs/2610.26935)
  (preprints): memory persisting misbehavior past the moment that caused it.
- [Obsolete Facts](https://arxiv.org/abs/2610.36308) (preprint): staleness
  as a distinct pre-generation failure — the bi-temporal design's counter.

## The counter-wave (the honest section)

Three preprints argue designed memory can be a net cost:
[MemoryTax](https://arxiv.org/abs/2610.29823),
[Memory Tax: net value](https://arxiv.org/abs/2610.34073), and
[naive RAG is stronger than we thought](https://arxiv.org/abs/2610.30755).
The concession is recorded: if a memory layer's value is retrieval
optimization, stronger models searching raw history make it a tax. The
bounded answer stands: governance is a different product — approval,
provenance, and provable erasure are not retrieval claims, and raw-history
search offers none of them. The docs answer to this wave is owed and
tracked.

## Fixtures the harness inherits

[AfterDeleteBench](https://arxiv.org/abs/2610.31366) (post-deletion
persistence), [MemHarmBench](https://arxiv.org/abs/2611.01700)
(cross-session injection in memory-persistent agents),
[SciValidityBench](https://arxiv.org/abs/2610.34767) (right fact, wrong
decision), [Epistemic Parity](https://arxiv.org/abs/2610.33128)
(cross-source conflict resolution), and MCCL's
interference/adaptation/transfer triad
([2610.29357](https://arxiv.org/abs/2610.29357)) — the preregistered
protocol set for the planned public harness, beside the naive-RAG baseline
the counter-wave demands.

## The ceiling

Every paper in this file is a PREPRINT (one NeurIPS-2026 workshop acceptance
noted in [20](./20-agent-memory-oct-2026-sweep.md)). Verdicts are relevance
triage against shipped behavior, not replication. The sweep is operational
and daily; this file is the November snapshot, and the working register
lives outside the repository. No affiliation with any listed work.
