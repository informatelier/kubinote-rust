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
| `kubirag-retrieval` | motore BM25 compatibile Lucene (analyzer it/en, espansione, frasi, diversificazione) e vettori in memoria | F2 |
| `kubirag-llm` | gateway Ollama sincrono: generazione in streaming NDJSON ed embedding | F2 |
| `kubirag-cli` | binario `kubirag`: REPL, `-c`, `--script` con la sintassi del `CommandTerminal` Kotlin | F2: `/load`, `/list`, `/pin`, `/unpin`, `/pinned`, `/stat`, `/embed`, query |
| `kubirag-acceptance` | suite di accettazione sui golden della versione Kotlin | F0–F2 |
| `kubirag-http`, `kubirag-publish`, `kubirag-voice` | | fasi F3–F6 |

### Bounded context in `kubirag-domain`

| Modulo | Aggregate root | Comandi | Domain event |
|---|---|---|---|
| `notebook` | `RagNotebookManifest` (revisione per concorrenza ottimistica, invarianti nel costruttore) | `RagNotebookCommand`: add, add-and-pin, replace, remove, pin, unpin, set-pins, update-properties, refresh, reconcile | `RagNotebookAuditEvent` con snapshot prima/dopo |
| `concept` | `AuthorityConceptGraphFile` (modello in `kubi-kernel`) | in F4 | log interno al grafo |
| `config` | `RagConfig` (value object con i vincoli di `validateConfig`) | | |
| `retrieval` | nessuno: politiche pure (chunking, lingua, fusione, contesto, prompt e parsing del rerank LLM, profilo degli embedding) | | |

Il bounded context **Retrieval** è una proiezione del notebook: `RetrievalRuntime`
(`kubirag-app`) tiene documenti materializzati, indice e vettori e orchestra BM25 → secondo
stadio → contesto → LLM; `RuntimeProjection` lo ricostruisce dopo ogni commit del manifest.
Le porte `SearchEngine`, `VectorStore`, `LlmGateway`, `ProjectionStore` e
`DocumentMaterializer` sono implementate da `kubirag-retrieval`, `kubirag-llm` e
`kubirag-store-json`; `kubirag-cli` è il composition root.

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

## Fase F2: retrieval e CLI minima

| Key result | Verifica in F2 |
|---|---|
| O2-KR1 qualità ≥ Lucene | overlap@5 medio **100%** (minimo 100%) e nDCG@5 **0.8855**, uguale a Lucene: i punteggi BM25 coincidono bit per bit su 24 query del corpus e 13 sintetiche (`kubirag-retrieval/tests/golden_lucene.rs`, `kubirag-acceptance/tests/retrieval_engine.rs`) |
| O2-KR2 latenza | p95 **0.22 ms** contro 1.60 ms di Lucene sulle stesse 960 query, cold start del motore **28 ms** contro 416 ms; il binario apre un notebook di 21 sorgenti e risponde a `/stat` in 30–70 ms. In CI le soglie sono verificate in `--release` |
| O2-KR3 esiti del rerank | i 6 `RerankOutcome` raggiunti dalla pipeline reale con un Ollama finto, ogni fallimento con l'ordine BM25 intatto (`kubirag-acceptance/tests/rerank_outcomes.rs`) |

Parità verificata anche su analyzer (51 chunk, 41 campioni), lingua (68 casi), chunking
(2 configurazioni), proiezioni JSON byte per byte e testi della CLI (`kubirag-cli/tests/cli.rs`).

### Divergenze note dalla versione Kotlin

- `/load` di un PDF non è ancora disponibile; `.qbignore` non è interpretato.
- I ruoli (`@ruolo`, `@party`) non sono letti: la CLI risponde come Kotlin senza ruoli definiti.
- Il rerank LLM riceve il testo vero anche dei chunk inglesi (Kotlin passa una stringa vuota).
- `/lucene domanda` non include `/lucene` nella domanda (Kotlin sì).
- `/embed` salva subito i vettori: non serve `/store`. Come in Kotlin, `/load` non crea embedding.
- Numeri sempre con il punto decimale (Kotlin usa la locale di sistema).
- Il testo di `--explain` approssima quello di Lucene; i punteggi sono identici.
- Niente TLS né proxy verso Ollama (`ureq` senza feature); `OLLAMA_BASE_URL` vale per la
  generazione, gli embedding usano `ollamaBaseUrl` della configurazione, come in Kotlin.
- Il terminale non stampa le righe di debug di `CommandTerminal` (`runWith`, `executeLine`) e
  aggiunge `/help`; la cronologia della chat non è salvata.

La CI usa il branch omonimo di `kubicommons-rust` quando esiste (modifiche accoppiate di kernel
e KubiRAG), altrimenti `main`.

## Uso

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo test --release -p kubirag-acceptance --test retrieval_engine -- --nocapture   # O2-KR2

cargo run --release -p kubirag-cli -- --store kubirag            # REPL
cargo run --release -p kubirag-cli -- -c "/stat come installare kubirag"
cargo run --release -p kubirag-cli -- --script comandi.txt
```

La CLI usa le stesse radici di Kotlin: `.qb/` nella directory corrente (o `QB_ROOT_PATH`) e
l'autorità di `qb-project.json` (o `QB_AUTHORITY_PATH`); legge `.qb/qb-rag.json` e parla con
Ollama all'indirizzo della configurazione.

`kubicommons-rust` è privato: la CI lo scarica con il secret `KUBICOMMONS_RUST_TOKEN` (token fine-grained con lettura dei contenuti di quel repository). Per sviluppare insieme a un checkout locale, aggiungi in `Cargo.toml`:

```toml
[patch."https://github.com/informatelier/kubicommons-rust"]
kubi-kernel = { path = "../kubicommons-rust/crates/kubi-kernel" }
```
