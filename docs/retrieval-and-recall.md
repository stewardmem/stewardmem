# Retrieval & Recall

This page explains **how StewardMem finds the right memory** — the retrieval pipeline, the fusion algorithm, query expansion, and how it stays honest when it doesn't know the answer. No LLM decides here; everything is deterministic and inspectable.

## The retrieval pipeline

Recall is **hybrid with a graph leg**: the vector and lexical legs run concurrently, the graph leg runs with them by default, and all three are merged.

```
      query
        │
        ├────▶ Vector leg (vec0 KNN over quantized embeddings)
        │
        ├────▶ Lexical leg (FTS5 / BM25)
        │
         └────▶ Graph leg (Personalized PageRank, default-on; disable with graph=false / BRAIN_RECALL_GRAPH_ENABLED=false)
                     │
                     ▼
            Reciprocal Rank Fusion (RRF, k=60)
                     │
                     ▼
            [optional] Cross-encoder rerank
            (enterprise / desktop / quality-local:
             mxbai-rerank-large-v1, fallback bge-reranker-v2-m3)
                     │
                     ▼
              rank + provenance
```

The optional rerank stage is **off by default** (the edge profile stays rerank-free,
the v0.9.5 doctrine) and is **fail-open** — a reranker fault leaves the RRF order
untouched. See [Configuration](./configuration.md) for the profile matrix and
`BRAIN_RERANK_*` variables.

### 1. The vector leg

Embeddings are computed in-process — by the static `model2vec` model (default
profile: no transformer forward pass, just token lookup) or, on the opt-in
`enterprise` / `desktop` profiles, by local transformer embeddings (`BAAI/bge-m3`
1024-d / `gte-base-en-v1.5` 768-d) through the same pipeline. Vectors are stored
in a SQLite `vec0` table, int8/binary quantized (4–32× smaller) so the whole
index stays small on edge hardware. KNN (k-nearest-neighbors) finds the closest
vectors to the query embedding.

### 2. The lexical leg

The same text is indexed in SQLite FTS5 and scored with BM25 — the classic term-frequency/documents-frequency ranking. This catches exact terms, code identifiers, and phrases that a vector search might miss.

### 3. Fusion with Reciprocal Rank Fusion

Rather than trusting a single score, RRF merges the two ranked lists by **rank position**:

```
score(result) = Σ over each leg of  1 / (k + rank_in_that_leg)   where k = 60
```

This is deterministic and needs no learned weights. A result ranked #1 in both legs gets the highest fused score.

### 4. Graph leg (default-on, v1.11+)

The graph leg runs **Personalized PageRank** over the knowledge graph by default — the deterministic version of the HippoRAG-2 retrieval approach. It seeds from entities matched to the query and spreads probability mass over connected entities, then expands to the chunks those entities touch. It's fused into the same RRF merge as a third leg (vector + FTS + graph), so connected knowledge surfaces without opting in — a single multi-hop walk links related domains (e.g. VMware↔VxRail↔vSAN↔storage↔fabric). Callers may pass `graph=false` per-request; the process-wide kill switch is `BRAIN_RECALL_GRAPH_ENABLED=false`. The leg applies the same tenant/owner/scope predicates as the vector and FTS legs (domain label, access_scope, owner, PII flag carried on the hit), so enabling it never widens what a principal can read. In v1.12, this leg is noise-aware: taxonomy edges (`tagged_with`) weigh 0.1 and mega-hubs are dampened, so real semantic connections win.

### 5. PRF query expansion

PRF (pseudo-relevance feedback) expands the query with related terms — but only when the top result appears in **both** dense and lexical lists within a bounded rank. This cross-retriever agreement gate means expansion fires on genuine signal, never on a single fused score. It never injects content from quarantined rows.

## Structured query (`QueryDoc`)

`POST /recall` takes a structured query document (the request struct in
`src/handlers/recall.rs` is `RecallRequest`; there is no `q`/`k` alias — a
body with those keys fails deserialization):

```json
{
  "query": "blueberry alternative",
  "limit": 5,
  "sources": ["memory", "vault"],
  "provenance": true,
  "graph": false
}
```

- **Lexical control** — a `LexSpec` with terms, quoted phrases, exclusions (`-"..."`), and exact code paths.
- **Filters** — `source`/`sources` (ingest kind: `memory` · `markdown` · `structured` · `manual` · `vault`), `since` (ISO timestamp), `domain`, `min_relevance`, `include_decayed`.
- **Provenance** — per-retriever ranks, fused score, expansion terms.
- **Context packing** — `max_context_tokens` re-ranks the hit set by budgeted
  monotone **submodular maximization** (`src/search/packing.rs`): evidence is
  packed to maximize coverage under the caller's token budget rather than
  truncated by score order.

## Provenance

Every result carries `provenance`: per-retriever ranks, the fused score, and any expansion terms. The recall decision-path viewer shows *why* each chunk was chosen — the per-retriever ranks, fused score, relevance tier, and source. Since v1.27.12 each hit additionally carries its stored **provenance tags** — `source`, `node_kind`, `lawful_basis`, `region` — which the OpenClaw plugin renders as a `[src: · mk: · lb: · reg:]` line inside the untrusted-data fence, so the model can attribute (not just trust) each recalled item.

## Abstention: knowing when you don't know

When retrieval quality is too low to support a claim, `/recall` returns:

```json
{ "decision": "low_confidence", "hits": [] }
```

Instead of returning top-1 garbage. This is driven by a **calibrated multi-signal recommendation** (rank overlap, gap, lexical density) — never a magic `score < 0.3` cutoff. In v1.12, the graph leg can auto-engage as a "rescue pass" when the estimator says the query is ambiguous, before the server abstains.

## Span verification (v1.5)

`POST /verify` checks whether a claim is literally supported by a chunk's text — a deterministic, case-insensitive substring match over one chunk. It returns `{supported, decision, match_ranges}`. No embeddings, no LLM, no model load. This is the "show your work" endpoint.

## Decay & relevance (v1.14)

Chunks can carry `expires_at` (strict decay, default-excludes) and `min_relevance` tiers. Decayed chunks are excluded by default and surfaced via `GET /decayed` for operator review — nothing decays autonomously.

## Code index (C0)

A separate, per-repo **code index** now exists as substrate: `brain code-index ingest <repo-root>`
walks the tracked corpus, parses Rust and TypeScript with tree-sitter into scope-qualified
symbols, chunks by symbol (windows only where a file has no symbol, and tagged), and fills three
lexical tables plus a vector table. `brain code-index status <repo-root>` reports what the index
holds — counts, a per-extension coverage table that names what is counted but never parsed, and
`fresh` or `stale` decided by the commit alone. Incremental: a re-ingest reparses exactly the
files whose content hash moved (never mtime), and a same-commit re-ingest writes zero data rows.

**Nothing consumes it yet.** No route, no retrieval surface, no token claim — the code index is
not part of `/recall` or any other pipeline in this page. What exists is the baseline: a frozen
46-question set (SHA-256-pinned in the crate; a changed set fails the build) run through three
lexical control arms — file-level BM25, chunk+symbol lexical, and the two fused by reciprocal
rank. `brain code-index recall <repo-root>` prints recall@1/5/10 (file level), recall@10 (symbol
level), each split by whether the question carries a code identifier, with the set's hash beside
the table. The measured floor at C0 (2026-11-27, brain-server at `1919ca34`: 827 files, 20,209 symbols,
71,604 chunks): **file-level recall@5 = 0.652 overall, 0.800 for identifier questions, 0.611
without**. No vector number may ever be quoted without this table beside it — that is the round's
preregistered claim, and the vector layer (C1) must clear 0.70 at recall@5 or the plan's
stop-and-reassess clause fires.

The index lives in one file per repo (`<root>/.brain-code-index.db`, `--index` overrides) —
rebuildable from a commit, never in the DSAR, legal-hold, or retention surface. The crate that
owns it (`brain-code-core`) loads no model and makes no network call; the embedding profile's
model is supplied by the kernel through a port, so the same index builds under any profile
(C0's measurements use the default `local` profile, 512-d).

## Next steps

- **[Knowledge Graph](./knowledge-graph.md)** — the graph layer that powers the third recall leg (default-on).
- **[Architecture](./architecture.md)** — where retrieval sits in the whole system.
- **[API Reference](./api.md)** — the exact request/response contract.
