# Contratti di KubiRAG

Suite di accettazione del porting. Tutto ciò che sta in `golden/` è prodotto dalla versione
Kotlin di riferimento (kubirag commit `09aba65`, kotlinx-serialization 1.10) oppure copiato
dal repository kubirag; i test di `crates/kubirag-acceptance` lo usano contro la versione Rust.

| Percorso | Contenuto | Origine |
|---|---|---|
| `golden/notebook/manifests/empty-manifest.json` | notebook vuoto | `RagNotebookManifestRepository.write` |
| `golden/notebook/manifests/rich-manifest.json` | tutti i campi, unicode, escape, risorse managed/https/bound | `RagNotebookManifestRepository.write` |
| `golden/notebook/manifests/real-kubirag-manifest.json` | manifest reale del repository kubirag | copia |
| `golden/notebook/read-classification.json` | 17 casi di lettura (Current, Legacy, versioni, invarianti) | `RagNotebookManifestRepository.read` |
| `golden/notebook/stable-notebook-ids.json` | id deterministici dei notebook legacy | `stableNotebookId` |
| `golden/notebook/audit/notebook-audit.json` | journal con due domain event | `RagNotebookAuditJournal.append` |
| `golden/notebook/audit/real-kubirag-audit.json` | journal reale del repository kubirag | copia |
| `golden/notebook/scenarios/scenarios.json` | 24 passi: tutti i comandi, rifiuti, conflitti, file modificati e cancellati | scritto a mano |
| `golden/notebook/scenarios/expected/<scenario>/` | manifest finale, journal (id `event-<n>`, timestamp fisso), `OperationResult` di ogni passo | `RagNotebookApplicationService` |
| `golden/config/{default,custom}-qb-rag.json` | `.qb/qb-rag.json` con default omessi, `Float` e `Double` | `RagConfig.storeConfig` |
| `golden/config/real-kubirag-qb-rag.json` | configurazione reale del repository kubirag | copia |
| `golden/concept/rich-input.json`, `rich-concepts.json` | grafo non canonico e come lo scrive kubirag | `AuthoritativeConceptGraphRepository.write` |
| `golden/concept/real-kubirag-concepts.json` | grafo reale del repository kubirag | copia |
| `golden/retrieval/corpus/` | 21 guide e manuali it/en di kubirag | copia di `doc/guides`, `doc/manuals` |
| `golden/retrieval/queries.json` | 24 query it/en/mix con le sorgenti rilevanti | scritto a mano |
| `golden/retrieval/lucene-baseline.json` | top-10 di `LuceneSearchEngine` per ogni query | generatore |
| `golden/http/openapi.json`, `excluded-routes.json` | contratto HTTP pubblicato da kubirag | copia di `src/site/resources/openapi` |
| `golden/http/spa-routes.json` | route chiamate dal frontend, incrociate con l'OpenAPI | `tools/spa_routes.py` |

## Cosa hanno fatto emergere i golden

- **Il writer normalizza il manifest**: una risorsa `https://` dichiarata `MANAGED` diventa
  `EXTERNAL`, `BOUND://` diventa `bound://`, gli spazi del path spariscono, `metadata.path` dei
  documenti viene ricavato dalle risorse `document-source:<docId>` (e rimosso dagli altri). Il
  porting del writer (F1) deve replicare queste regole, coperte da `rich-manifest.json`.
- **Il manifest non ha newline finale** quando lo scrive Kotlin; il file reale committato in
  kubirag ne ha uno, aggiunto da un editor. Il codec Rust legge entrambi e scrive come Kotlin.
- **Versione numerica**: `"version": 1` è `Invalid` (kotlinx rifiuta il numero nel campo stringa),
  non `UnsupportedVersion`.
- **Lucene sul corpus**: hit@1 18/24 e nDCG@5 0.8855. Sono la soglia di O2-KR1; le 6 query
  mancate al primo posto sono utili per capire se tantivy migliora o peggiora.

- **Il writer ricalcola gli hash** delle risorse locali a ogni scrittura: un file gestito
  modificato fuori dal notebook non resta in drift dopo il commit successivo, mentre uno
  cancellato diventa `RAG_MANAGED_NOTE_MISSING` al primo `rebuildProjections` (scenario F1).
- **I rifiuti del writer sono rifiuti del comando**: credenziali in un URL remoto o una nota
  gestita mancante producono `REJECTED` con il messaggio del writer, e l'evento finisce nel journal.
- **`qb-rag.json` scrive i `Float` come `Float.toString`** (`0.6`, `2.5`): serde allargherebbe
  a `f64` e scriverebbe `0.6000000238418579`.

## Rigenerare

Serve il fat JAR di kubirag (`mvn -DskipTests install` in kubirag, dopo `mvn install` in kubicommons).

```bash
cd contracts/jvm-generator
mvn -q package -Dkubirag.jar=/percorso/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar
LC_ALL=C.UTF-8 java -cp target/kubirag-contracts.jar:/percorso/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar \
  it.informatelier.kubirag.contracts.KubiragContractsKt generate ../golden
python3 ../tools/spa_routes.py /percorso/kubirag/src/main/resources \
  ../golden/http/openapi.json ../golden/http/excluded-routes.json ../golden/http/spa-routes.json
```

`LC_ALL=C.UTF-8` serve perché la JVM, senza locale, non sa creare file con nomi non ASCII.
Il generatore è deterministico: due esecuzioni producono gli stessi byte. Un golden che cambia
è una modifica del contratto e va rivista insieme al codice Kotlin che l'ha causata.
