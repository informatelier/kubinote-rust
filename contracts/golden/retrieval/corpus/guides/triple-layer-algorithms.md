# Triple-Layer Algorithm Guide

Architecture rationale and algorithm choices for the three-tier knowledge extraction pipeline.
Consult this before modifying search, indexing, or concept-extraction logic.

---

## Overview

KubiRAG processes documents through three complementary layers, each operating at a different
level of semantic depth and computational cost:

```
Layer 1 — Smile  (Statistical)   fast, corpus-wide, no LLM
Layer 2 — Lucene (Lexical)        fast, per-query, no LLM
Layer 3 — LLM    (Cognitive)      slow, on-demand, context-aware
```

The layers are not redundant — they answer different questions:

| Question | Layer |
|---|---|
| What topics does this corpus contain? | Smile |
| Which documents are relevant to this query? | Lucene |
| What do these documents mean in context? | LLM |

---

## Layer 1 — Smile: Statistical Cartographer

**Entry point:** `SmileMindmapService.generate()`
**Triggered by:** `/mindmap --smile` (CLI), `GET /mindmap` (web, default)

### What it does

Groups documents into K thematic clusters using unsupervised machine learning,
then extracts the most representative terms per cluster as the mindmap's L1/L2 concepts.

### Algorithm: TF-IDF + K-Means

**Step 1 — Tokenization (delegated to Lucene)**

Text is tokenized using Lucene's `ItalianAnalyzer` / `EnglishAnalyzer` with language
auto-detection. This is a deliberate choice: Smile's NLP module only provides an English
stemmer (Porter/Lancaster). Delegating to Lucene ensures that Italian inflections
("correndo", "corso", "corsa") collapse to the same stem ("corr-") — critical for
mixed IT/EN corpora.

**Step 2 — TF-IDF matrix**

Each document becomes a vector of TF-IDF weights:

```
tfidf(t, d) = tf(t, d) × log(N / df(t))
```

- `tf`: raw term frequency in document `d`
- `df`: number of documents containing term `t`
- `N`: total documents

Terms that appear in too few docs (noise) or too many (stopword residue) are filtered
out via `minDocFreq` / `maxDocFreqRatio` before building the matrix.

**Step 3 — K-Means clustering**

Smile's `KMeans.fit(tfidfMatrix, k, maxIter, tol)` partitions the document space.
`MathEx.setSeed(42L)` is called before each fit to guarantee deterministic results —
clusters change only when the corpus changes, not between identical requests.

**Why K-Means over LDA?**

Smile 3.x does not include Latent Dirichlet Allocation. Its `smile.classification.LDA`
is Linear Discriminant Analysis (a supervised classifier), not topic modeling.
K-Means on TF-IDF vectors is a valid alternative: cluster centroids encode term
importance directly, making L1/L2 concept extraction straightforward without
post-processing probability distributions.

**Step 4 — Concept extraction from centroids**

For each cluster, terms are ranked by their centroid weight (the cluster's average
TF-IDF value for that dimension). The top term becomes L1; the next `l2Count` terms
become L2 children.

**Sunburst weights**

- L1 `value` = `docsInCluster` — visual area proportional to cluster size
- L2 `value` = `centroidWeight × 100` — visual area proportional to term dominance

**Focus-term drill-down**

`--term <T>` stems T through the same Lucene analyzer, then filters documents using
bidirectional prefix match (`t.startsWith(stem) || stem.startsWith(t)`). K-Means
runs on the filtered subset, producing topic clusters *within* that theme rather
than across the full corpus.

---

## Layer 2 — Lucene: Lexical Archiver

**Entry point:** `RagLucene.searchTopDocuments()`
**Triggered by:** every user query (CLI and web)

### What it does

Retrieves the top-N document chunks most relevant to a query using a multi-stage
pipeline: BM25 scoring, optional hybrid fusion with vector similarity, optional LLM
re-ranking, and result diversification.

### Algorithm: BM25 + Hybrid Fusion

**BM25 (default, always active)**

Lucene's default similarity function. Scores documents by term overlap with the query,
penalizing very long documents to avoid length bias. Works well for exact and near-exact
term matches but misses synonyms and paraphrases.

**Query Expansion (`QueryExpansionService`)**

Before BM25 search, the query is expanded with domain-specific synonyms from a static
dictionary. This compensates for vocabulary mismatch between user queries and document
terminology without the latency of an LLM call.

**Phrase Query boost (`phraseQueryBoost = 1.5f`)**

Exact phrase matches are boosted over bag-of-words matches. Helps when users quote
specific terms or document titles.

**Hybrid Search (`HybridSearchService`, implemented but not connected)**

> ⚠ **Not active.** `HybridSearchService.fuseScores()` is implemented and unit-tested
> but is never called in `searchTopDocuments()`. The `hybridSearchEnabled` config flag
> is wired into `SearchOptimizationConfig` but the fusion code path is dead.
> All queries currently use BM25-only ranking. See CLAUDE.md for the known status.

The intended algorithm (for future activation):

```
score_hybrid(d) = luceneWeight × score_bm25_norm(d) +
                  vectorWeight × score_cosine_norm(d)
```

Planned weights: BM25 = 0.7, vector = 0.3. Both score sets would be min-max normalized
to [0,1] before fusion. Vectors are produced by Ollama's embedding model and stored in
`InMemoryVectorStore` (brute-force cosine similarity, no HNSW index).

**Result Diversification**

After ranking, chunks from the same parent document are deduplicated: only the
highest-scoring chunk per parent survives. Prevents a single long document from
monopolizing the context window.

**LLM Re-ranking (`LLMRerankingService`, optional)**

Top-N Lucene results are re-ordered by an LLM prompt that judges relevance against
the original question. Adds latency (~5s timeout, falls back to original order on
timeout). Useful when BM25 surface-matches the wrong document.

### Language-aware indexing

Two indexed fields per document:

- `content` — tokenized with `ItalianAnalyzer` (Snowball stemmer IT)
- `content_en` — tokenized with `EnglishAnalyzer` (Porter stemmer EN)

A `PerFieldAnalyzerWrapper` selects the correct analyzer at query time based on
language detection. This ensures Italian and English documents compete fairly in
the same index without cross-language stem collisions.

---

## Layer 3 — LLM: Cognitive Analyst

**Entry point:** `RAGSystem.queryLucene()` / `queryLuceneAsyncLLM()`
**Triggered by:** every user query (after Lucene retrieval)

### What it does

Synthesizes an answer from the retrieved document context, respecting the active
system role, conversation history, and pinned documents.

### RAG context construction

Context is built in priority order:

1. **Pinned documents** — always included, prefixed `PINNED`; user-curated,
   high-signal sources that survive query changes
2. **Retrieved documents** — top-N from Lucene, prefixed `RELEVANT <id>`;
   include `metaContent` metadata for provenance

The LLM receives: `context + conversation history (last 10 turns) + question`.

### Two pipeline variants

| Method | Used by | Differences |
|---|---|---|
| `queryLuceneAsyncLLM()` | CLI `/lucene` **and** web POST `/chat/query` | full pipeline + streaming via Kotlin `Flow`; used by all live endpoints |
| `queryLucene()` | — (currently unused) | identical pipeline but blocks until the full response is assembled; no active callers |

Both call `updateLuceneConfig(ragConfig)` before searching, ensuring runtime config
changes take effect without restart.

### Streaming

Responses stream chunk-by-chunk via `AsyncOllamaClient.generateStreamAsync()` using
Kotlin `Flow`. The CLI prints each chunk immediately; the web SSE endpoint forwards
chunks to the browser. Temperature is set low (0.3–0.7) to keep answers grounded in
the retrieved context.

### Ralph-loop (background mining)

The `ralphloop` package provides an autonomous task executor that runs LLM analysis
in the background over multiple iterations. Intended for relationship extraction
("Company X is linked to Supplier Y via Contract Z") that would be too slow for
interactive queries. Output is structured JSON that can enrich Lucene document
metadata in future iterations.

---

## Interaction between layers

```
User query
    │
    ├─▶ Lucene (Layer 2) ──────────────────────────────────────▶ top-N chunks
    │       ├─ BM25 search
    │       ├─ Hybrid fusion (+ vector cosine)
    │       └─ LLM re-rank (optional, Layer 3 micro-use)
    │
    └─▶ LLM (Layer 3) ◀── context (pinned + retrieved chunks)
            └─ streamed answer

Corpus change (new document added)
    │
    ├─▶ Lucene index updated (chunked, dual-language)
    ├─▶ Vector store updated (Ollama embedding)
    └─▶ Smile (Layer 1) re-runs on next /mindmap call
            └─ TF-IDF matrix rebuilt from scratch (stateless)
```

Smile is **stateless and lazy**: it reads `DocumentStore` on each call and rebuilds
the TF-IDF matrix from scratch. There is no incremental update. For large corpora
(> 5000 docs) this may become a bottleneck — a future optimization would be to
cache the matrix and invalidate on document add/remove.

---

## Configuration reference

Key `RagConfig` fields that govern all three layers:

| Field | Layer | Default | Effect |
|---|---|---|---|
| `hybridSearchEnabled` | Lucene | `true` | BM25 + vector fusion |
| `hybridSearchLuceneWeight` | Lucene | `0.7` | BM25 share in fusion |
| `hybridSearchVectorWeight` | Lucene | `0.3` | vector share in fusion |
| `reRankingEnabled` | LLM | `true` | LLM post-ranks Lucene results |
| `ragMaxDocs` | Lucene/LLM | `3` | docs sent to LLM context |
| `queryExpansionEnabled` | Lucene | `true` | synonym expansion before BM25 |
| `diversificationEnabled` | Lucene | `true` | one chunk per parent doc |
| `similarityTrashold` | Vector | `0.3` | min cosine score for vector results |
