<!-- doc: c1d8e2abadec7ccc75bafdd4d1d03d1ade2d65852ee047f72d8a431649ef237c | source: golden | pages: 1 | tier: text-layer
     extracted: 2026-11-07T00:00:00+00:00 | extractor: pdf-extract/0.12.1 | layout: unverified -->

## Page 1


Table: Retrieval Legs

leg          source            cost        notes
lexical      FTS5 index        O(terms)    exact + phrase matches
semantic     vec0 int8         O(k)        cosine over quantized rows
graph        typed edges       O(hops)     two-hop paths, labelled
fusion       RRF               O(legs)     reciprocal rank merge

Table: Lifecycle Verdicts
verdict      stored            retrieved   operator surface
clean        yes               yes         none
quarantine   yes               no          review queue
reject       no                no          refusal code
pending      proposal only     no          HITL gate
<!-- anchor: p1 -->
