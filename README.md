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
| `kubirag-domain` | aggregati e value object puri, un modulo per bounded context | F1: Notebook Governance con comandi, Concept Knowledge, configurazione |
| `kubirag-app` | servizi applicativi e porte (trait), senza I/O | F1: servizio del notebook |
| `kubirag-store-json` | adattatori di persistenza compatibili con i file Kotlin | F1: manifest con lock e CAS, journal, concetti, `qb-rag.json` |
| `kubirag-acceptance` | suite di accettazione sui golden della versione Kotlin | F0–F1 |
| `kubirag-retrieval`, `kubirag-llm`, `kubirag-http`, `kubirag-cli`, `kubirag-publish`, `kubirag-voice` | | fasi F2–F6 |

### Bounded context in `kubirag-domain`

| Modulo | Aggregate root | Comandi | Domain event |
|---|---|---|---|
| `notebook` | `RagNotebookManifest` (revisione per concorrenza ottimistica, invarianti nel costruttore) | `RagNotebookCommand`: add, add-and-pin, replace, remove, pin, unpin, set-pins, update-properties, refresh, reconcile | `RagNotebookAuditEvent` con snapshot prima/dopo |
| `concept` | `AuthorityConceptGraphFile` (modello in `kubi-kernel`) | in F4 | log interno al grafo |
| `config` | `RagConfig` (value object con i vincoli di `validateConfig`) | | |

Il flusso di un comando è esagonale: `kubirag-app` controlla la revisione attesa, chiede al
dominio di decidere (`RagNotebookCommand::decide`, puro), fa il commit con compare-and-set
attraverso la porta `ManifestRepository`, ricostruisce le proiezioni e scrive sempre un domain
event, anche per i rifiuti e i conflitti. Gli adattatori di `kubirag-store-json` implementano
le porte sul filesystem con lo stesso lock sidecar della JVM.

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

## Fase F1: kernel e notebook

| Key result | Verifica in F1 |
|---|---|
| O1-KR1 round-trip dei golden | 24 passi di comandi eseguiti da Kotlin e da Rust: manifest e journal byte per byte, `OperationResult` identici; `qb-rag.json` (default, personalizzato, reale), `qb-concepts.json` (forma canonica e reale), `qb-evidence.json` (in `kubicommons-rust`) |
| O1-KR2 invarianti | `proptest` con 10.000 sequenze casuali di comandi (`kubirag-domain/tests/notebook_invariants.rs`) |
| O1-KR3 dominio senza I/O | job `domain-purity` esteso a `kubirag-app` |
| O1-KR4 identità e lock | id deterministici delle risorse (`nameUUIDFromBytes`), CAS sotto il lock sidecar della JVM |

La CI usa il branch omonimo di `kubicommons-rust` quando esiste (modifiche accoppiate di kernel
e KubiRAG), altrimenti `main`.

## Uso

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

`kubicommons-rust` è privato: la CI lo scarica con il secret `KUBICOMMONS_RUST_TOKEN` (token fine-grained con lettura dei contenuti di quel repository). Per sviluppare insieme a un checkout locale, aggiungi in `Cargo.toml`:

```toml
[patch."https://github.com/informatelier/kubicommons-rust"]
kubi-kernel = { path = "../kubicommons-rust/crates/kubi-kernel" }
```
