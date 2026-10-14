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
| `kubirag-workspace` | composition root condiviso: radici `.qb/`/`_qb/`, configurazione, notebook governato, catalogo dei notebook | F3 |
| `kubirag-http` | server axum per la SPA, con il frontend di kubirag incorporato nel binario | F3: governance, documenti, notebook, chat, sessioni, ruoli, configurazione, template |
| `kubirag-cli` | binario `kubirag`: REPL, `-c`, `--script` con la sintassi del `CommandTerminal` Kotlin, `server` | F2–F3 |
| `kubirag-acceptance` | suite di accettazione sui golden della versione Kotlin | F0–F3 |
| `kubirag-publish`, `kubirag-voice` | | fasi F5–F6 |

### Bounded context in `kubirag-domain`

| Modulo | Aggregate root | Comandi | Domain event |
|---|---|---|---|
| `notebook` | `RagNotebookManifest` (revisione per concorrenza ottimistica, invarianti nel costruttore) | `RagNotebookCommand`: add, add-and-pin, replace, remove, pin, unpin, set-pins, update-properties, refresh, reconcile | `RagNotebookAuditEvent` con snapshot prima/dopo |
| `concept` | `AuthorityConceptGraphFile` (modello in `kubi-kernel`) | in F4 | log interno al grafo |
| `conversation` | `ChatSessions` (sessioni per utente, limite, branch) e `RoleCatalog` | create, rinomina, branch, cancella; ruoli | |
| `config` | `RagConfig` (value object con i vincoli di `validateConfig`) | | |
| `retrieval` | nessuno: politiche pure (chunking, lingua, fusione, contesto, prompt e parsing del rerank LLM, profilo degli embedding) | | |

Il bounded context **Retrieval** è una proiezione del notebook: `RetrievalRuntime`
(`kubirag-app`) tiene documenti materializzati, indice e vettori e orchestra BM25 → secondo
stadio → contesto → LLM; `RuntimeProjection` lo ricostruisce dopo ogni commit del manifest.
Le porte `SearchEngine`, `VectorStore`, `LlmGateway`, `ProjectionStore` e
`DocumentMaterializer` sono implementate da `kubirag-retrieval`, `kubirag-llm` e
`kubirag-store-json`; `kubirag-workspace` è il composition root comune alla CLI e al server.

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

## Fase F3: server HTTP per la SPA

`kubirag server` sostituisce `RagServer` (Javalin) per il frontend esistente, che è incorporato
nel binario byte per byte (35 file di `src/main/resources`, kubirag 09aba65) e non è stato
toccato. Il server legge e scrive gli stessi file della versione Kotlin, quindi i due si
possono alternare sullo stesso workspace (non in contemporanea: il lock del manifest lo
impedisce).

| Key result | Verifica in F3 |
|---|---|
| O3-KR1 route della SPA | Lo scenario `contracts/golden/http` ha 124 scambi registrati contro Kotlin. Rust li riproduce **124/124** con stato, header, corpo e file `.qb/` identici (`kubirag-acceptance/tests/http_parity.rs`). Le risposte rispettano l'OpenAPI di kubirag, salvo tre lacune dello spec che riguardano anche Kotlin. Il frontend chiama 103 route: tutte sono servite, rimandate o statiche. Sono servite e verificate 34 coppie verbo-route; 86 rispondono `501` e sono rimandate a F4–F6; il resto sono asset. |
| O3-KR2 binari < 50 MB | Il job `release` produce linux-x64, linux-arm64, macOS arm64 e Windows x64 e ne misura la dimensione. Il binario linux-x64 pesa **8,7 MB**. |
| O3-KR3 immagine < 100 MB | Il job `docker` usa un `Dockerfile` multi-stage su distroless `cc`. L'immagine pesa **49 MB** (12 MB compressa). |
| SPA senza modifiche | `contracts/tools/spa_smoke.mjs`, job `spa-smoke`: Chromium apre la SPA, carica una fonte e fa una domanda in chat. Sul server Kotlin e su quello Rust il testo della pagina è identico. |

Le route dei contesti non ancora portati (note, artefatti, studio, wiki, slide, libro, concetti,
evidenze, bound, mindmap, PDF, scraping, archivi) rispondono `501` con
`{"error": "...", "code": "NOT_IMPLEMENTED", "phase": "F4|F5|F6"}`. L'elenco completo è
`DEFERRED_ROUTES` in `kubirag-http`. La SPA mostra quei pannelli vuoti, e il test di
copertura fallisce se una route chiamata dal frontend non è né servita né elencata.

### Divergenze note dalla versione Kotlin (F3)

- `@ruolo /index` in chat non è supportato (dipende dall'indicizzazione dei ruoli, F5).
- Un corpo JSON illeggibile riceve `400 {"error": ...}`. Jackson risponde con un `500`.
- L'upload di un PDF risponde `202 requiresConfig` come Kotlin, ma il passo successivo
  (`/documents/pdf-chunked`) è rimandato a F5.
- Il comportamento sul secondo upload dello stesso file è identico a Kotlin. Dopo il commit il
  documento punta alla risorsa gestita, quindi il secondo upload diventa un nuovo `ADD`.
- `qb-rag.json` registra `version` = `<versione del crate>-rust`. Kotlin scrive la propria
  build. Come in Kotlin, `OLLAMA_BASE_URL` prevale su `ollamaBaseUrl` anche nella CLI.
- `GET /models` richiede che `/api/show` restituisca `modelfile`, `template`, `license` e
  `parameters`. Come in Kotlin, un modello senza questi campi viene escluso.
- Correzione rispetto a F2: il lock del manifest (`.qb/rag/notebooks/locks/<id>.lock`) e il
  journal (`_qb/governance/rag/notebooks/<id>.json`) ora usano gli stessi percorsi di
  `RagWorkspace`, anche nella CLI. In F2 il journal stava nella cartella del notebook.
- `GET /chat/query` (la variante SSE che la SPA non usa) è rimandato. La SPA usa `POST`.

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
cargo run --release -p kubirag-cli -- server kubirag --port 7070   # SPA su http://localhost:7070

docker build -t kubirag .          # richiede kubi-kernel in .deps/ (vedi Dockerfile)
docker run -p 7070:7070 -v "$PWD:/data" kubirag
```

`kubirag server` accetta gli stessi argomenti della versione Kotlin: lo store, `--port`, e
`--load` con `--recursive` e `--include` per importare fonti all'avvio.

La CLI usa le stesse radici di Kotlin: `.qb/` nella directory corrente (o `QB_ROOT_PATH`) e
l'autorità di `qb-project.json` (o `QB_AUTHORITY_PATH`); legge `.qb/qb-rag.json` e parla con
Ollama all'indirizzo della configurazione.

`kubicommons-rust` è privato: la CI lo scarica con il secret `KUBICOMMONS_RUST_TOKEN` (token fine-grained con lettura dei contenuti di quel repository). Per sviluppare insieme a un checkout locale, aggiungi in `Cargo.toml`:

```toml
[patch."https://github.com/informatelier/kubicommons-rust"]
kubi-kernel = { path = "../kubicommons-rust/crates/kubi-kernel" }
```
