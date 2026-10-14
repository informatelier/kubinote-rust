# Logging Conventions

How to emit diagnostics in KubiRAG without polluting the user-facing terminal.

---

## The two channels

KubiRAG has **two independent output channels**. Mixing them is a bug.

| Channel  | API                              | Audience      | Destination           |
|----------|----------------------------------|---------------|-----------------------|
| Content  | `println` / `print` / `chunkCallback` | End user    | Terminal (REPL)       |
| Logger   | SLF4J `logger.xxx(...)`          | Developer/ops | `.qb/qb-app.log`      |

The content channel is what the user sees at the `>` prompt — the answer text, a formatted table, a `/help` listing. The logger channel is invisible during normal use; it only appears in the rolling logfile for post-mortem debugging.

**Rule**: never emit diagnostics, timing data, chunk counts, or internal state through the content channel. Never emit user-facing output through the logger.

### The motivating bug

In `OllamaClient3.executeStreamRequest`, end-of-stream telemetry was being written with `chunkCallback(" [time ms:$elapsed] ")` — the same callback that streamed the LLM's answer to the terminal. When Qwen3 kept the socket open and emitted trailing frames after `done: true`, the loop never exited, and the terminal filled with an endless series of `[time ms:0]` tokens on top of the correct answer.

The fix was two-part:

1. Break the loop on the first `done: true` frame (the real bug).
2. Move the timing line off `chunkCallback` and onto `logger.info(...)` (channel discipline).

Without step 2, even a correctly-terminated stream would still have leaked "`[time ms:234]`" into the user's answer buffer on every request. The guide exists so this class of mistake does not recur.

---

## Declaring a logger

Every class that logs does so via an **instance field**, declared the same way:

```kotlin
import org.slf4j.LoggerFactory

class MyService {
    private val logger = LoggerFactory.getLogger(MyService::class.java)
    // ...
}
```

- Always `private val` — never exposed outside the class.
- Always `::class.java` — the codebase does not use KotlinLogging or `Logger<MyService>()` variants.
- Never a companion-object logger — instance field is enough and avoids the extra indirection.
- Never share one logger across multiple classes — the logger name is how you turn things on/off in `kubirag-logback.xml`.

Grep `RagLucene.kt`, `HybridSearchService.kt`, `LLMRerankingService.kt`, `RAGSystem`, `InMemoryVectorStore.kt`, or `OllamaClient3.kt` for a reference declaration.

---

## Level ladder

| Level   | When to use                                                                                     |
|---------|-------------------------------------------------------------------------------------------------|
| `error` | Unrecoverable failure. An operation aborted and the user either saw an error or will shortly. Always pass the exception as the last argument. |
| `warn`  | Recoverable anomaly worth remembering. Fallback taken, safety valve tripped, unusual state detected. |
| `info`  | One line per completed high-level operation. Stream finished, config loaded, session restored.  |
| `debug` | Per-step diagnostics. Per-chunk data, per-document scores, parse errors, branch decisions.       |
| `trace` | Byte-level or per-character detail. Almost never used in this codebase.                          |

The `root` level in `kubirag-logback.xml` is `error`, so by default only errors reach the logfile. `info` and `debug` lines are only visible for classes that have an explicit per-class override (see below).

### Rule of thumb: one `info` per operation

A stream handler should emit **one** `info` line when the stream completes — summarizing elapsed time, chunk count, done frames, and parse errors. Not one per chunk. Per-chunk detail goes to `debug`.

```kotlin
logger.info(
    "stream complete: ms={} chunks={} doneFrames={} parseErrors={}",
    elapsedTime, chunkCount, doneCount, parseErrors
)
```

If every request prints three `info` lines, the logfile becomes useless during investigation. Stay disciplined.

---

## Message format

**Prefer SLF4J `{}` placeholders over Kotlin string templates** for new code:

```kotlin
// Good — no string built unless the level is enabled
logger.debug("chunk #{} done={} contentLen={}", chunkCount, parsed.done, content.length)

// Tolerable in cold paths — builds the string even when debug is off
logger.info("Restored ${docs.size} documents from disk")
```

Why `{}` matters: SLF4J only interpolates the arguments after checking the effective level. With a `

 template, Kotlin builds the final string unconditionally, then SLF4J decides whether to keep it. In a hot loop (per-chunk, per-document, per-token) that extra allocation is measurable.

Existing code that uses `

 templates does **not** need to be rewritten. The guidance applies to new code and any file you're already editing.

### Exceptions

Pass the exception as the **last positional argument** — SLF4J will print the stack trace:

```kotlin
try {
    reRank(docs)
} catch (e: Exception) {
    logger.error("Re-ranking failed, returning original results", e)
    return docs
}
```

Do not concatenate `${e.message}` into the message string — you lose the stack trace and gain nothing.

### Truncate user-supplied content

When logging a line of input, a parse-error fragment, or a query string, truncate it:

```kotlin
logger.debug("parse error (#{}): line='{}' msg={}", parseErrors, line.take(200), e.message)
```

A pathological 10 MB stream frame should not land in the logfile.

---

## Why the file is named `kubirag-logback.xml` (not `logback.xml`)

Logback auto-discovers only two filenames on the classpath: `logback-test.xml` and `logback.xml`. KubiRAG intentionally uses a **third, non-standard name** (`kubirag-logback.xml`) so that when KubiRAG is pulled into another project as a library dependency, logback's auto-discovery never picks it up. The downstream project's own config wins by default, instead of being silently shadowed by KubiRAG's file travelling inside the fat JAR.

Because the file has a non-standard name, logback won't find it on its own — it has to be told. `Starter.main()` sets the system property before any logger is created:

```kotlin
fun main(args: Array<String>) {
    if (System.getProperty("logback.configurationFile") == null) {
        System.setProperty("logback.configurationFile", "kubirag-logback.xml")
    }
    // ... rest of bootstrap
}
```

This line must stay as the first statement of `main`, before any class that declares a `private val logger = LoggerFactory.getLogger(...)` gets loaded — otherwise logback will initialize with its defaults and ignore the property afterwards.

The `if (== null)` guard respects manual overrides: running with `-Dlogback.configurationFile=/path/to/other.xml` still works (useful for ad-hoc debug sessions pointing at an experimental config).

Background: this pattern was adopted after a bug where `kubicommon` (a library dependency) shipped its own `logback.xml`, which won the fat-JAR merge against KubiRAG's copy — DEBUG lines from `OllamaClient3` silently never appeared. Since KubiRAG is itself used as a library by other projects, the same trap would have cascaded one level deeper. The rename makes the class of bug structurally impossible.

---

## Where logs go

`src/main/resources/kubirag-logback.xml` defines:

- A rolling `FILE` appender writing to `.qb/qb-app.log`, rotated by date and size (100 MB per file, 2 GB cap, 3 days of history).
- No console appender — this is intentional. If logs went to stdout, they would collide with the REPL content channel.
- Root level `error`.
- Per-class overrides as needed (see `OllamaClient3` for an example).

To read recent logs during development:

```bash
tail -f .qb/qb-app.log
```

---

## Enabling `info` or `debug` for a specific class

When you are investigating something — a streaming anomaly, a retrieval miss, an embedding failure — open `kubirag-logback.xml` and add a `<logger>` entry **above** the `<root>` block:

```xml
<!--
    OllamaClient3 streaming telemetry.
    - INFO: one line per completed stream with elapsed ms, chunk count, doneFrames, parseErrors.
    - DEBUG: additionally logs per-chunk info (done flag, content length) and parse errors with line previews.
-->
<logger name="it.informatelier.kubilang.ragollama.OllamaClient3" level="info"/>
```

Bump `level="info"` to `level="debug"` to see the per-chunk detail. Change it back when you're done — the rotation cap will protect you from a runaway debug trace, but a debug level left on in production still hurts disk and readability.

The logger `name` attribute must match the **fully-qualified class name** passed to `LoggerFactory.getLogger(...)`. Typos here silently do nothing.

### Enabling a whole package

Use a package prefix to catch multiple classes:

```xml
<logger name="it.informatelier.kubilang.ragollama" level="debug"/>
```

Useful when a feature spans several files.

---

## When `println` is still correct

`println` / `print` are **not banned**. The REPL is fundamentally a terminal UI; its output has to go to stdout.

| Use `println` when...                                   | Use `logger` when...                                  |
|---------------------------------------------------------|-------------------------------------------------------|
| Showing the answer text to the user                     | Timing, counts, internal state                        |
| Printing a table of documents, roles, sessions          | Errors that the user will recover from transparently  |
| Command output (`/help`, `/config`, `/model`)           | Protocol-level decisions (chose backend X, fell back) |
| Formatted prompts and banners                           | Parse errors, network retries                         |
| Progress bars and spinners (Spinner class)              | One-line-per-operation summaries                      |

A good mental check: **if I pipe the terminal output into a file and hand it to the user, should this line be in the file?** If yes → `println`. If no → `logger`.

---

## Recipes

### Adding a logger to a new class

```kotlin
import org.slf4j.LoggerFactory

class NewService {
    private val logger = LoggerFactory.getLogger(NewService::class.java)

    fun doWork() {
        logger.debug("doWork start")
        // ...
        logger.info("doWork complete: processed={}", count)
    }
}
```

No configuration change needed — the line will land in `.qb/qb-app.log` as soon as you raise the effective level for this class.

### Investigating a streaming anomaly

1. Edit `kubirag-logback.xml`: set the relevant class logger to `level="debug"`.
2. Rebuild: `make build` (logback config is bundled into the JAR).
3. Reproduce the issue via `make cli`.
4. Inspect `.qb/qb-app.log` — look for the per-chunk `debug` lines and the final `info` summary.
5. Revert `kubirag-logback.xml` when done.

### Seeing `info` output from a whole subsystem

```xml
<logger name="it.informatelier.kubilang.lucene" level="info"/>
```

Rebuild and rerun. Every class under `lucene/` whose logger follows the standard declaration will now emit `info`-and-above to the logfile.

### Logging an exception without losing the stack trace

```kotlin
try {
    client.chat(...)
} catch (e: IOException) {
    logger.warn("chat request failed, will retry in {}ms", retryDelay, e)
}
```

Note: `e` is the last argument. SLF4J detects it and prints the stack trace automatically — the `{}` placeholders only consume the non-exception arguments.

### Hot-loop guard clause (rare)

If building the arguments to a `debug` call is itself expensive (e.g. serializing a 10k-element list), guard with `isDebugEnabled`:

```kotlin
if (logger.isDebugEnabled) {
    logger.debug("state dump: {}", expensivelyRenderState())
}
```

For most call sites, SLF4J's lazy `{}` substitution is already enough — this guard is a last resort for genuinely expensive argument construction.

---

## Checklist before committing

- [ ] New loggers follow the `private val logger = LoggerFactory.getLogger(MyClass::class.java)` pattern.
- [ ] No `println` added for diagnostic or timing output.
- [ ] No diagnostic data flows through a content callback (e.g. `chunkCallback`, UI writer, stdout).
- [ ] `error` calls pass the exception as the last argument.
- [ ] New `info` calls emit at most one line per completed operation.
- [ ] Any `kubirag-logback.xml` level bumps made during investigation have been reverted.
