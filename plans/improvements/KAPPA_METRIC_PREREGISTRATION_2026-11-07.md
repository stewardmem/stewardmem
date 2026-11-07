# κ METRIC PREREGISTRATION — the rule that will judge every future model deployment

**Decided:** 2026-11-07 (dated AT WRITING). **Measured state at decision:** the bench holds **0 tuples, 0 labels, 0 verdicts** (CG-06 S0; the pilot's H4 = 0 — no resolves, the triage KT wall gates the tuple flow). **`first_label_at`: null.**
**The proof this decision preceded the data is the git history of THIS repository:** this document's commit precedes every use of the labeling routes (`POST /workflow/kappa/labels`) — zero label rows exist today, and none may land before a commit that postdates this one.
**Status:** TERMINUS RULE. The conjunction below is the agreement rule for every future model deployment on this bench, decided at n=0 — the last moment it cannot be data-driven.

---

## 1 · The preregistered rule (the conjunction)

A labeled batch PASSES the human-agreement terminus rule if and only if ALL THREE hold:

1. **Cohen's κ ≥ 0.70** (7000 integer ten-thousandths — the recorded bar, `src/workflow/kappa.rs` `KAPPA_BAR_UNITS = 7000`, unchanged);
2. **Gwet's AC1 ≥ 0.80** (8000 integer ten-thousandths);
3. **the raw percent agreement is recorded** alongside them.

**Why a conjunction and not a swap:** Vach et al. (2023) — AC1 is NOT a substitute for κ; the two answer different questions. κ answers "how much agreement is there beyond chance, penalizing prevalence skew"; AC1 answers "how much agreement is there, robust to prevalence skew". Under the skewed prevalence this bench WILL have (abstain-vs-answer, resolve-vs-escalate — the pilot population was 19/19 escalated), κ alone is provably unreliable: the kappa paradox can collapse it toward 0 (even negative) at 90% raw agreement, flattering a bad rater pair OR damning a good one. AC1 alone is blind to exactly the prevalence structure κ sees. The conjunction is STRICTER than the prior single-metric reading of "κ ≥ 0.70 reported-as-data-only" — that is the point of deciding it at n=0, where it cannot be data-driven.

**The machine never auto-gates.** The bars are REPORTED-AS-DATA constants (`KAPPA_BAR_UNITS`, `AC1_BAR_UNITS`); the κ/AC1 values enter the machine's record only when a human types them into `POST /workflow/calibration/sign` — and the sign payload carries all three values together (no single-metric signing), so the audit chain can verify the conjunction was honored by a person, not assumed of one.

## 2 · The metrics, defined before any data

All three statistics are computed per report cell — a (domain × frozen partition × rater-slot-pair) group over the jointly labeled tuples, latest-wins — in integer ten-thousandths, by hand-rolled pure arithmetic (zero dependencies; statrs carries no agreement coefficients, checked via Context7 2026-11-07).

- **Raw percent agreement** — the observed-agreement proportion `Pa = agrees / n_joint`, ten-thousandths.
- **Cohen's κ** — `κ = (Pa − Pe) / (1 − Pe)`, `Pe = Σₖ pₐ(ₖ)·p_b(ₖ)` over the pooled label set (the promoted `cohen_kappa_units`, unchanged).
- **Gwet's AC1** — `AC1 = (Pa − Pe(γ)) / (1 − Pe(γ))` with **`Pe(γ) = (1/(q−1)) · Σₖ πₖ(1−πₖ)`**, where q = distinct labels in the pair's pooled ratings and πₖ = the average marginal proportion of label k. (Binary-case check: with positive-class proportion π, `Pe(γ) = 2π(1−π)` — the form quoted in the planning notes; the general q-category form above is the one implemented, and it is the one that reproduces the published fixture in §3. Degenerate inputs are named refusals in the κ fn's own shape: empty, length-mismatched, or a single category.)

**The prevalence flag (operationalized).** A cell's *positive-class prevalence* is the pooled prevalence of its MODAL label — the per-label proportion πₖ over both raters' labels on the joint set, maximized. (The bench vocabulary `agree | disagree | uncertain` names no designated positive class; the paradox is driven by whatever category dominates, by any name. The modal pooled πₖ is the dominance measure, and it is the same πₖ the AC1 formula computes.) **Law: when the modal prevalence is < 20% or > 80%, the cell is flagged `prevalence-skewed`, and the report displays BOTH κ and AC1 side by side with the paradox note.** The >80% arm is the live one for this bench (over q categories the modal share cannot fall below 1/q, so the <20% arm is latent until the vocabulary ever widens past six labels — a dated addendum). The flag is display data, like the bars: it marks the cells where the κ number means less than it looks.

**Why these thresholds:** 20%/80% brackets the region where the paradox literature demonstrates κ's collapse (Feinstein & Cicchetti 1990; Gwet 2008 — the demonstrations sit at 84–90% majority prevalence). Inside the band, κ behaves; outside it, the report says so next to the number.

## 3 · The fixture (published, not invented) — the implementation is pinned to it

The classic prevalence-paradox illustration from the Gwet literature: two raters, 100 items, both raters 90% "yes" marginals, contingency table **both-yes 82 · A-yes/B-no 8 · A-no/B-yes 8 · both-no 2** → raw agreement **84%**. Published roundings: **κ = 0.11, AC1 = 0.80**. Exact ten-thousandths the implementation must reproduce:

- `κ = (0.84 − 0.82) / (1 − 0.82) = (1/9)` → **1111**
- `π_yes = 0.90, π_no = 0.10; Pe(γ) = (0.9·0.1 + 0.1·0.9) / (2−1) = 0.18`
- `AC1 = (0.84 − 0.18) / (1 − 0.18) = 33/41` → **8049**

Any implementation whose AC1 on this table differs from 8049 (or whose κ differs from 1111) is wrong, whatever it claims. (Recording, because the planning notes carried a transcription seam: reading `Pe(γ) = 2γ(1−γ)` with γ = Σπₖ(1−πₖ)/(q−1) yields AC1 = 7730 and FAILS the published 0.80 — the two expressions' γ are different quantities; the standard general form above is the one that reproduces the published numbers, and the fixture is the arbiter.)

## 4 · The measurement protocol (the enrichment-set note)

Agreement is measured on the **disagreement-enriched sample as well as the uniform one**. arXiv:2609.30290 (citation #1 for this rule) is the failure this preregistration exists to prevent: a production LLM-as-judge audited only after deployment — *"its agreement with human annotators has never actually been measured"* — measured κ = 0.04 on a disagreement-enriched set vs 0.42 uniform-random, with **77.1% over-flagging** of human-faithful cases hiding behind the flattering uniform number. Consequence for this bench, decided now: when the labeling rounds begin, the report's per-cell numbers are read over the disagreement corpus the bench mines BY DESIGN (the disagreement rows are the tuples), and any future summary of "the bench's agreement" must state which sample it quotes. A uniform-random agreement number alone is the over-flagging camouflage.

## 5 · The further-change law (named acts only)

Any FURTHER change to the bar values, the metric set, the flag thresholds, or the conjunction's shape — after the first label lands — is a **named preregistered act on measured data**: a dated addendum to this document stating (a) the measured numbers that motivate it, (b) the new rule, (c) what the old rule would have said about the same data. The `MAX_REHYPOTHESES` pattern (`src/workflow/gdl.rs:75` — the bound named, the change a dated act, never a tidy). Silent drift of this rule is the one failure mode the preregistration cannot tolerate, because the rule's entire authority is that it was fixed while blind.

## 6 · Records

- This document: `plans/improvements/KAPPA_METRIC_PREREGISTRATION_2026-11-07.md`, committed before any labeling route use (the git order is the proof).
- The implementation pins: KM-1 (the §3 fixture, behavioral) · KM-2 (every cell emits {κ, AC1, raw, flag} together — no single-metric cell) · KM-3 (the flag fires on a 9:1 skewed fixture, silent on a balanced one) · KM-4 (the sign payload carries all three together; a single-metric sign refuses by name) · KM-5 (`gwet_ac1_units`' unit shape == `cohen_kappa_units`' — i32 ten-thousandths, same Result shape). Rows in `docs/gate-law-register.md`.
- Plan: `brain-steward-ip/plans/improvements/02_KAPPA_METRIC_PREREGISTRATION.md`; evidence: `IMPROVE02_EVIDENCE_<date>.md` (same repo).

## 7 · Citations

- Gwet, K.L. (2008), "Computing inter-rater reliability and its variance in the presence of high agreement", *British Journal of Mathematical and Statistical Psychology* 61:29–48 — AC1; the paradox illustration family. https://www.agreestat.com
- Vach, W. et al. (2023) — AC1 is not a substitute for κ; different questions.
- Feinstein, A.R. & Cicchetti, D.V. (1990), *J Clin Epidemiol* 43:543–549 — the paradox demonstrations.
- The 2026 NLP preregistration guidance: https://pmc.ncbi.nlm.nih.gov/articles/PMC10205778 · https://arxiv.org/html/2603.06865v2
- arXiv:2609.30290 — the unmeasured production judge (κ 0.04 enriched / 0.42 random; 77.1% over-flagging).
