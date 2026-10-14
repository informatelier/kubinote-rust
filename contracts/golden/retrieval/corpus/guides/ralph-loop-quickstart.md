# Ralph-loop Quick Start Guide

Get Ralph-loop running in under 5 minutes.

## Prerequisites

- KubiRAG built and running (`make cli`)
- Ollama running at `http://localhost:11434`
- A model loaded in Ollama (e.g. `ollama pull llama3`)

---

## Step 1 — Start the CLI

```bash
make cli
```

You'll see the KubiRAG prompt (`>`).

---

## Step 2 — Run your first task

Process a single built-in task:

```
/ralph-loop --tasks "documentation" --iterations 1
```

Ralph-loop will:
1. Load the `documentation` task
2. Create a clean iteration context
3. Query the RAG system with the task description
4. Save progress to `progress.md`
5. Display a summary

---

## Step 3 — Process multiple tasks

```
/ralph-loop --tasks "test-coverage,documentation,code-quality"
```

Built-in task IDs: `test-coverage`, `documentation`, `code-quality`, `performance`

---

## Step 4 — Check configuration

```
/ralph-config show
```

Adjust limits as needed:

```
/ralph-config set maxIterations 50
/ralph-config set qualityThreshold 0.9
```

---

## Step 5 — Resume after interruption

If the loop was interrupted, resume from where it left off:

```
/ralph-loop --resume
```

State is read from `ralph-loop-state.json`.

---

## Common Options

| Option | Example | Effect |
|---|---|---|
| `--tasks` | `--tasks "test-coverage"` | Select specific tasks |
| `--iterations` | `--iterations 10` | Limit loop iterations |
| `--auto` | `--auto` | Skip user prompts |
| `--debug` | `--debug` | Enable verbose output |
| `--resume` | `--resume` | Resume from saved state |
| `--quality` | `--quality 0.9` | Override quality threshold |

---

## Output Files

| File | Purpose |
|---|---|
| `progress.md` | Human-readable iteration log |
| `ralph-loop-state.json` | Machine-readable state for `--resume` |
| `ralph-loop-config.json` | Persisted configuration |
| `ralph-loop-context-archive/` | Archived iteration contexts |

---

## Next Steps

- Read the full manual: `doc/ralph-loop-feature-manual.md`
- Explore the API reference: `doc/ralph-loop-api.md`
- Browse example tasks: `examples/ralph-loop/`
