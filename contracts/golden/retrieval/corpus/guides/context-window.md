# Context Window — Tuning & Monitoring

How KubiRAG manages the LLM context window: the model's real `num_ctx`, the app-side
budgets that decide how much conversation history is sent, and the commands / web display
used to set and monitor them.

For the broader retrieval-tuning workflow (`/chunk`, `/stat`, full `/config` table) see
[tuning-guide.md](tuning-guide.md). This guide zooms in on the context dimension.

---

## Mental model: three layers, don't confuse them

The word "context" maps to **three distinct knobs**. Only one of them is the model's real
context length; the other two are local app-side budgets that shape the prompt *before* it
reaches Ollama.

| Layer | Parameter | Unit | What it controls | Where it acts |
|-------|-----------|------|------------------|---------------|
| **Model** | `contextWindow` | real tokens | Ollama `num_ctx` — the model's context length | Sent on every generate/chat request |
| **App — count** | `chatContextWindowSize` | messages | How many recent messages enter the prompt | CLI **and** web |
| **App — budget** | `chatContextMaxTokens` | est. tokens (chars/4) | Token ceiling that trims old messages | CLI `/ask` (active); elsewhere a reference budget |

> **Key point:** `chatContextMaxTokens` and `chatContextWindowSize` are **not** the model's
> context window. They are heuristic, app-side limits (token estimate = characters / 4).
> The real model context window is `contextWindow` (→ `num_ctx`).

Defaults: `contextWindow = 4096`, `chatContextWindowSize = 20`, `chatContextMaxTokens = 3000`.

---

## The parameters in detail

### `contextWindow` — Ollama `num_ctx` (the real context window)

The maximum number of tokens the model will consider (prompt + generated answer). KubiRAG
injects it into **every** Ollama request via `RagConfig.ollamaOptions()`:

```kotlin
if (!m.containsKey("num_ctx")) m["num_ctx"] = contextWindow
```

Because the merge rule lets a caller's own option win, a specific call can still override
`num_ctx`; otherwise the configured value is used. Before this existed, KubiRAG never set
`num_ctx` and the model silently used its built-in default.

- **Ceiling:** can't usefully exceed the model's own maximum (see `/models` and `/show info`).
- **Cost:** a larger `num_ctx` raises RAM/VRAM use and can slow prompt evaluation. Set it to
  what you actually need, not the model's max.

### `chatContextWindowSize` — how many messages

A simple `takeLast(N)` over the session's messages (user **and** assistant messages each
count — it is messages, not turns):

```kotlin
fun getMessagesWindowed(windowSize) =
    if (windowSize <= 0) all else all.takeLast(windowSize)
```

Applied everywhere history is built: CLI `/ask` & `/talk`, the web chat (`CtrlQuery`), and
the prompt assembly in `RAGSystem` — where each kept message is **also truncated to 500
characters**. With `20`, only the last 20 messages survive; older ones are dropped.

### `chatContextMaxTokens` — the token budget

A token ceiling (estimated as chars/4) that drops the **oldest** messages until the history
fits:

```kotlin
while (mutable.isNotEmpty() && mutable.sumOf { it.length } / 4 > maxTokens) {
    mutable.removeFirst()   // drop oldest
}
```

### How the two combine (order matters)

In the **CLI `/ask`** path both run in sequence:

```kotlin
val windowed = getMessagesWindowed(chatContextWindowSize)                 // 1) keep last N
val msgs     = getMessagesWithinTokenBudget(windowed, chatContextMaxTokens) // 2) trim to budget
```

First filter by **count**, then by **token budget**. So `chatContextMaxTokens` can further
shrink what `chatContextWindowSize` already selected.

> **Asymmetry to be aware of:** the token-budget trim is currently active **only in the CLI
> `/ask` path**. In the **web** path history is limited only by `chatContextWindowSize` plus
> the 500-char-per-message truncation; `chatContextMaxTokens` does **not** trim the web chat
> history. Elsewhere `chatContextMaxTokens` serves as a *reference budget*: the `/chunk` RAG
> fill-rate, role-analysis truncation, and the "% of budget" figure in the occupancy display.

---

## What "occupied" means

The occupancy estimate (CLI `/context`, web badge, `GET /context/usage`) is the **persistent
baseline** re-sent on every turn:

```
occupied ≈ estimateTokens(systemPrompt)
         + estimateTokens(pinned docs)      // "PINNED <id>: <metaContent> <content>"
         + estimateTokens(windowed history) // last N messages, each ≤ 500 chars
```

with `estimateTokens(text) = text.length / 4`.

Retrieved RAG chunks are **not** in this baseline — they are added per query and bounded by
`chunkSize × ragMaxDocs`, reported separately as the **RAG allowance**. So the true prompt at
query time is roughly `occupied + (up to) ragAllowance + the question`.

> All figures are heuristic (chars/4). For the exact prompt token count, Ollama returns
> `prompt_eval_count` in its response — not yet surfaced in the UI, a candidate enhancement.

---

## Tuning — how to set each

| Parameter | CLI | Web Settings | `.qb/qb-rag.json` | Range |
|-----------|-----|--------------|-------------------|-------|
| `contextWindow` (num_ctx) | `/context <tokens>` | "Context window (num_ctx, tokens)" | yes | 256 – 1,000,000 |
| `chatContextWindowSize` | — *(no CLI setter)* | "Context window (messages)" | yes | ≥ 1 |
| `chatContextMaxTokens` | — *(no CLI setter)* | "Max context tokens" | yes | ≥ 100 |

> There is **no dedicated CLI command** to set the two app-side budgets — change them from the
> web Settings panel (which `PUT /config` + persists) or by editing `.qb/qb-rag.json`. Only
> `contextWindow` has a CLI setter (`/context`). All three persist across restarts.

### Recommendations

- **Match `num_ctx` to the model and the job.** Long RAG contexts or many pinned docs need a
  larger `num_ctx`; short chats don't. Check the model's max with `/models` before raising it.
- **Keep the app budgets *under* `num_ctx`.** The prompt = system + pinned + history + retrieved
  + answer. Leave headroom for the retrieved chunks (`chunkSize × ragMaxDocs`) and the answer.
  If `chatContextMaxTokens` ≈ `num_ctx`, there is no room for RAG or output.
- **`chatContextWindowSize`** controls how many conversation turns the model "remembers";
  **`chatContextMaxTokens`** is the safety belt so a few very long messages don't blow up the
  prompt (most effective from the CLI today).
- **Pinned docs are always included** and count fully toward occupancy — un-pin what you don't
  need (watch the `pinned ≈` line in `/context`).

---

## Monitoring

### CLI

```
/context            # num_ctx + token budget + estimated occupancy (system/pinned/history) + %
/context 8192       # set num_ctx to 8192, persist, then re-print the report
/models             # installed models with their max context window (← marks the current one)
/show info          # raw model_info incl. <arch>.context_length for the active model
/chunk <file>       # RAG fill-rate: chunkSize × ragMaxDocs vs chatContextMaxTokens
```

`/context` with no argument prints, e.g.:

```
context window (num_ctx) = 4096 tokens
chat token budget (chatContextMaxTokens) = 3000 tokens
estimated occupied ≈ 1240 tokens (30% of window, 41% of budget)
  • system prompt ≈ 120
  • pinned docs   ≈ 870
  • chat history  ≈ 250
  + RAG allowance ≈ 2304 tokens added per query (chunkSize × ragMaxDocs)
(estimates are heuristic: chars/4)
```

### Web

- **Badge** in the chat header: `🪟 ~1,240 / 4,096 tok (30%)`, colour-coded green/amber/red at
  70% / 90% of the window. Refreshes after each reply, on settings save, and on click.
- **Settings → Model**: the list of installed models with their max context window (click a row
  to select that model); the num_ctx field lives in **Settings → Chat**.

### Endpoints

| Endpoint | Returns |
|----------|---------|
| `GET /config` / `PUT /config` | all config incl. `contextWindow`, `chatContextWindowSize`, `chatContextMaxTokens` |
| `GET /context/usage?sessionId=…` | occupancy breakdown (system/pinned/history), `contextWindow`, percentages |
| `GET /models` | `[{name, maxContext, current}]` — installed models + max context window |

---

## Caveats & gotchas

- **Heuristic, not exact.** Token counts are chars/4. Real tokenization varies by model and
  language; treat the numbers as a guide, not a guarantee.
- **Web history isn't token-trimmed.** Only `chatContextWindowSize` (+ 500-char truncation)
  limits the web chat history today; `chatContextMaxTokens` does not trim it.
- **`num_ctx` costs memory.** Raising it increases RAM/VRAM and prompt-eval time; don't set it
  to the model max "just in case".
- **`num_ctx` can't exceed the model.** Setting it higher than the model's trained context has
  no benefit (and Ollama may clamp it). Use `/models` to see the ceiling.
- **Pinned docs dominate occupancy** for document-heavy sessions — they're sent every turn.

---

## Quick reference

| Want to… | Do this |
|----------|---------|
| See current occupancy | `/context` or the web badge |
| Change the model context window | `/context <tokens>` or Settings → Chat |
| See each model's max context | `/models` (or `/show info` for the active one) |
| Limit how many messages are remembered | `chatContextWindowSize` (web Settings / JSON) |
| Cap history size in tokens | `chatContextMaxTokens` (web Settings / JSON; CLI `/ask`) |
| Check RAG fits the budget | `/chunk <file>` |

See also: [tuning-guide.md](tuning-guide.md) for the full `/config` parameter table and the
`/chunk` · `/stat` diagnostics.
