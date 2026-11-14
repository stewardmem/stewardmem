# cascade_bench trap summary — the DISCERN trap taxonomy (round 20)

determinism witness (T2 same query → same evidence): **true**
posture: deterministic /recall + the store's zero-token answer bound (LLM-free by law)

| pool | gold-abstain | gold-answer | TP | FP | FN | abstain P | abstain R | abstain F1 | false-decline | contamination gnd | contamination ans | conflict-surfaced | fabrication |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| attribute-swap | 3 | 3 | 0 | 0 | 3 | - | 0.000 | - | 0.000 | - | - | - | 0 |
| near-miss | 4 | 3 | 4 | 0 | 0 | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 | 0.000 | - | 0 |
| contradiction | 0 | 3 | 0 | 0 | 0 | - | - | - | 0.000 | - | - | 0.000 | 0 |
| pooled | 7 | 9 | 4 | 0 | 3 | 1.000 | 0.571 | 0.727 | 0.000 | 0.000 | 0.000 | 0.000 | 0 |

**THE VERDICT: MEASURED-ZERO** — near-miss contamination rate 0/4 — the collision-family mechanism parks BY NAME (falsifier F1): the failure class does not manifest in this retrieval posture

