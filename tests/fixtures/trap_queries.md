# Trap Query Fixture (round 20 — the DISCERN trap taxonomy against this store's shape)

This file is the **trap probe set** for the round-20 DISCERN adoption round:
the entity-trap taxonomy (attribute-swap, near-miss-entity) plus the
contradiction-at-recall pair DISCERN's own benchmark cannot test, encoded
against a synthetic seeded store. Sources: DISCERN (arXiv:2611.15729) via
research note 25 and blog 29; the plan is `20_DISCERN_ADOPTION_ROUND.md` in
the plans repository. Changes require a new file hash recorded in the S0
commit message and `IMPROVE20_EVIDENCE_2026-11-14.md`.

## THE GOLD-DERIVATION LAW (preregistered; machine-checked by `parse_trap_fixtures`)

Every gold answer below is DERIVED from the corpus table in THIS file — the
same table the trap mode ingests verbatim through `/add` before measuring —
never hand-written to match output. The parser asserts, per query kind:

- **attribute-swap** (gold = abstain): the `Entity` span appears in ≥1 corpus
  doc (the entity is real); NO doc containing the entity span contains the
  `AttrKey` span (the attribute does not hold on it); and the `AttrKey` span
  appears in ≥1 OTHER corpus doc (the attribute exists in-store on a
  different entity — the plausible-junk-hit posture; a trap whose attribute
  exists nowhere would test nothing).
- **attribute-swap-control** (gold = answer): the `Relevant` doc contains the
  entity span (the attribute holds, the store can answer).
- **near-miss** (gold = clean abstain): the `NeighborSpan` appears in ZERO
  corpus docs (the neighbor was never stored — derived, not asserted); the
  `TwinSpan` appears in ≥1 doc; and the twin span shares ≥2 normalized tokens
  with the neighbor span (the confusability that makes the trap a near-MISS).
- **near-miss-control** (gold = answer): the `Relevant` doc contains the real
  (stored) entity's span.
- **contradiction** (gold = a defined resolution surfaced): `Relevant` is
  exactly two docs; BOTH contain the `Entity` span and the `AttrKey` span;
  the two docs' normalized texts differ (a conflict, not a duplicate). Both
  docs are seeded by independent `/add` calls with no revise and no validity
  overlay — both live, the DISCERN-untestable case.
- **contradiction-control** (gold = answer): the `Relevant` doc contains the
  entity span AND the `AttrKey` span, and EXACTLY ONE corpus doc contains
  both (the attribute is single-source — no conflicting doc exists).

A derivation failure is a fixture bug (falsifier F3): fix the fixture
derivation, never the gold.

## THE MEASURED POSTURE

The trap arm is LLM-free by law (round 20's no-LLM-surface rule): it runs
deterministic `/recall` (T2, zero tokens), then the store's own zero-token
answer bound — hits empty → the bench's preregistered abstain line; hits
non-empty → the top hit's content verbatim (the evidence-as-answer bound the
S0 arms already preregister). This measures the store's retrieval + commit
bounds; writer-tier trap behavior is a later round. The near-miss contamination
detector: the twin's marked span in the GROUNDING (any hit) or the ANSWER for
a query about a neighbor that was never stored. The neighbor's own marked span
must appear NOWHERE (the anti-fabrication witness — it cannot, it was never
seeded).

## Corpus (the seeded trap store — 12 docs, ingested verbatim in index order)

| 0 | Zephyron Cloud Solutions reported annual revenue of 5.1 billion dollars for fiscal 2025. |
| 1 | Zephyron Cloud Solutions is headquartered in Seattle. |
| 2 | Zephyron Cloud Solutions was founded in 2014. |
| 3 | Northwind Analytics is headquartered in Rotterdam. |
| 4 | Northwind Analytics was founded in 2019. |
| 5 | Northwind Analytics reported annual revenue of 42 million dollars. |
| 6 | Northwind Analytics reported annual revenue of 87 million dollars. |
| 7 | Meridian Robotics is headquartered in Osaka. |
| 8 | The CEO of Meridian Robotics is Lena Hartmann. |
| 9 | Meridian Robotics builds assembly robots for automotive factories. |
| 10 | Halcyon Foods is headquartered in Lisbon. |
| 11 | Halcyon Foods sells packaged soups under the Golden Ladle brand. |

Contradiction pair: docs 5 and 6 (Northwind Analytics revenue — two
independently-written conflicting facts, no revise, both live). The Zephyron
Solutions/Systems pair reuses DISCERN's own worked example deliberately: two
real distinct entities sharing every name token but one.

## Trap queries (16; asserted counts: attribute-swap 3, attribute-swap-control 3, near-miss 4, near-miss-control 3, contradiction 2, contradiction-control 1)

### T1 — [attribute-swap]
Entity: "Zephyron Cloud Solutions"
AttrKey: "CEO"
Query: "Who is the CEO of Zephyron Cloud Solutions?"
Relevant: []

### T2 — [attribute-swap-control]
Entity: "Zephyron Cloud Solutions"
Query: "What is the revenue of Zephyron Cloud Solutions?"
Relevant: [0]

### T3 — [near-miss]
NeighborSpan: "Zephyron Cloud Systems"
TwinSpan: "Zephyron Cloud Solutions"
Query: "What is the revenue of Zephyron Cloud Systems?"
Relevant: []

### T4 — [near-miss-control]
Entity: "Zephyron Cloud Solutions"
Query: "Where is Zephyron Cloud Solutions headquartered?"
Relevant: [1]

### T5 — [contradiction]
Entity: "Northwind Analytics"
AttrKey: "revenue"
Query: "What is the annual revenue of Northwind Analytics?"
Relevant: [5, 6]

### T6 — [contradiction-control]
Entity: "Meridian Robotics"
AttrKey: "build"
Query: "What does Meridian Robotics build?"
Relevant: [9]

### T7 — [attribute-swap]
Entity: "Halcyon Foods"
AttrKey: "CEO"
Query: "Who is the CEO of Halcyon Foods?"
Relevant: []

### T8 — [attribute-swap-control]
Entity: "Meridian Robotics"
Query: "Who is the CEO of Meridian Robotics?"
Relevant: [8]

### T9 — [near-miss]
NeighborSpan: "Zephyron Cloud Systems"
TwinSpan: "Zephyron Cloud Solutions"
Query: "Where is Zephyron Cloud Systems headquartered?"
Relevant: []

### T10 — [near-miss-control]
Entity: "Northwind Analytics"
Query: "When was Northwind Analytics founded?"
Relevant: [4]

### T11 — [attribute-swap]
Entity: "Zephyron Cloud Solutions"
AttrKey: "sell"
Query: "What product does Zephyron Cloud Solutions sell?"
Relevant: []

### T12 — [attribute-swap-control]
Entity: "Halcyon Foods"
Query: "What does Halcyon Foods sell?"
Relevant: [11]

### T13 — [near-miss]
NeighborSpan: "Northwind Analytics Group"
TwinSpan: "Northwind Analytics"
Query: "Who is the CEO of Northwind Analytics Group?"
Relevant: []

### T14 — [near-miss-control]
Entity: "Meridian Robotics"
Query: "Where is Meridian Robotics headquartered?"
Relevant: [7]

### T15 — [contradiction]
Entity: "Northwind Analytics"
AttrKey: "revenue"
Query: "How much revenue does Northwind Analytics report?"
Relevant: [5, 6]

### T16 — [near-miss]
NeighborSpan: "Meridian Robotics Limited"
TwinSpan: "Meridian Robotics"
Query: "Who is the CEO of Meridian Robotics Limited?"
Relevant: []
