# Tuning Guide

How to diagnose and improve retrieval quality using `/chunk`, `/stat`, and the full
`/config` parameter reference.

---

## Overview

KubiRAG has two diagnostic commands designed specifically for tuning:

| Command | Question it answers |
|---------|---------------------|
| `/chunk <file>` | "How will this document be split, and does my chunk size fit the context window?" |
| `/stat <query>` | "For this query, which documents were found, at what scores, and how did reranking change the order?" |

Use `/chunk` **before** loading a document to preview the chunking strategy.
Use `/stat` **after** a query gives a poor answer to diagnose which retrieval stage went wrong.

Both commands are **read-only** — they never write to the document store or Lucene index.

---

## /chunk — Chunking Preview

### Basic usage

```
/chunk README.md
/chunk docs/architecture.pdf
/chunk notes.md report.md          # multiple files at once
/chunk README.md --detail          # show 120-char previews instead of 50-char
```

### Comparing chunk configs without changing stored settings

```
/chunk README.md --chunk-size 512
/chunk README.md --chunk-size 512 --overlap 64
```

When `--chunk-size` or `--overlap` are provided, the command renders **two tables
per file** — one for the active config and one for the override — plus a delta line
showing chunk count and fill rate changes. The stored config is never modified.

### Reading the output

**Per-file table:**

```
 ── README.md ──────────────────────────────────────────────────
   Size: 8,420 chars  |  2,105 words  |  ≈2,105 LLM tokens

   Chunks: 4

   ┌───────┬────────┬────────┬──────────────────────────────────────┐
   │ chunk │  words │  chars │ preview                              │
   ├───────┼────────┼────────┼──────────────────────────────────────┤
   │  0/3  │    768 │  3,860 │ "# KubiRAG — A RAG system over…"    │
   │  1/3  │    768 │  3,812 │ "commands are registered in Ra…"     │
   │  2/3  │    768 │  3,790 │ "Layer 2 — Lucene: Lexical Arc…"     │
   │  3/3  │    321 │  1,620 │ "## Voice Voice uses Sherpa-ON…"     │
   └───────┴────────┴────────┴──────────────────────────────────────┘
   Unique words: 2,105  |  indexed: 2,625  |  duplicated by overlap: 520
```

- **words** — whitespace-tokenized word count; this is the unit `chunkSize` refers to
- **chars** — raw character count of the chunk
- **duplicated by overlap** — words counted twice or more across chunks due to the
  sliding window; overlap improves recall at the cost of index size

**Context Window Analysis:**

```
 ── Context Window Analysis ──────────────────────────────────
   chunkSize × ragMaxDocs = 768 × 3 = 2,304 words
   chatContextMaxTokens   = 3,000 tokens  (est. chars/4)
   RAG fill rate ≈ 76.8%  ⚠ dense — retrieval context dominates
```

> **Note on units:** `chunkSize` is in whitespace words; `chatContextMaxTokens` is in
> LLM tokens estimated as `chars / 4`. These differ by roughly 30% for typical text —
> the fill rate is an approximation, not an exact budget.

### Advisory thresholds

| Condition | Message | Suggested action |
|-----------|---------|-----------------|
| Fill rate > 70% | ⚠ dense | Reduce `chunkSize` or `ragMaxDocs` |
| Fill rate < 30% | ✓ lean | Can increase `chunkSize` for richer context |
| > 20 chunks per file | ⚠ Large doc | Consider PDF page-based import (`/load` with `pagesPerChunk`) |
| Last chunk < 50% of `chunkSize` | ⚠ Small tail | Document splits unevenly; try a smaller `chunkSize` |

### Workflow: tune before loading

```
# 1. Preview with default config (chunkSize=768, overlap=128)
/chunk large-report.md

# 2. Try an alternative
/chunk large-report.md --chunk-size 512 --overlap 64

# 3. If satisfied, change the stored config via web UI or JSON, then load
/load large-report.md
```

> **Chunk config changes do not automatically re-index loaded documents.**
> After changing `chunkSize` / `chunkOverlap`, use `/store`, then restart the server —
> the Lucene index is rebuilt from `kubirag-documents.json` on startup.

---

## /stat — Pipeline Statistics

### Basic usage

```
/stat what is the kubirag architecture
/stat kubernetes deployment strategy --explain    # adds Lucene BM25 explanation per doc
```

### Reading the output

**Level 1 — BM25 / Lucene:**

```
 [1/3] BM25 / Lucene  (12ms)  cache: 5 hits / 2 misses (71.4%)
   Query expansion: "architecture kubirag"
                 → "architecture architettura kubirag"
   Candidates: 6
   ┌──────┬───────────┬──────────────────────────────────────────────┐
   │ rank │   score   │ path                                         │
   ├──────┼───────────┼──────────────────────────────────────────────┤
   │  #1  │  3.4210   │ …docs/CLAUDE.md                              │
   │  #2  │  2.9870   │ …src/ragollama/RagOllama.kt                  │
   │  #3  │  2.1230   │ …src/lucene/RagLucene.kt                     │
   └──────┴───────────┴──────────────────────────────────────────────┘
```

- **score** — raw BM25 score (Lucene TF-IDF). Higher = stronger term overlap.
  Scores are not normalized; compare relative values, not absolute ones.
- **Query expansion** — terms added from the synonym dictionary before BM25 search.
  If irrelevant synonyms appear here, that explains spurious results.
- **cache** — `DirectoryReader` cache hits. Cold cache (first query after load) is normal.

**Level 2 — LLM Reranking:**

```
 [2/3] LLM Reranking  (1.2s)  enabled: true
   ┌──────┬──────────┬──────────────────────────────────────────────┐
   │ rank │ bm25-was │ path                                         │
   ├──────┼──────────┼──────────────────────────────────────────────┤
   │  #1  │    #1    │ CLAUDE.md             (=)                    │
   │  #2  │    #3    │ RagLucene.kt          (↑ +1)                 │
   │  #3  │    #2    │ RagOllama.kt          (↓ −1)                 │
   └──────┴──────────┴──────────────────────────────────────────────┘
```

- **bm25-was** — the document's rank before LLM reranking.
- **(↑/↓/=)** — promotion, demotion, or no change.
- If reranking consistently demotes the correct document, consider disabling it
  (`reRankingEnabled = false`) — it adds ~1–5s latency and can hurt when BM25 is
  already well-tuned.

**Level 3 — Context:**

```
 [3/3] Context
   maxDocs: 3  |  pinned: 0  |  selected: 3
   Estimated context: 4,280 chars
```

- **pinned** — documents always included regardless of retrieval rank (see `/pin`).
- **Estimated context chars** — the RAG portion of the LLM prompt, before conversation
  history is added. If this is large relative to `chatContextMaxTokens × 4`, answers
  may be cut off.

### Using `--explain`

```
/stat kubernetes pods --explain
```

Appends the Lucene BM25 explanation for each result — shows per-field term
frequency and IDF breakdown. Useful for diagnosing why an unexpected document ranked
high (e.g., accidental title match inflating the score via `titleBoost`).

---

## /config — Full Parameter Reference

Display the current config at any time:

```
/config
```

Parameters are grouped below by function.

---

### Model & Connection

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `modelName` | `qwen3:4b` | `/model <name>` | ✓ | LLM used for generation and reranking |
| `embeddingModelName` | `qwen3:4b` | `/embedder <name>` | ✓ | Model used for vector embeddings |
| `loopModel` | `""` | JSON | — | Model for background enrichment jobs; falls back to `modelName` if blank |
| `ollamaBaseUrl` | `http://localhost:11434/api` | `/ollama <url>` | ✓ | Ollama API endpoint |

```
/model llama3.2
/embedder nomic-embed-text
/ollama http://gpu-server:11434/api
```

> ⚠ **No automatic invalidation.** Changing `embeddingModelName` does **not** clear
> stored vectors — old embeddings from the previous model silently coexist with new
> query embeddings, producing garbage similarity scores.
> After changing the embedder: `/unload-all` to remove all documents, then re-load
> them with `/load` or `/load-all` so they are re-embedded with the new model.

---

### Retrieval

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `ragMaxDocs` | `3` | `/ragmaxdocs <n>` | ✓ | Max documents sent to LLM context |
| `similarityTrashold` | `0.3` | `/similarity <val>` | ✓ | Min cosine score for vector results (0.0–1.0) |
| `hybridSearchEnabled` | `true` | JSON | — | ⚠ Flag exists but fusion is not connected — has no effect |
| `hybridSearchLuceneWeight` | `0.7` | JSON | — | BM25 weight in hybrid fusion (future use) |
| `hybridSearchVectorWeight` | `0.3` | JSON | — | Vector weight in hybrid fusion (future use) |

```
/ragmaxdocs 5       # more docs = richer context but slower, fills more of the window
/similarity 0.5     # raise threshold if too many low-quality vector matches appear
```

> **Hybrid search is not active.** `HybridSearchService.fuseScores()` is implemented
> but never called — all queries use BM25 only. The `hybridSearchEnabled` flag and
> the weight fields are reserved for when fusion is connected.
>
> `hybridSearchLuceneWeight` + `hybridSearchVectorWeight` must sum to `1.0` once active.
> These fields require editing `.qb/qb-rag.json` directly (no CLI command or web UI).

---

### Chunking

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `chunkSize` | `768` | Web / JSON | ✓ | Chunk size in whitespace-tokenized words |
| `chunkOverlap` | `128` | Web / JSON | ✓ | Overlap between consecutive chunks (words) |
| `pdfDefaultPagesPerChunk` | `20` | prompt in `/load` | — | Default pages per chunk when importing PDFs |

The effective step between chunk starts is `chunkSize - chunkOverlap`.
Chunks smaller than `minChunkSize` (256 words, hardcoded) are discarded unless they
are the final chunk of a document.

> **How to change via JSON:**
> Edit `.qb/qb-rag.json`, set `"chunkSize": 512`, restart. The Lucene index is rebuilt
> automatically from `kubirag-documents.json`.
>
> **Use `/chunk` to preview before committing.** Re-indexing all documents takes time
> proportional to corpus size.

**Choosing chunk size:**

| Scenario | Recommended `chunkSize` |
|----------|------------------------|
| Short, focused documents (< 2 pages) | 256–512 |
| Standard technical docs / articles | 512–768 (default) |
| Large books or manuals | 1024–2048 (with higher overlap) |
| Queries that need broad context | larger chunks, fewer docs |
| Queries that need precise fact lookup | smaller chunks, more docs |

---

### Search Pipeline

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `queryExpansionEnabled` | `true` | Web / JSON | ✓ | Add synonyms to query before BM25 |
| `phraseQueryEnabled` | `true` | JSON | — | Boost exact phrase matches |
| `diversificationEnabled` | `true` | Web / JSON | ✓ | Keep only one chunk per parent document |
| `reRankingEnabled` | `true` | Web / JSON | ✓ | Re-order BM25 results via LLM |
| `titleBoost` | `2.0` | Web / JSON | ✓ | BM25 score multiplier for title field matches |

```
# Disable reranking for faster queries (saves 1–5s per query)
# → Web UI or set "reRankingEnabled": false in .qb/qb-rag.json

# Disable query expansion if synonyms cause spurious matches
# → Web UI or set "queryExpansionEnabled": false
```

> `phraseQueryEnabled` and `diversificationEnabled` affect recall significantly.
> Disabling diversification lets multiple chunks from the same document appear in results —
> useful for long documents where you want contiguous context, but risks monopolising the
> context window with one source.

---

### Context Window

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `chatContextMaxTokens` | `3000` | Web / JSON | ✓ | Token budget for conversation history (est. chars/4) |
| `chatContextWindowSize` | `20` | Web / JSON | ✓ | Max conversation turns kept before token trimming |

The conversation history is trimmed from the front (oldest turns removed first) when
`sumOf { message.length } / 4 > chatContextMaxTokens`.

> If the RAG context (`chunkSize × ragMaxDocs`) is already close to
> `chatContextMaxTokens`, there is little room for conversation history.
> Use `/chunk` to check the fill rate.

---

### Generation

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `thinkingEnabled` | `true` | `/thinking on\|off` | — | Enable extended reasoning (thinking tokens) |
| `streamMaxChars` | `64_000` | JSON | — | Circuit breaker — stops streaming after N chars |
| `repeatPenalty` | `1.2` | JSON | — | Ollama repeat penalty; higher = less repetition |
| `structuredOutput` | `false` | JSON | ✓ | Request JSON-formatted output from the LLM |

```
/thinking off    # faster responses; useful if model doesn't benefit from thinking
```

> `streamMaxChars` and `repeatPenalty` require editing `.qb/qb-rag.json`.
> `repeatPenalty = 1.0` disables the penalty entirely; values above `1.5` can make
> the model overly terse.

---

### Debug & Advanced

| Parameter | Default | CLI | Web | Description |
|-----------|---------|-----|-----|-------------|
| `debug` | `false` | `/debug true` | — | Print internal pipeline details to terminal |
| `maxAnalysisTokens` | `500` | JSON | — | Token limit for role-analysis output |
| `saveAnalysisResult` | `true` | JSON | — | Persist role-analysis results to disk |

```
/debug true    # shows BM25 scores, reranking order, and context size inline during queries
/debug false   # return to clean output
```

> `/debug true` surfaces the same information as `/stat` but inline with every query.
> Use `/stat` for a structured one-shot view; use `/debug` to monitor live behaviour
> across multiple queries.

---

## Common Tuning Scenarios

### "The answer is generic and doesn't reference my documents"

1. Run `/stat <your query>` — check Level 1. Are the right documents appearing?
2. If **no relevant documents in BM25**: lower `similarityTrashold` (`/similarity 0.2`),
   check query expansion is on, try `/ragmaxdocs 5`.
3. If **right documents retrieved but wrong answer**: check Level 3 — is `estimatedContextChars`
   very large? The LLM may be overloaded. Reduce `ragMaxDocs` or `chunkSize`.
4. If **wrong documents in BM25**: check if query expansion is adding misleading synonyms
   (visible in Level 1 output). Disable with `queryExpansionEnabled = false`.

---

### "Retrieval is too slow"

The main latency contributors:

| Step | Typical cost | How to disable |
|------|-------------|----------------|
| LLM reranking | 1–5s | `reRankingEnabled = false` |
| Thinking mode | 2–10s extra | `/thinking off` |
| Vector embedding | 0.5–2s | n/a — hybrid fusion not yet connected; vectors embedded at index time only |

Start by disabling reranking — it has the largest single impact.

---

### "Context window fills up, leaving no room for conversation"

1. `/chunk <any loaded file>` — read the **Context Window Analysis** fill rate.
2. If fill rate > 70%:
   - Reduce `/ragmaxdocs 2` (immediate, no re-index needed)
   - Or reduce `chunkSize` to 512 (requires re-index)

---

### "A very long document is poorly covered"

1. `/chunk <file>` — look for > 20 chunks or a very small last chunk.
2. If > 20 chunks: each query can only retrieve `ragMaxDocs` of them. Increase
   `/ragmaxdocs` or consider splitting the document into logical sections.
3. For PDFs: use `/load` with a smaller `pagesPerChunk` (e.g. 10 instead of 20) to
   create more granular page-range chunks.

---

### "Reranking keeps demoting the correct document"

Run `/stat <query>` and compare Level 1 (BM25 rank) vs Level 2 (reranked rank) for the
document you expect to appear first. If reranking consistently hurts:

- Disable reranking: `reRankingEnabled = false`
- Or try a different `modelName` — reranking quality depends heavily on the model.

---

## Config persistence

All `/xxx` CLI commands that change config call `ragConfig.storeConfig()` automatically.
Settings are saved to `.qb/qb-rag.json` and restored on the next startup.

Parameters marked **JSON** must be edited in `.qb/qb-rag.json` by hand:

```bash
# Example: change chunkSize to 512 and disable hybrid search weights tuning
cat .qb/qb-rag.json | python3 -m json.tool   # view current config
# Edit the file, then restart with: make cli
```

After editing chunk parameters, the Lucene index is rebuilt automatically on restart
from `kubirag-documents.json` — no manual migration needed.
