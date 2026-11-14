# cascade_bench trap summary — the DISCERN trap taxonomy (round 20)

determinism witness (T2 same query → same evidence): **true**
posture: deterministic /recall + the store's zero-token answer bound (LLM-free by law)

| pool | gold-abstain | gold-answer | TP | FP | FN | abstain P | abstain R | abstain F1 | false-decline | contamination gnd | contamination ans | conflict-surfaced | fabrication |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| attribute-swap | 3 | 3 | 0 | 0 | 3 | - | 0.000 | - | 0.000 | - | - | - | 0 |
| near-miss | 4 | 3 | 0 | 0 | 4 | - | 0.000 | - | 0.000 | 1.000 | 1.000 | - | 0 |
| contradiction | 0 | 3 | 0 | 0 | 0 | - | - | - | 0.000 | - | - | 0.000 | 0 |
| pooled | 7 | 9 | 0 | 0 | 7 | - | 0.000 | - | 0.000 | 1.000 | 1.000 | 0.000 | 0 |

**THE VERDICT: CONTAMINATION MANIFESTS** — near-miss contamination rate 1.000 (grounding) / 1.000 (answer) > 0 — the failure class manifests in this retrieval posture; the conditional mechanism (S3) opens

