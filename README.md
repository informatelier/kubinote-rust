# kubinote-rust

Porting in Rust di [KubiRAG](https://github.com/informatelier/kubirag): notebook di conoscenza
locale con RAG su Ollama. Strategia **strangler fig**: la versione Rust legge e scrive gli stessi
notebook della versione Kotlin, quindi si migra una funzione alla volta e si può tornare indietro.

Target: Linux, macOS e Windows (CI su tutti e tre).

## Architettura

Workspace Cargo esagonale: il compilatore impone la direzione delle dipendenze, come ArchUnit
nella versione Kotlin. Il kernel condiviso è [`kubi-kernel`](https://github.com/informatelier/kubicommons-rust),
seconda implementazione del Published Language di `kubicommons`.

| Crate | Ruolo | Stato |
|---|---|---|
| `kubirag-domain` | aggregati e value object puri, un modulo per bounded context | F0: Notebook Governance |
| `kubirag-store-json` | persistenza compatibile con i file Kotlin | F0: codec del manifest |
| `kubirag-acceptance` | suite di accettazione sui golden della versione Kotlin | F0 |
| `kubirag-app`, `kubirag-retrieval`, `kubirag-llm`, `kubirag-http`, `kubirag-cli`, `kubirag-publish`, `kubirag-voice` | | fasi F1–F6 |

### Bounded context in `kubirag-domain`

| Modulo | Aggregate root | Domain event |
|---|---|---|
| `notebook` | `RagNotebookManifest` (revisione per concorrenza ottimistica, invarianti nel costruttore) | `RagNotebookAuditEvent` con snapshot prima/dopo |

## Fase F0: contratti e golden file

I golden in [`contracts/golden`](contracts/README.md) sono generati dalla versione Kotlin e
verificati dai test di `kubirag-acceptance`.

| Key result | Verifica in F0 |
|---|---|
| O1-KR1 round-trip dei golden | manifest (vuoto, completo, reale) e audit journal byte per byte; classificazione in lettura su 17 casi |
| O1-KR3 dominio senza I/O | job `domain-purity` della CI (`cargo tree`) |
| O1-KR4 identità e lock | `stableNotebookId` e lock JVM ↔ Rust (in `kubicommons-rust`) |
| O2-KR1 retrieval | 24 query it/en con la baseline Lucene: hit@1 18/24, nDCG@5 0.8855 |
| O3-KR1 route della SPA | 92 route chiamate dal frontend: 90 nell'OpenAPI di kubirag, 2 asset HTML statici |

## Uso

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Per sviluppare insieme a un checkout locale di `kubicommons-rust`, aggiungi in `Cargo.toml`:

```toml
[patch."https://github.com/informatelier/kubicommons-rust"]
kubi-kernel = { path = "../kubicommons-rust/crates/kubi-kernel" }
```
