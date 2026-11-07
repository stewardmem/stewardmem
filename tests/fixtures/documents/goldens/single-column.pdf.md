<!-- doc: 60764ccb6dcc930a26181babe10f0b691f081d1057b3d792b16536c0a190dc6f | source: golden | pages: 3 | tier: text-layer
     extracted: 2026-11-07T00:00:00+00:00 | extractor: pdf-extract/0.12.1 | layout: unverified -->

## Page 1


Local-First Memory for Agents

The kernel stores what an agent learns. Every write crosses the same
gate: the injection screen reads the text, the trust label bounds its
provenance, and the audit chain records the write without the bytes.

Recall is a fusion of three legs. The lexical leg matches exact terms
through a full-text index. The semantic leg embeds the query and ranks
vectors by cosine distance. The graph leg walks typed relationships so
a two-hop answer can cite the path it walked.

Nothing in the hot path calls a model provider. Embeddings are local,
deterministic, and zero-token; the ranking is a pure function of the
stored rows. A recall that cannot run offline is a recall that has
already failed its own posture.
<!-- anchor: p1 -->

## Page 2


The Write Gate

An agent write is a proposal until a human approves it. The proposal
queue is the only promotion path: no insert may bypass it, and the
behavioral pins prove the negative by counting rows after a write.

Provenance rides every row. The source label is a bounded vocabulary,
the owner stamp records the principal, and the origin field derives
from the label so an agent cannot forge human authorship.

Quarantine is the middle verdict. A flagged chunk is stored but never
retrieved by default: it carries no vector, so the ANN top-k cannot
surface it. The reviewer sees the verdict without a round trip.
<!-- anchor: p2 -->

## Page 3


Determinism as a Product Law

Same input bytes, same output bytes. The law applies to extraction,
to embedding, to ranking, and to every golden the tests pin. A path
that cannot be re-run into the same bytes is a path that cannot be
audited, and an unaudited path does not ship.

The fence is the advisory layer around untrusted context. Storage
stays verbatim; the strips compose at the boundary, and the sentinel
vocabulary has exactly one definition in the tree.
<!-- anchor: p3 -->
