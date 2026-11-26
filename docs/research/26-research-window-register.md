# 26 · The Research Window Register

**Date:** 2026-11-26 · **Status:** ACTIVE · **Owner:** StewardMem research
**Policy:** `brain-steward-ip/docs/RESEARCH_WINDOW_POLICY.md` (the rule this file operationalizes)
**Inputs:** [18 · The Cited Work](./18-cited-work.md) and [19 · The Standards Register](./19-standards-register.md) answer *what is cited*. This file answers *what we decided about it, and why*.

---

## What this file is

This is a **decision log, not a bibliography.** One row per paper or standard, one decision per row, one reason per decision. It exists so that a later session can answer four questions about any source the product cites without leaving this file: **which layer is it in, what did we decide, why, and when was it last verified.**

Three laws govern every row:

1. **A refused row with a written reason is a success.** An adopted row with no reason is a liability.
2. **Every date, ID, article number, and designation is verified, never recalled.** Preprints are labelled in the title cell.
3. **Refusals are recorded, not deleted.** Re-opening a refused paper needs a named trigger: a new version that changes a result, a contradicting benchmark, or a shipped attack.

A row without a reason is a defect, not a formatting choice.

---

## The schema

Every row carries the nine columns policy §6 lists (the prompt calls it "eight-field"; the enumerated list has nine, and this file implements the list, not the count):

| Field | Meaning |
|---|---|
| `id` | arXiv ID or canonical URL |
| `title` | Short title; `(preprint)` appended when the source is not peer-reviewed |
| `date` | First submission or publication date, `YYYY-MM-DD` |
| `layer` | `adapt` / `verify` / `re-verify` / `ignore` |
| `status` | `adopted` / `triaged` / `refused` / `parked` |
| `mechanism` | The one mechanism the source bears on |
| `claim` | The StewardMem claim or feature it touches |
| `decision` | `build` / `test` / `cite` / `refuse` / `park` |
| `reason` | The one-line justification, written so it does not get re-litigated |

**Layer meanings** (policy §3): `adapt` = ≤ 6 months old and changes a shipped claim or fills a named gap; `verify` = older but a reference implementation, benchmark, or standard, re-checked against the tree; `re-verify` = an older source contradicted by a newer one; `ignore` = older and off-topic, or newer and unreplicated or off-scope.

**Decision meanings:** `build` = a plan item exists; `test` = an eval or probe, no product change; `cite` = referenced in a note or plan, not built; `refuse` = deliberately not integrated; `park` = a named trigger would re-open it.

---

## Cadence

Four commands, one rule. Run them from `/Users/mark/Sites/arxiv-watch` unless the
path says otherwise; the register is the record, the commands are how it stays
true. A command that exits non-zero produces no register change.

**Daily** — the standing digest, the only scheduled run that advances the cursor:

```sh
python3 arxiv_watch.py --days 5 --mark-seen
```

**Monthly** — the active-window sweep this register was backfilled from. No
`--mark-seen`: the monthly run re-reads the window so a paper the daily digest
missed stays visible.

```sh
python3 arxiv_watch.py --days 180 --max-per-lane 25 --md /tmp/sweep-180d.md --rss /tmp/sweep-180d.xml
```

**Quarterly** — the foundation floor, one query per age band, re-verification
only (`--query` prints and exits; it never touches seen-state):

```sh
python3 arxiv_watch.py --query 'all:"agent memory" AND submittedDate:[202301010000 TO 202512312359]' --max-results 25
python3 arxiv_watch.py --query 'all:"information retrieval" AND submittedDate:[200901010000 TO 202012312359]' --max-results 25
python3 arxiv_watch.py --query 'all:"security fatigue" AND submittedDate:[201601010000 TO 202012312359]' --max-results 25
```

**Annual** — the standards lane, from the `research-watch` registry, never arXiv:

```sh
research_sweep(categories=["standards", "regulatory"], days=365)
```

**The failure rule.** `--mark-seen` is never passed by a scheduled run that did
not exit 0. The tool enforces it: any lane failure raises, `main` exits 2, and
neither the digest nor `state.json` is written. A `--mark-seen` run over an empty
sweep is refused for the same reason. The drill and its receipt are in §Evidence.

---

## 1 · Active window (2026-06-01 → 2026-11-26)

Backfilled from the 180-day sweep of 2026-11-26 (all eight lanes, `--max-per-lane 25`, no `--mark-seen`) and reconciled against notes 20–25 and the two plans. The sweep command, the counts, and the exclusion list are in §Evidence. Papers already adopted get an `adapt / adopted` row that links to the plan; papers a prior note decided get a row that links to the note.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|
| 2611.18133 | Forget the Lineage: Forward-Secure, Lineage-Complete Cryptographic Erasure (preprint) | 2026-11-13 | adapt | adopted | cryptographic erasure hierarchy | DSAR deletion certificates | build | Adopted by plans/proof-hardening (LinEra): key hierarchy + lineage turns a row count into a key-and-lineage certificate; spike first. |
| 2611.07579 | Per-Query Recall Guarantees for Approximate Nearest Neighbor Search in Dynamic Vector Databases (preprint) | 2026-11-05 | adapt | adopted | per-query recall guarantee | deterministic recall | build | Adopted by plans/proof-hardening (PQR-Search): per-query recall probe + exact fallback after deletes. |
| 2611.23602 | A-Tag: Cryptographic File Tagging with Enforced Live Human Presence (preprint) | 2026-11-17 | adapt | adopted | live human presence | human-gated writes | build | Adopted by plans/proof-hardening (A-Tag): presence as evidence, beside the existing digest-bound approval. |
| 2611.32085 | ExpGuard: Guarding Long-Horizon LLM Agents against Corrupted Experience (preprint) | 2026-11-24 | adapt | adopted | execution-dependency guard | KCS solved-case reuse | build | Adopted by plans/proof-hardening (ExpGuard): declared depends_on edges before a resolution is reused. |
| 2606.08919 | Oversight Has a Capacity: Calibrating Agent Guards to a Subjective, Fatiguing Human (preprint) | 2026-06-08 | adapt | adopted | oversight capacity / inverted-U | review queue design | build | Adopted by plans/oversight-ux: escalation past reviewer capacity lowers safety; the queue is ranked and load-aware. |
| 2610.22987 | From Approval to Abuse: Privilege Escalation Attacks on Harness-Protected Coding Agents (preprint) | 2026-10-16 | adapt | adopted | allow-rule privilege escalation | proposal gate / allow-rules | build | Adopted by plans/oversight-ux: broad agent-proposed allow-rules are an attack; any auto-permit is boundary-scoped and expiring. |
| 2605.11360 | Options, Not Clicks: Lattice Refinement for Consent-Driven MCP Authorization (preprint) | 2026-05-12 | adapt | adopted | boundary-scoped auto-permit | review queue design | build | exception: adopted by plans/oversight-ux on 2026-11-26; dated 2026-05-12, below the 2026-06-01 floor, kept because it fills the named friction-budget gap. |
| 2608.27443 | Do User-Authored Permission Policies Improve Protection Against AI Agent Overreach? (preprint) | 2026-08-27 | adapt | adopted | user-authored permission policy | review queue design | build | Adopted by plans/oversight-ux: config-first policy is not a friction win; 133 of 148 overreach actions followed human approval. |
| 2609.02465 | Can Risk-Based Alerting Mitigate Cybersecurity Alert Fatigue? (preprint) | 2026-09-02 | adapt | adopted | risk-based alert ranking | review queue design | build | Adopted by plans/oversight-ux: rank the queue by risk, not chronology; FIFO is a fatigue machine. |
| 2610.35835 | I-HITL: Inalienable Human-in-the-Loop (preprint) | 2026-10-26 | adapt | adopted | inalienable human-in-the-loop | human-gated writes | build | Adopted by plans/oversight-ux: approval must be enforceable on a channel the agent cannot read. |
| 2609.18411 | The Verifiable Action Card: Trustworthy Human-in-the-Loop Control for Secure Automation (preprint) | 2026-09-16 | adapt | adopted | exact-action approval binding | digest-bound approval | build | Adopted by plans/oversight-ux: render canonically out of band, re-verify the exact action at dispatch. |
| 2609.21081 | Loopjacking: Hijacking Human-in-the-Loop Approval (preprint) | 2026-09-17 | adapt | adopted | approval state substitution | digest-bound approval | build | Adopted by plans/oversight-ux: the digest binds the displayed bytes; use-time comparison and atomic consumption are shipped and pinned. |
| 2611.34762 | TASK Memory: Beyond Text-Only Memory with Activation Steering and Knowledge Editing against Memory Hallucination | 2026-11-25 | adapt | triaged | use-time memory hallucination | citation gate / abstention | cite | Correct memory retrieved but contradicted in the answer; the citation gate is the shipped counter, the proposed fix is model-side steering. |
| 2611.33005 | Leto: Utility-Centric Agent Memory | 2026-11-24 | adapt | triaged | utility-centric memory evaluation | custodian eval | cite | Judge memory by whether it improves the response, not fidelity; supports the HEAMS direction, no mechanism. |
| 2611.31656 | BRAG: Bayesian Retrieval Augmented Generation | 2026-11-23 | adapt | refused | learned Bayesian retrieval | deterministic recall | refuse | A learned generative retrieval model; the RRF leg is the shipped law and a learned ranker is refused by name. |
| 2611.31344 | Query Obfuscation with Embedding-Optimized Noise (EON): A Representational Blindspot in Agentic Retrieval | 2026-11-23 | adapt | triaged | query obfuscation blindspot | dense retrieval | cite | Plan proof-hardening §6 cites-but-does-not-build EON; a representational failure in dense retrieval, no shipped surface. |
| 2611.29804 | FORGETMENOT: Auditing Memory Driven Privacy Risks in Commercial AI Agents | 2026-11-22 | adapt | triaged | memory privacy audit | DSAR / erasure | cite | Plan proof-hardening §6 cites-but-does-not-build FORGETMENOT; the leak measurement that pairs with LinEra. |
| 2611.31307 | When Successful Collaboration Becomes an Attack Surface: Insider Memory Poisoning in Multi-Agent Systems | 2026-11-22 | adapt | triaged | insider memory poisoning | memory poisoning | cite | Poisoning through collaboration in multi-agent stores; threat-model input, no new control. |
| 2611.28911 | Ads for Agents: Measuring Ad-Serving MCP Servers | 2026-11-21 | adapt | triaged | ad-serving MCP servers | MCP tool surface | cite | Plan proof-hardening §6 cites-but-does-not-build Ads for Agents; third-party MCP content is the A2M surface. |
| 2611.27138 | Double counting in LLM evidence fusion: exact posteriors separate rule compliance from posterior gain | 2026-11-20 | adapt | triaged | evidence double counting | provenance-weighted evidence | cite | Shared-source reports counted as independent; supports provenance weighting, no deterministic transfer. |
| 2611.27095 | Epistemic Guard: Out-of-Process MCP Receipt Proxying, Post-Turn Prose Supervision, and Provenance-Gated Memory for Tool-Using Autonomous Agents | 2026-11-20 | adapt | triaged | MCP receipt proxying | hash-chained audit | cite | Policy §4 names Epistemic Guard; out-of-process receipts and provenance-gated memory, no build. |
| 2611.27046 | Actions Speak Louder than Words: Stateful Privacy Sniffing of LLM Agents via Behavioral Covert Channels in Web Interactions | 2026-11-20 | adapt | triaged | behavioral covert channels | injection screen | cite | Stateful privacy sniffing through agent behaviour; threat-model input for the read seam. |
| 2611.27041 | Subverting Tool Arguments: Defense-Evading Metadata Poisoning in LLM Agents | 2026-11-20 | adapt | triaged | tool-metadata argument poisoning | MCP tool surface | cite | Policy §4 names defense-evading metadata poisoning; the untrusted-metadata posture already holds. |
| 2611.26427 | MemFolder: Structure-Guided Memory Folding for Efficient Agent Memory Management | 2026-11-19 | adapt | triaged | structure-guided memory folding | consolidation | cite | Consolidation for cost; adjacent to doc 12, no deterministic mechanism to adopt. |
| 2611.26361 | How Much Should an Agent Remember? Deployment Lessons from Self-Distilled Memory for LLM Agents | 2026-11-19 | adapt | triaged | self-distilled procedural lessons | retention policy | cite | Learns short lessons from its own trajectories; supports retention limits, the learned pipeline is refused. |
| 2611.26050 | Studying AI Act Mentions: A first look at the impact of the European Union's AI regulation on Hugging Face | 2026-11-19 | adapt | triaged | AI Act adoption measured | standards currency | cite | Measures AI Act mentions on Hugging Face; evidence the standards lane is live, not a shipped claim. |
| 2611.25855 | HabitusMem: Belief Memory over Evolving User Preferences for LLM Agents | 2026-11-19 | adapt | triaged | belief memory over preferences | bi-temporal supersession | cite | Models whether a preference is still current; validates supersession, no new store shape. |
| 2611.25583 | DriftGuard: Online Memory-Drift Detection via Causal Attribution for LLM Agents | 2026-11-19 | adapt | refused | causal memory-drift attribution | consolidation | refuse | Plan proof-hardening §8: drift_census is a census-breach recorder, not drift attribution; scope exclusion. |
| 2611.25123 | Forget Freely, Grep Later: Lossless, Index-Free Memory for Long-Horizon LLM Agents | 2026-11-18 | adapt | triaged | lossless index-free memory | memory-vs-context claim | cite | Raw-history retrieval as the baseline the counter-wave demands; governance remains the differentiator. |
| 2611.24422 | Resolution as Attention for Efficient Agent Memory Perception | 2026-11-18 | adapt | triaged | resolution as attention | resolution seam | cite | Allocates perceptual detail per interaction; adjacent to the resolution seam, no deterministic transfer. |
| 2611.24165 | MemoSync: Co-Managing Developer-Agent Metamemory Across Contexts | 2026-11-18 | adapt | triaged | developer metamemory | transparency | cite | Developers cannot see what the agent stores and applies; supports the review surface, no mechanism. |
| 2611.23588 | ARISE: Practical Self-Evolving Memory for Accurate and Cost-Efficient Enterprise Agents | 2026-11-17 | adapt | refused | self-evolving memory | human-gated policy loop | refuse | Evolves memory from its own failures; the store keeps the human in the policy loop, so the autonomous loop is refused. |
| 2611.23346 | Securing and Auditing Long-Horizon LLM Agents: State-Transfer Auditing and Controls in the Agent Harness | 2026-11-17 | adapt | adopted | boundary-transfer audit | hash-chained audit | build | Adopted by plans/proof-hardening (State-Transfer): audit the seams, not only prompts and tool calls. |
| 2611.22461 | Surfacing Disagreement Beats Resolving It: A Gated Ablation Study of Provenance-Weighted Memory for Multi-Agent Debate | 2026-11-17 | adapt | triaged | surfacing disagreement | preserve-at-write / resolve-at-read | cite | Preserving disagreement beats resolving it; independent validation of the two-seam posture. |
| 2611.22245 | Multi-Agent Memory Inconsistency: Where Factual Conflicts Disappear in LLM Agents | 2026-11-17 | adapt | refused | multi-agent memory inconsistency | multi-principal store | refuse | Plan proof-hardening §8: matters only if delegations ship; parked behind that trigger. |
| 2611.22179 | RAG in a Trenchcoat: When Minimal Memory Is Enough for Agentic Systems, and When It Isn't | 2026-11-17 | adapt | triaged | minimal RAG memory | memory-vs-context claim | cite | Policy §4 names RAG in a Trenchcoat; almost none of the added primitives earn their cost, a counter-wave citation. |
| 2611.22102 | EvoDisco: Strategy Discovery for Evolving Agent Memory | 2026-11-17 | adapt | refused | evolved memory operators | write/recall path | refuse | Evolutionary discovery of memory operators; a learned policy in the write or recall path is refused by the zero-token law. |
| 2611.21811 | ARC-2: Continuous Compliance Through Delta Management | 2026-11-16 | adapt | triaged | continuous compliance deltas | compliance evidence | cite | Policy §4 names ARC-2; delta management for post-deployment compliance, no build. |
| 2611.21577 | When Tool Names Are Not Identities: Measuring Provider Misbinding in MCP-Based AI Agents | 2026-11-16 | adapt | triaged | provider misbinding | MCP tool surface | cite | Policy §4 names provider misbinding; tool names are not identities, the static self-authored tools posture holds. |
| 2611.21478 | Jetriever: Shifting Agent Memory Retrieval from Similarity to Utility | 2026-11-16 | adapt | refused | LLM utility ranking | deterministic recall | refuse | An LLM-derived System One scores every memory; a learned ranker in recall is refused, the utility question stays in eval. |
| 2611.21259 | Evaluating Authorization and Trust Boundaries in Agentic Orchestration Frameworks | 2026-11-16 | adapt | triaged | orchestration trust boundaries | authorization | cite | Evaluates authorization and trust boundaries across frameworks; threat-model input for the gate seam. |
| 2611.21019 | MemDoctor: Stage-wise Error Attribution and Repair for MLLM Agent Memory | 2026-11-16 | adapt | triaged | stage-wise memory error attribution | memory repair | cite | Attributes a wrong answer to write, update, or retrieve; diagnostic vocabulary, no shipped mechanism. |
| 2611.20927 | LEDGER: Auditable Agent Memory Under Change | 2026-11-16 | adapt | triaged | auditable memory under change | hash-chained audit | cite | Policy §4 names LEDGER; explicit rules decide current value and deletion, close to the store's posture but not built. |
| 2611.20922 | Experience Rot: Experienced Agents Fail Where Novices Succeed | 2026-11-16 | adapt | triaged | experience rot | KCS reuse | cite | Accumulated experience degrades decisions; independent motivation for ExpGuard's dependency invalidation. |
| 2611.20759 | Instant Weight-Space Consolidation for Long-Term Memory in LLMs | 2026-11-16 | adapt | refused | weight-space consolidation | store architecture | refuse | Consolidates into model weights; the model is not the store, refused as out-of-architecture. |
| 2611.20298 | Type-Confusion Security | 2026-11-15 | adapt | triaged | type-confusion security | MCP tool surface | cite | Type confusion at a trust boundary; threat-model input, no shipped surface named. |
| 2611.20144 | HEAMS: Holistic Evaluation of Agent Memory Systems | 2026-11-15 | adapt | adopted | custodian evaluation | the whole governance story | test | Adopted by plans/proof-hardening (HEAMS): five-axis custodian score; build as an eval, not a product change. |
| 2611.19859 | When Facts Change, Which Decisions Break? Dependency-Aware Context Graphs for Agents | 2026-11-15 | adapt | triaged | dependency-aware context graphs | KB lifecycle | cite | Policy §4 names dependency invalidation; which decisions break when a fact changes, the ExpGuard direction. |
| 2611.19752 | StaleBench: Isolating Stale Retrieval in Agent Memory, and Pricing the Fix | 2026-11-15 | adapt | adopted | Contamination@5 freshness metric | freshness / currency | test | Adopted by plans/proof-hardening (StaleBench): stale material in the top-5 priced beside recall. |
| 2611.18439 | Between Obligations and Evidence: A Participatory Study of Translating the EU AI Act into Technical Requirements | 2026-11-13 | adapt | triaged | obligations to evidence | compliance evidence | cite | Plan proof-hardening §8: AI Act evidence framing, routed to the standards register, not code. |
| 2611.15925 | ShadowBench: Exposing Lexical Anchoring and the Illusion of Forgetting in Large Language Models | 2026-11-11 | adapt | triaged | lexical anchoring / illusory forgetting | erasure ceiling | cite | Retrieval fails without name tokens, faking forgetting; supports the store-side erasure distinction. |
| 2611.15013 | Protecting Sensitive Knowledge Under LLM Unlearning: The Proxy Threat and a Superposition-Aware Defense | 2026-11-11 | adapt | triaged | unlearning proxy threat | erasure ceiling | cite | Overlapping representations survive unlearning; model-side residue the store cannot control, recorded as a ceiling. |
| 2611.14808 | OverServe: Resource Amplification Attacks on SaaS Agents via Tool--Prompt Co-Design | 2026-11-10 | adapt | triaged | resource amplification | MCP tool surface | cite | Tool-prompt co-design amplifies provider cost; threat-input for SaaS agent surfaces. |
| 2611.11539 | PRISM: Causal Counterfactual Minimization of Agent Tool Privileges via Information Bottleneck Role Induction | 2026-11-08 | adapt | triaged | counterfactual tool privileges | authorization | cite | Minimizes tool privileges via an information bottleneck; adjacent to scope-gated tools, the learned method is not adopted. |
| 2611.10843 | Agents of Chaos: A Systematization of Knowledge (SoK) on Malicious Attacks on Agentic AI Systems | 2026-11-07 | adapt | triaged | agentic attack SoK | threat model | cite | Systematization of attacks across memory, tools, and coordination; a threat-model index, no new control. |
| 2611.09721 | MemoryDojo: Benchmarking Poisoning Through LLM Agent Memory | 2026-11-06 | adapt | triaged | memory poisoning benchmark | eval harness | test | MemoryDojo benchmarks poisoning across memory systems; an eval-fixture candidate for the public harness. |
| 2611.05866 | Auditable Unlearning for LLMs | 2026-11-04 | adapt | triaged | auditable unlearning | erasure certificate | cite | Plan proof-hardening §8: framing for the certificate's fourth axis; folded into LinEra's certificate shape. |
| 2611.04546 | Are LLM Assistants Regulatory Ready? Answers from regenold's 2026 EU AI Act Q&A Challenge | 2026-11-03 | adapt | triaged | AI Act Q&A benchmark | compliance evidence | cite | 110 expert-curated AI Act questions; a benchmark for the compliance evidence pack, not a product claim. |
| 2611.03121 | ComplianceAtlas: A Taxonomy-Driven Agentic AI Framework for Automated AI Act Gap Analysis and Remediation | 2026-11-02 | adapt | triaged | AI Act gap analysis | compliance evidence | cite | Policy §4 names ComplianceAtlas; taxonomy-driven gap analysis, no build. |
| 2611.02605 | StateFence: Task-State Protection Against Evidence-Chain Contamination in Blue-Team LLM Agents | 2026-11-02 | adapt | triaged | task-state contamination | injection screen | cite | Protects a blue-team agent's task state from evidence-chain contamination; threat-input for the injection screen. |
| 2611.01700 | MemHarmBench: Evaluating Cross-Session Prompt Injection in Memory-Persistent AI Agents | 2026-11-01 | adapt | triaged | cross-session injection benchmark | eval harness | cite | Plan proof-hardening §8: fixture candidate for the public harness. |
| 2610.41056 | Accept Changes, Reject Attacks: Robust Memory Management for LLM Agents | 2026-10-29 | adapt | triaged | admission as discrimination | human write gate | cite | Note 23: robust memory is accepting the correction and rejecting the attack; the human gate is the answer by construction. |
| 2610.38543 | The State of the Practice in Privacy Impact Assessments: Understanding the Professionals' Perspectives, Experiences, and Current Procedures | 2026-10-28 | adapt | triaged | privacy impact assessment practice | compliance evidence | cite | How professionals run PIAs and DPIAs; context for the compliance evidence pack, not a shipped claim. |
| 2610.35451 | Then there is prompt injection, a malicious email, ...: The Wild Wide West of OpenClaw Agentic System Security | 2026-10-26 | adapt | triaged | open-web prompt injection | injection screen | cite | Field report of injection in an agentic deployment; threat-input, the screen is the shipped control. |
| 2610.34800 | Phantom Forgetting: How BatchNorm State Fakes Deletion in Edge Split Machine Unlearning | 2026-10-26 | adapt | triaged | phantom forgetting | erasure ceiling | cite | BatchNorm state fakes deletion in split learning; an erasure-evaluation pitfall, recorded as a ceiling. |
| 2610.33112 | Conformance as a Distribution: Delegation Contracts, Error Budgets, and the Allocation of Evaluation Authority in Use-Based AI Regulation | 2026-10-23 | adapt | triaged | conformance as a distribution | compliance evidence | cite | Conformance is a failure rate under a declared distribution, not a model score; sharpens the evidence claim. |
| 2610.31588 | VERA: A Responsible AI Dashboard for Assessing LLM Deployments in Light of the EU AI Act | 2026-10-22 | adapt | triaged | responsible-AI dashboard | compliance evidence | cite | VERA maps benchmarks to AI Act principles; adjacent to the evidence pack, no build. |
| 2610.31380 | AI Risk Ontologies for the EU AI Act Compliance: AIRO, VAIR, and their Integration with DPV | 2026-10-22 | adapt | triaged | AI risk ontologies | compliance evidence | cite | AIRO/VAIR ontologies for AI Act risks; vocabulary for the standards register, no shipped surface. |
| 2610.28810 | Combining Long-Term Memory and LLMs Comes at a High Cost: From Prompt Injection to Persistent Compromise | 2026-10-20 | adapt | triaged | persistent compromise via memory | memory poisoning | cite | Long-term memory turns a one-session injection into a persistent compromise; the write gate is the counter. |
| 2610.28425 | After the Attacker Leaves: Benchmarking Carryover Attacks on LLM Agents | 2026-10-20 | adapt | triaged | carryover attacks | injection screen | cite | Note 23: attacks that keep working after the attacker leaves; threat-input, counter is the write gate plus provenance. |
| 2610.22828 | GRAD: Graph-Structured Antibody Defense for LLM Agents | 2026-10-16 | adapt | triaged | graph antibody defense | quarantine posture | cite | Graph-structured defense against distributed poisoning; comparison point for quarantine, not integrated. |
| 2610.19613 | Are We Really Measuring Forgetting? Pitfalls in Evaluating Machine Unlearning for Multimodal LLMs | 2026-10-14 | adapt | triaged | multimodal unlearning evaluation | erasure evidence | cite | Three pitfalls in measuring forgetting; eval discipline for any erasure claim. |
| 2610.17659 | Can Agents Truly Forget? Explore Machine Unlearning for LLM Agents | 2026-10-13 | adapt | triaged | agent-level unlearning | erasure ceiling | cite | Can agents truly forget; the model-side comparison to the store-side certified deletion. |
| 2610.16916 | Transparent Information Flow Modeling for Role-Aware AI Act Assessment | 2026-10-12 | adapt | triaged | role-aware AI Act assessment | compliance evidence | cite | Information-flow modelling for role-aware assessment; standards vocabulary, no build. |
| 2610.14431 | MemForm: Benchmarking Memory Formation Poisoning in LLM Agents | 2026-10-10 | adapt | triaged | memory-formation poisoning benchmark | eval harness | test | MemForm characterizes the formation interface; an eval-fixture candidate beside MemoryDojo. |
| 2610.13267 | Forgetting is Not Always Bad: A Neuro-Inspired Memory Repair Mechanism for Poisoned LLM Agents | 2026-10-09 | adapt | triaged | repair by decay | memory poisoning defense | cite | Note 20: heuristic forgetting is a probability; certified erasure is a receipt, so decay is not adopted as the repair. |
| 2610.10957 | TRACE: A Governance Framework for Measuring Explainability Debt in Production AI Systems | 2026-10-07 | adapt | triaged | explainability debt | compliance evidence | cite | TRACE measures the progressive inability to explain a decision; adjacent to the trace-replay claim. |
| 2610.10629 | The Harness as the Only Mutable Surface: Compliance-Bounded Self-Evolution of LLM Agents in Credit Pipelines, with a Measured Admission Gate | 2026-10-07 | adapt | triaged | reviewable self-evolution | human-gated change surface | cite | Confines self-evolution to the reviewable harness; independent support for a named, approved change surface. |
| 2610.08951 | ASPIRE: Agentic Safety & Prompt Injection Red-teaming Engine | 2026-10-06 | adapt | triaged | injection red-teaming | eval harness | cite | ASPIRE is a red-team engine; a harness input, not a shipped mechanism. |
| 2610.04310 | Modeling Deletion Requests in Machine Unlearning | 2026-10-03 | adapt | triaged | deletion-request behaviour | DSAR UX | cite | Note 20: how requesters exercise erasure; DSAR UX input, no mechanism. |
| 2610.04083 | Self-Propagating Misalignment in LLM Agents, and Why Auditing or Disabling Memory Is Not Enough | 2026-10-02 | adapt | triaged | self-propagating misalignment | write gate | cite | Note 20: no-adversary delayed payload; counter is the write gate, provenance, and audit chain already shipped. |
| 2610.03073 | SecJev: Bringing Security Expertise to System One Decision Models | 2026-10-02 | adapt | triaged | System One security decisions | calibration | cite | Brings security expertise to System One decision models; adjacent to the calibrated-abstention lane. |
| 2610.00392 | Measuring LLM Agent Defenses by Layer: GDA Measurement and an Envelope-Layer Attack-Defense Model | 2026-09-30 | adapt | triaged | defense measurement by layer | injection screen | cite | Separates model recognition failure from agent-layer blocking; a measurement discipline for the screen. |
| 2609.37196 | ToolFence: Fine-Grained Authorization for Secure Tool-Using LLM Agents | 2026-09-29 | adapt | triaged | fine-grained tool authorization | authorization | cite | ToolFence catches within-tool argument hijacking; adjacent to scope-gated tools, no adoption. |
| 2609.33371 | API Secrets Should Never Become Tokens in the LLM's Vocabulary: A Threat Analysis of API Credential Handling in LLM Agent Systems and an Empirical Evaluation of a Vault-Mediated Execution Boundary | 2026-09-27 | adapt | triaged | credential leakage into memory | secret boundary | cite | Pasted credentials persist in history, logs, and memory; supports the secret-file boundary, no new control. |
| 2609.33102 | ORBIT: A Framework for Multi-Agent Safety and Security Evaluations | 2026-09-27 | adapt | triaged | multi-agent safety evaluation | eval harness | cite | ORBIT evaluates multi-agent safety and security; harness input, no shipped mechanism. |
| 2609.32103 | LLM Unlearning Evaluation with TRIAGE | 2026-09-26 | adapt | triaged | unlearning evaluation | erasure evidence | cite | Note 20: model-world unlearning needs mechanistic evidence; the store-side answer is bytes-gone receipts. |
| 2609.29045 | The Tokens Remember: When Tokenization Bypasses Knowledge Editing and Unlearning | 2026-09-24 | adapt | triaged | tokenization residue | erasure ceiling | cite | Note 20: deletion claims break on physical residue; the sealed-snapshot posture already states this ceiling. |
| 2609.22818 | The Price of Safety: Benign-Case Utility and Token Overhead of Memory-Poisoning Defenses in LLM Agents | 2026-09-19 | adapt | triaged | defense token cost | poisoning defense economics | cite | Note 20: every surveyed defense pays tokens on the hot path; the deterministic gate pays zero, a publishable cost axis. |
| 2609.22712 | Trustworthy Agentic AI: Failure Modes, Mitigation Strategies, and a Lifecycle Framework for Autonomous LLM Systems | 2026-09-19 | adapt | triaged | agentic-AI lifecycle trust | threat model | cite | Failure modes and a lifecycle framework; a taxonomy for the threat model, no build. |
| 2609.17648 | Trust propagation and structural containment in Multi-agent LLM pipelines | 2026-09-15 | adapt | triaged | trust propagation | authorization | cite | Low-privilege agents influence higher-privilege ones; threat-input for per-principal authorization. |
| 2609.17320 | Emergence World: Adversarial Stress-Testing of Long-Horizon Multi-Agent Systems | 2026-09-15 | adapt | triaged | long-horizon adversarial stress test | eval harness | cite | Emergence World runs persistent multi-agent stress tests; a harness pattern, not a product change. |
| 2609.16098 | Universal Defenses for Tool-Integrated LLM Agents Against Adversarial Attacks | 2026-09-14 | adapt | triaged | universal tool-agent defenses | injection screen | cite | Defenses across injection, poisoning, and backdoors; a comparison set, not integrated. |
| 2609.13889 | When Malicious Instructions Persist: Persistent Memory Poisoning Attack on Harness-Based Agents | 2026-09-12 | adapt | triaged | harness persistent poisoning | memory poisoning | cite | Note 20: PMPA attacks the harness-plus-memory class; threat-model input, no new control. |
| 2609.09404 | An Experimental Evaluation of Multimodal Prompt Injection Attacks on Agentic AI Frameworks | 2026-09-08 | adapt | triaged | multimodal prompt injection | injection screen | cite | MMPIBench measures injection delivered through images; threat-input for the screen's input classes. |
| 2609.08747 | MemSentry: A Framework for Detecting Persistent Memory Poisoning in Agentic AI | 2026-09-08 | adapt | triaged | poisoning detection | quarantine posture | cite | Note 20: a detection framework to compare against quarantine; not integrated. |
| 2609.02265 | CAPTURE: Disentangling Preference Drift from Memory Poisoning in Personalized LLM Agents | 2026-09-02 | adapt | triaged | preference drift vs poisoning | memory poisoning | cite | Separates genuine drift from poisoning; the review queue already asks the human this question. |
| 2609.00523 | Transferable End-to-End Optimization for Indirect Long-Term Memory Poisoning in LLM Agents | 2026-09-01 | adapt | triaged | end-to-end memory poisoning | memory poisoning | cite | Optimizes write, retrieve, and use together; threat-input that the write gate must stay the boundary. |
| 2608.30177 | Understanding Stage-Wise Utility-Risk Trade-offs in LLM Agent Memory | 2026-08-31 | adapt | triaged | stage-wise utility-risk trade-off | retention policy | cite | More retained information raises both utility and risk; supports retention as a control, no mechanism. |
| 2608.29943 | On the Recoverability of Private Information Unlearning in Large Language Models | 2026-08-30 | adapt | triaged | unlearning recoverability | erasure ceiling | cite | Quantifies whether unlearning erased or hid; the model-side analogue of a deletion receipt. |
| 2609.29647 | AgentKernel: The Trust-Native Agentic Operating System | 2026-08-29 | adapt | triaged | trust-native agent OS | architecture | cite | AgentKernel puts trust at the OS layer; adjacent architecture, nothing to adopt for a memory store. |
| 2608.25429 | Distance Is Not Enough: Forget-Retain Alignment Gap Predicts LLM Relearning Robustness | 2026-08-26 | adapt | triaged | relearning robustness | erasure ceiling | cite | Weight distance does not predict relearning; an erasure-claim caveat, recorded not adopted. |
| 2608.22061 | MEMORY Wins All: Indirect Bias Injection Attacks via Social Media Content | 2026-08-22 | adapt | triaged | indirect bias injection | memory poisoning | cite | Social-media content steers persistent memory toward a bias; threat-input for untrusted ingestion. |
| 2608.21230 | Utility Under Attack: Agent Memory Poisoning and the Limits of Content Screening and Provenance Ranking | 2026-08-21 | adapt | triaged | content screening limits | memory poisoning | cite | Poisoning 1.2% of a LongMemEval corpus drops accuracy 0.85 to 0.30; the write gate is the counter, screening alone is not. |
| 2608.20631 | Weighted Memory Tree: Remembering What Matters for Long-Horizon LLM Agents | 2026-08-21 | adapt | triaged | weighted memory retention | retention policy | cite | Weighted memory tree for long-horizon recall; an alternative retention heuristic, not adopted. |
| 2608.03844 | MAFIA: Query-Only Memory Attacks via Probing and Factual Injection against Audited LLM Agents | 2026-08-04 | adapt | triaged | query-only memory attacks | memory poisoning | cite | MAFIA stays effective in large benign pools and under audit; threat-input that auditing alone is not a defense. |
| 2608.01637 | Salami Attack: Stealthy Collusive Memory Poisoning against OpenClaw | 2026-08-03 | adapt | triaged | collusive memory poisoning | memory poisoning | cite | Salami attack: individually benign records collude; motivates whole-store review, not per-item screening. |
| 2606.19242 | Runtime Compliance Verification for AI Agents | 2026-06-17 | adapt | triaged | runtime GDPR tracing | compliance evidence | cite | Note 20: runtime conformance tracing for agents; nearest peer framing of the gate layer. |

## 2 · Foundation floor

Reference implementations, benchmarks, and standards that are older than the adaptation window. These are **verified against the tree, not re-adopted.** Every `verify` row names a tree anchor (`path:line` or a test name) and a check date; a row with no anchor is `verify / gap` and becomes a plan item.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|
| RRF-2009 | RRF (Cormack, Clarke & Büttcher, 2009) | 2009 (SIGIR) | verify | triaged | `src/search/mod.rs:31` RRF_K=60; `src/search/mod.rs:1088` rrf_fuse | hybrid fusion | cite | checked 2026-11-26: RRF_K=60 and rrf_fuse are in the tree; the 2009 paper is the reference, not re-adopted. |
| BM25-2009 | BM25 (Robertson & Zaragoza, 2009) | 2009 (FnT IR) | verify | triaged | `src/search/mod.rs:1838` bm25(knowledge_fts) | lexical leg | cite | checked 2026-11-26: FTS5 bm25 ranking is the lexical leg; no separate BM25 implementation. |
| PQ-2011 | Product Quantization (Jégou et al., 2011) | 2011 (TPAMI) | verify | triaged | `src/migration.rs:552` vec_quantize_int8 + vec_quantize_binary | vector storage | cite | checked 2026-11-26: int8+binary scalar quantization in the PQ family, not PQ itself; the docs say so. |
| 2005.11401 | RAG (Lewis et al., 2020) | 2020-05-22 | verify | triaged | `src/chunker.rs:75` chunk_markdown | chunk-then-retrieve | cite | checked 2026-11-26: heading-aware splitter exists; RAG is framing, retrieval stays deterministic. |
| 2310.08560 | MemGPT (Packer et al., 2023) | 2023-10-12 | verify | triaged | `src/service/suggest.rs:67` record_feedback + the suggest surface | opt-in anticipation | cite | checked 2026-11-26: anticipation is reviewable and nothing is silently injected; the paging metaphor is not implemented. |
| 2304.03442 | Generative Agents (Park et al., 2023) | 2023-04-07 | verify | triaged | `src/handlers/ump_ops.rs:45` retrieval_signals | recall scoring | cite | checked 2026-11-26: similarity, recency, salience, scope, and provenance are explicit signals. |
| 2402.17753 | LoCoMo (Maharana et al., 2024) | 2024-02-27 | verify | triaged | gap: no LoCoMo harness in tree; plan item in `docs/research/13-benchmark-landscape-2026.md` | memory benchmark | build | checked 2026-11-26: the benchmark shape is cited, but no harness runs it; the public harness is the plan item. |
| 2410.10813 | LongMemEval (Wu et al., 2024) | 2024-10-14 | verify | triaged | gap: no harness in tree; `BENCHMARKS.md:637` explicitly disclaims parity | memory benchmark | build | checked 2026-11-26: named in doc 13 and explicitly not a parity claim; the harness is the plan item. |
| mem0-product | Mem0 (product) | n/a (product) | verify | triaged | `src/service/suggest.rs:67` record_feedback | UMP feedback | cite | checked 2026-11-26: the memory_id/feedback shape is the UMP feedback surface; a product, not a paper. |
| letta-product | Letta (product; formerly MemGPT) | n/a (product) | verify | parked | gap: no self-editing memory in tree; not a shipped claim | agent self-editing | park | checked 2026-11-26: nothing implements agent self-editing; no claim depends on it, so parked, not built. |
| 2501.13956 | Graphiti / Zep (2025) | 2025-01-20 | verify | triaged | `src/graph_supersede.rs:10` valid_at/invalid_at/superseded_at | bi-temporal KB | cite | checked 2026-11-26: the bi-temporal triple and the at-most-one-current invariant are in the tree. |
| 2502.14802 | HippoRAG 2 (2025) | 2025-02-20 | verify | triaged | `src/search/graph_ppr.rs:199` personalized_pagerank | graph retrieval | cite | checked 2026-11-26: PPR over the graph is the graph leg; the reference is HippoRAG-2-style, not a fork. |
| NIST-fatigue-2016 | Security fatigue (Stanton et al., NIST, 2016) | 2016 (NIST) | verify | triaged | `src/handlers/gate.rs:314` proposal TTL; `src/handlers/gate.rs:1213` quorum_gate | review queue | cite | checked 2026-11-26: TTL and quorum bound approval load; the fatigue model itself is a design input, not measured here. |
| Nouwens-2020 | Consent fatigue (Nouwens et al., 2020) | 2020 (USENIX) | verify | triaged | `src/handlers/gate.rs:1544` outreach_consent branch | consent registry | cite | checked 2026-11-26: the consent registry and the outreach_consent gate branch are in the tree. |
| 1706.04599 | On Calibration of Modern Neural Networks (Guo et al., 2017) | 2017-06-14 | verify | triaged | `src/handlers/recall.rs:964` abstention_decision | calibrated abstention | cite | checked 2026-11-26: the gate is threshold-based; temperature scaling is not fitted, so the calibration reference is cited, not implemented. |
| 2312.10997 | Retrieval-Augmented Generation for Large Language Models: A Survey (Gao et al., 2023) | 2023-12-18 | verify | triaged | `src/chunker.rs:75` chunk_markdown | RAG taxonomy | cite | checked 2026-11-26: a survey of the chunk-then-retrieve shape the chunker implements; no mechanism adopted. |
| 2404.16130 | From Local to Global: A Graph RAG Approach to Query-Focused Summarization (Edge et al., 2024) | 2024-04-24 | verify | triaged | `src/search/graph_ppr.rs:199` personalized_pagerank | graph retrieval | cite | checked 2026-11-26: the graph leg is PPR; GraphRAG's community-summary pipeline is not implemented and is not claimed. |
| 2507.09089 | Measuring the Impact of Early-2025 AI on Experienced Open-Source Developer Productivity (METR, 2025) | 2025-07-12 | verify | triaged | gap: no productivity measurement in the tree | honest ceilings | cite | checked 2026-11-26: counter-evidence that AI tooling can slow experts; no shipped claim depends on a speedup, and none is made. |
| 2512.13564 | Memory in the Age of AI Agents (survey, 2025) | 2025-12-15 | verify | triaged | gap: framing only; the design space is implemented across docs 01-17 | memory taxonomy | cite | checked 2026-11-26: cited in the index as the survey that taxonomizes the deterministic design space; no mechanism to adopt. |
| 2602.03578 | Use Graph When It Needs: Efficiently and Adaptively Integrating RAG with Graphs (2026) | 2026-02-03 | verify | triaged | `src/search/mod.rs:1617` complexity-gated graph activation | graph activation | cite | checked 2026-11-26: the code names this paper at the gate; the graph leg engages only above the complexity threshold. |
| 2602.05665 | Graph-based Agent Memory: Taxonomy, Techniques, and Applications (2026) | 2026-02-05 | verify | triaged | gap: framing only; the graph store is `src/search/graph_ppr.rs` | graph-memory direction | cite | checked 2026-11-26: the index cites it as institutionalizing the direction; a taxonomy, no mechanism. |
| 2603.27910 | GAAMA: Graph Augmented Associative Memory for Agents (2026) | 2026-03-29 | verify | triaged | `src/search/graph_ppr.rs:137` dampen_hubs | hub dampening | cite | checked 2026-11-26: the `w_ij · min(1, θ/deg(i))` dampening is implemented with θ = 50. |
| 2605.01386 | MemORAI: Memory Organization and Retrieval via Adaptive Graph Intelligence (2026) | 2026-05-02 | verify | triaged | `src/search/graph_ppr.rs:64` type_base_weight | static-type weighting | cite | checked 2026-11-26: relation-type base weights are static, as the paper argues; no learned adaptation. |
| 2606.01435 | Reliable Post-Retrieval Assembly for Agent Memory (2026) | 2026-05-31 | verify | triaged | `src/consolidate.rs:473` find_subject_conflicts | deterministic conflict resolution | cite | checked 2026-11-26: conflicts are detected and put to a human, not resolved autonomously; the posture matches, the code is independent. |

## 3 · Standards / regulation

Continuous, not date-bounded. Every row carries a canonical URL and `verified YYYY-MM-DD`. A moved designation is corrected in [19](./19-standards-register.md) and recorded here as `re-verify / corrected`, naming old → new.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|
| ISO/IEC 42001:2023 | AI management systems | 2023 | verify | triaged | AI management system | compliance evidence | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/81230.html |
| ISO/IEC 23894:2023 | AI risk management | 2023 | verify | triaged | AI risk guidance | compliance evidence | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/77304.html |
| ISO/IEC 27001:2022 | Information security management | 2022 | verify | triaged | ISMS baseline | compliance evidence | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/27001 |
| ISO/IEC 30401:2018 | Knowledge management systems | 2018 | verify | triaged | KCS loop | solved-case reuse | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/68683.html |
| ISO 10002:2018 | Complaints handling | 2018 | verify | triaged | complaint lifecycle | complaint register | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/71580.html |
| ISO 10003:2018 | External complaints handling | 2018 | verify | triaged | external disputes | remedy matrix | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/71580.html (series page) |
| ISO 10004:2018 | Satisfaction monitoring | 2018 | verify | triaged | outcome measurement | goodwilling ledger | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/71580.html (series page) |
| ISO 10001:2018 | Quality management — umbrella | 2018 | verify | triaged | quality frame | complaint lifecycle | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/62085.html |
| ISO 18295-1:2017 | Customer contact centres | 2017 | verify | triaged | contact-centre process | self-assessment | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/64739.html |
| ISO 23592:2021 | Data quality | 2021 | verify | triaged | data-quality measurement | consolidation posture | cite | verified 2026-11-26 (designation + URL; iso.org returns 403 to automated fetch, so the text was not re-read) · https://www.iso.org/standard/68683.html (catalogue) |
| Regulation (EU) 2016/679 (GDPR) | Personal-data law | 2018-05-25 | verify | triaged | data-subject rights | DSAR / erasure / access | cite | verified 2026-11-26 against the EUR-Lex OJ: Arts 4, 10, 12-17, 19, 22 re-read; the article table was corrected (next row) · https://eur-lex.europa.eu/eli/reg/2016/679/oj |
| GDPR article table — CORRECTED | old → new mapping | 2018-05-25 | re-verify | corrected | article numbers | DSAR / erasure / access | cite | verified 2026-11-26: old 'Art 4 AI literacy / Art 10 procurement trace / Art 12-13 logging / Art 14-16 14-day clock' → new 'Art 4 definitions / Art 10 criminal-conviction data / Art 12 transparency + one-month response / Art 13-14 information duties / Art 15 access / Art 16 rectification / Art 17 erasure / Art 19 onward notification / Art 22 automated decisions'. AI literacy is AI Act Art 4; logging is AI Act Art 12. https://eur-lex.europa.eu/eli/reg/2016/679/oj |
| Regulation (EU) 2024/1689 (AI Act) | EU Artificial Intelligence Act | 2024-08-01 | verify | triaged | AI obligations | compliance evidence | build | verified 2026-11-26: Art 113 dates confirmed (in force 2024-08-01; applies 2026-08-02; Ch I-II 2025-02-02); Arts 5, 10, 12, 14, 26(6), 50 read; Art 99 EUR 15M/3% ceiling; Digital Omnibus 2026/1744 adds Art 50(2) marking by 2026-12-02 and high-risk 2027-12-02 / 2028-08-02 · https://eur-lex.europa.eu/eli/reg/2024/1689/oj |
| Regulation (EU) 2024/3228 (ADR) | Alternative dispute resolution | 2024 | verify | triaged | ADR bodies | complaint escalation | cite | verified 2026-11-26: ODR platform discontinued 2025-07-20; national ADR body is the target · https://eur-lex.europa.eu/eli/reg/2024/3228/oj |
| Regulation (EU) 2024/2847 (CRA) | Cyber Resilience Act | 2024-12-10 | verify | triaged | cyber-incident reporting | incident runbook | cite | verified 2026-11-26 against the OJ: published 2024-11-20, in force 2024-12-10, reporting from 2026-09-11, main obligations 2027-12-11; 24h early warning, 72h notification, final report within one month (corrected) · https://eur-lex.europa.eu/eli/reg/2024/2847/oj |
| CRA final-report leg — CORRECTED | old → new deadline | 2024-12-10 | re-verify | corrected | incident-report clock | incident runbook | cite | verified 2026-11-26 against the OJ: old 'final report within 14 days' → new 'final report within one month after the incident notification' (Art 14(3)(c)). https://eur-lex.europa.eu/eli/reg/2024/2847/oj |
| NIST AI 100-1 (AI RMF 1.0) | AI Risk Management Framework | 2023-01 | verify | triaged | GOVERN/MAP/MEASURE/MANAGE | compliance map | cite | verified 2026-11-26: DOI resolves; four functions confirmed · https://doi.org/10.6028/NIST.AI.100-1 |
| NIST SP 800-53 Rev. 5 | Security and privacy controls | 2020 | verify | triaged | control catalogue | SOC 2 mapping | cite | verified 2026-11-26: CSRC page resolves · https://csrc.nist.gov/pubs/sp/800/53/r5/upd1/final |
| NIST SP 800-207 | Zero Trust Architecture | 2020 | verify | triaged | zero trust | per-principal authorization | cite | verified 2026-11-26: DOI and CSRC page resolve · https://doi.org/10.6028/NIST.SP.800-207 |
| NIST SP 800-63-4 | Digital Identity Guidelines | rev. 4 | verify | triaged | identity assurance | authentication surface | cite | verified 2026-11-26: CSRC page resolves · https://csrc.nist.gov/pubs/sp/800/63/4/final |
| SOC 2 (AICPA TSC) | Trust Services Criteria | 2017 | verify | triaged | trust-services criteria | self-assessment | cite | verified 2026-11-26: AICPA page resolves; no SOC 2 report is issued or implied · https://www.aicpa-cima.com/topic/audit-assurance/audit-and-assurance-greater-than-soc-2 |
| CISA SBOM guidance | Software Bill of Materials | 2026 | verify | triaged | SBOM practice | supply chain | cite | verified 2026-11-26: CISA SBOM page resolves; runtime closure (375 packages), not the dev tree (520) · https://www.cisa.gov/sbom |
| OWASP Top 10 for Agentic Applications (2026) | ASI01-ASI10 | 2026 | verify | triaged | agentic risks | threat model | cite | verified 2026-11-26: page resolves; ASI06 is memory/context poisoning · https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/ |
| OWASP Top 10 for LLM Applications | LLM risks (2025/2026) | 2025/2026 | verify | triaged | LLM risks | threat model | cite | verified 2026-11-26: landing page resolves; A-numbers are edition-scoped, so each matrix row must carry its edition · https://genai.owasp.org/ |
| OWASP Agent Control Standard (ACS) | Agent transparency and control | 2026-10-12 | verify | triaged | agent control | watch item | cite | verified 2026-11-26: landing page resolves; control set unread, no mapping claimed · https://genai.owasp.org/resource/agent-control-standard/ |
| Model Context Protocol spec 2026-07-28 | MCP specification | 2026-07-28 | verify | triaged | tool protocol | MCP bridge | cite | verified 2026-11-26: spec page resolves; tool descriptions are untrusted unless from a trusted server; extensions (Tasks, Skills, Apps) are opt-in · https://modelcontextprotocol.io/specification/2026-07-28 |
| Agent2Agent protocol (A2A) v1.0 | Agent-to-agent standard | v1.0 | verify | triaged | agent discovery/delegation | origin label | cite | verified 2026-11-26: protocol site resolves; Agent Cards and delegation, not a tool protocol · https://a2a-protocol.org/latest/ |
| RFC 9449 (DPoP) | OAuth 2.0 proof-of-possession | 2023-09 | verify | triaged | sender-constrained tokens | token rotation | cite | verified 2026-11-26: IETF page resolves; recorded alternative, not adopted (loopback deployment) · https://datatracker.ietf.org/doc/html/rfc9449 |
| OpenID Shared Signals and Events (SSE) | Security event streams / CAEP | draft | verify | triaged | asynchronous revocation | token revocation | cite | verified 2026-11-26: repository resolves; watch item, nothing consumes SETs · https://github.com/openid/sse |
| RFC 9106 (Argon2) | Argon2 memory-hard KDF | 2021-09 | verify | triaged | KDF | backup v3 | cite | verified 2026-11-26: IETF page resolves; Argon2id at m_cost=65536 · https://datatracker.ietf.org/doc/html/rfc9106 |
| NIST SP 800-38A / 800-38D | AES and GCM modes | 2001 / 2007 | verify | triaged | cipher modes | backup encryption | cite | verified 2026-11-26: CSRC pages resolve; fresh random nonce per artifact · https://csrc.nist.gov/pubs/sp/800/38/d/final |
| FIPS 203 / 204 / 205 | ML-KEM, ML-DSA, SLH-DSA | 2024-08 | verify | triaged | post-quantum algorithms | PQC inventory | cite | verified 2026-11-26: CSRC pages resolve; inventory only, no PQC deployed · https://csrc.nist.gov/pubs/fips/203/final |

## Evidence

### R7 reconciliation — measured 2026-11-26

**The sweep.** `python3 arxiv_watch.py --days 180 --max-per-lane 25 --md plans/research-window/scratch/sweep-180d-2026-11-26.md --rss plans/research-window/scratch/sweep-180d-2026-11-26.xml`, run 2026-11-26 from `/Users/mark/Sites/arxiv-watch`. Eight lanes, 25 per lane, no `--mark-seen`. Output: **169 papers in window (70 new)**. The seen cursor held **350 IDs** (199 dated 2026-10, 151 dated 2026-11) before the run.

| Quantity | Count | Where it comes from |
|---|---|---|
| swept | 169 | the sweep's own "169 papers in window" line |
| decided before this round | 19 | 3 adopted + 16 prior-note rows the sweep returned |
| register rows from the sweep | 96 | 169 − 73 exclusions |
| exclusions | 73 | §R2 exclusion list, one line each |
| adopted rows the per-lane cap did not return | 12 | 15 adopted − 3 the sweep returned |
| active rows | 108 | 96 + 12 |
| foundation rows | 24 | §2 |
| standards rows | 32 | §3 |
| total rows | 164 | 108 + 24 + 32 |

**The arithmetic.** `169 swept = 96 register rows ∩ sweep + 73 exclusions`. The 12 adopted-but-not-swept rows are extra register IDs; they do not enter the equation. `scripts/check-research-register.py --sweep` asserts both directions: `swept − registered − excluded = ∅` and `excluded − (swept − registered) = ∅`, over a non-empty register and a non-empty report.

**Decisions by outcome.** `build` 16, `cite` 136, `park` 1, `refuse` 7, `test` 4. A `build` is a plan item, not shipped code; `test` is an eval or probe; `cite` is a reference with no integration; `refuse` is a deliberate non-integration; `park` waits on a named trigger.

**Not recoverable.** The November sweep's exclusion count is not in the tree: doc 23 says "sixty-plus relevant papers", names 22 IDs, and never states the exclusion number. It is recorded as a gap, not guessed. This round's 73 are the first exclusion list written down with a reason per line.

### Refused this round — one reason each

| id | paper | reason |
|---|---|---|
| 2611.31656 | BRAG: Bayesian Retrieval Augmented Generation | A learned generative retrieval model; the RRF leg is the shipped law and a learned ranker is refused by name. |
| 2611.25583 | DriftGuard: Online Memory-Drift Detection via Causal Attribution for LLM Agents | Plan proof-hardening §8: drift_census is a census-breach recorder, not drift attribution; scope exclusion. |
| 2611.23588 | ARISE: Practical Self-Evolving Memory for Accurate and Cost-Efficient Enterprise Agents | Evolves memory from its own failures; the store keeps the human in the policy loop, so the autonomous loop is refused. |
| 2611.22245 | Multi-Agent Memory Inconsistency: Where Factual Conflicts Disappear in LLM Agents | Plan proof-hardening §8: matters only if delegations ship; parked behind that trigger. |
| 2611.22102 | EvoDisco: Strategy Discovery for Evolving Agent Memory | Evolutionary discovery of memory operators; a learned policy in the write or recall path is refused by the zero-token law. |
| 2611.21478 | Jetriever: Shifting Agent Memory Retrieval from Similarity to Utility | An LLM-derived System One scores every memory; a learned ranker in recall is refused, the utility question stays in eval. |
| 2611.20759 | Instant Weight-Space Consolidation for Long-Term Memory in LLMs | Consolidates into model weights; the model is not the store, refused as out-of-architecture. |

### Foundation gaps — named, not silently fixed

| item | gap | recorded as |
|---|---|---|
| LoCoMo | no harness in the tree; doc 13 names it and `BENCHMARKS.md:637` disclaims parity | §2, `build` |
| LongMemEval | no harness in the tree; same disclaimer | §2, `build` |
| Letta (self-editing memory) | no implementation, and no shipped claim depends on it | §2, `park` |
| Product quantization | the tree stores int8 + binary scalar quantization, not PQ codebooks; doc 08 says so in its own words | §2, `cite` |
| November sweep exclusions | the count was never written down in the prior round | this section |
| arxiv-watch | not under version control, so the R5 guard has no commit; the drill is the receipt | this section |

### Pin drills — pins 1–4 and 6–8

Run 2026-11-26 with `plans/research-window/scratch/drill_register.py` (12 checks, 12 passed). Each case copies the live register, applies one mutation, and asserts the checker's failure text; the untouched copy is the control.

| Pin | Mutation | Expected failure |
|---|---|---|
| 1 | first active row's reason set to `TBD` | `empty/placeholder reason` |
| 2 | a 2025 `adapt` row with no `exception:` | `adapt row dated 2025-01-01 with no exception:` |
| 3 | an RRF anchor replaced with "the engine" | `verify row mechanism has no tree anchor` |
| 4 | `verified 2026-11-26` stripped from a standards row | `no 'verified YYYY-MM-DD'` |
| 4b | the `https://` URL stripped from a standards row | `no https:// canonical URL` |
| 6 | spot check: adopted rows link to `plans/`, no phase table copied | 15 adopted rows, 15 name a plan; 0 phase rows |
| 7 | one exclusion line removed | `swept ids neither registered nor excluded` |
| 8 | the register reduced to its header | `anti-vacuity: the register has zero rows` |
| acceptance | the `1706.04599` row removed | `cited-work coverage` |

### R5 failure drill — a failed sweep does not advance seen-state

Run 2026-11-26 with `plans/research-window/scratch/R5-failsafe-drill.py` (9 checks, 9 passed). Three runs against the live tool:

| Run | Invocation | Result |
|---|---|---|
| A | forced lane failure (all eight `fetch` calls raise) + `--mark-seen` | exit 2, `state.json` byte-identical, no digest written |
| B | good sweep, no `--mark-seen` | exit 0, `state.json` byte-identical, digest written |
| C | good sweep + `--mark-seen`, throwaway `HERE` seeded with `{"seen": {}}` | exit 0, seen 0 → 48, real `state.json` untouched |

Run A is the pin: the previous code caught a lane exception, printed to stderr, and returned 0, so a scheduled `--mark-seen` would have advanced the cursor over a partial sweep. The guard makes any lane failure raise `SweepError`, `main` catch it, and return 2 before writing either output.

### R2 exclusion list — swept but not a register row

Each line is a swept paper with no decision to make: lane noise, off-topic, or outside the product's scope. The count reconciles in §R7: 169 swept = 96 register rows from the sweep + 73 exclusions (the other 12 register rows are the adopted papers the sweep's per-lane cap did not return).

- 2608.23601 — Stateful EDA flow tuning; lane noise.
- 2608.28361 — GRACE gradient coreset selection; model-side.
- 2609.00605 — Forget-set misalignment in LLM unlearning; model-side.
- 2609.06786 — Forget-retain interference in tabular data; model-side.
- 2609.10144 — Kernel-managed shared memory; OS layer, not the store.
- 2609.10439 — Layer-selective unlearning; model-side.
- 2609.11989 — Computational primitives of adaptation; theory, off-topic.
- 2609.16229 — Test-time unlearning via sparse autoencoders; model-side.
- 2609.20945 — Multilingual unlearning benchmark; model-side.
- 2609.25786 — Unlearning deprecated API knowledge; model-side.
- 2609.34251 — RADNPO unlearning method; model-side.
- 2609.37127 — LLM unbranding via unlearning; model-side.
- 2609.38723 — HPC systems for agents; infrastructure.
- 2609.38909 — Unlearning deceptive behaviours; model-side.
- 2609.38929 — Distributional unlearning in representation space; model-side.
- 2609.39279 — Dual-constrained erasure for safety alignment; model-side.
- 2610.02418 — Whiteout unlearning method; model-side.
- 2610.13717 — Federated k-means unlearning; model-side.
- 2610.30822 — SME guidance chatbot for the AI Act; off-scope.
- 2610.32238 — Autonomous cyber-weapons accountability; off-scope.
- 2610.32576 — Organizational privacy climate; off-scope.
- 2611.02561 — Ethical-evaluation framework for agents; off-scope.
- 2611.03955 — Statistical-independence unlearning method; model-side.
- 2611.04510 — Decision-model fairness evaluation; off-scope.
- 2611.06393 — Gender-performance perception study; off-topic.
- 2611.10028 — CSAM detection and legal implications; off-scope.
- 2611.10893 — Complementarity without ground truth; off-topic.
- 2611.11902 — Spectral unlearning method; model-side, not store-side erasure.
- 2611.13329 — Exchange-correlation functional discovery; lane noise.
- 2611.13696 — Private retrieval and inference for multi-agent systems; crypto/MPC, off-scope.
- 2611.15680 — Spatial reasoning over MCP; lane noise.
- 2611.16308 — CALPHAD phase-diagram assessment; lane noise.
- 2611.16312 — Robot-learning physics engine; lane noise.
- 2611.17381 — Production-readiness deployment tooling; lane noise.
- 2611.19123 — Tool-use cost comparison for MCP modes; not a memory claim.
- 2611.19417 — Revealed-preference alignment theory; off-topic.
- 2611.19439 — Uralic-language translation over MCP; lane noise.
- 2611.19670 — Corporate AI governance committees; off-scope.
- 2611.22761 — Configuration attacks on social robots; off-topic.
- 2611.23494 — 6G agentic networking; off-topic.
- 2611.25071 — Financial QA agent with MCP tools; lane noise.
- 2611.25985 — Legal theory for autonomous systems; off-topic.
- 2611.26435 — KV-cache compression during queue stalls; serving.
- 2611.26480 — Serverless session reservation measurement; infrastructure.
- 2611.26910 — Agent self-improvement benchmark; off-topic.
- 2611.26911 — Developmental brain atlases; lane noise.
- 2611.26912 — Self-evolving EDA scripting; lane noise.
- 2611.26980 — Runtime context eviction for serving; outside the store boundary.
- 2611.27462 — High-risk recruitment compliance; off-scope.
- 2611.27799 — EdTech child-safety policy assessment; off-scope.
- 2611.28148 — Multi-agent escrow markets; off-topic.
- 2611.28232 — Clinical-AI transparency methodology; off-scope.
- 2611.28356 — Memory for tuning heterogeneous models; model-tuning, not governance.
- 2611.29036 — Group cognition for collaborative agents; off-topic.
- 2611.29062 — Personalized agent harness generation; harness layer, not the store.
- 2611.29225 — Symbolic music creation in a DAW; lane noise.
- 2611.29255 — Scientific experiment guidance; off-topic.
- 2611.29333 — Embodied multi-party interaction platform; off-topic.
- 2611.29581 — Replicate independence in controlled-clone agents; off-topic.
- 2611.30165 — Multimodal training method; model-side, not store.
- 2611.30365 — Psychological-behaviour benchmark; off-topic.
- 2611.31082 — Circuit-rebuild benchmark; lane noise.
- 2611.31118 — Sandbox multiplexing for agents; serving infrastructure.
- 2611.31543 — Emergent social structure among LLM agents; off-topic.
- 2611.31547 — Android kernel exploit propagation; off-topic.
- 2611.31682 — Geospatial QA benchmark; lane noise.
- 2611.32255 — Agent sandbox forking; harness runtime, not memory.
- 2611.32545 — LLM serving scheduler; outside the store boundary.
- 2611.33093 — Hardware-design agent benchmark; lane noise.
- 2611.33926 — Discrete-element simulation tooling; lane noise.
- 2611.34215 — Training-time RL against collusion; not store governance.
- 2611.34254 — Chemistry research environment benchmark; lane noise.
- 2611.34907 — Video-generation memory; not a text-memory store surface.
