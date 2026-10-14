# Migrare un progetto KubiRAG dal layout prototipo `.qb`

Questa guida spiega come recuperare un progetto creato con una versione precedente di KubiRAG quando
l'avvio si interrompe con:

```text
QB_UNSUPPORTED_PROTOTYPE_LAYOUT: Prototype workspace under .qb is unsupported;
recreate it or export/import explicitly
```

La procedura è intenzionalmente non distruttiva: il vecchio contenuto di `.qb` resta disponibile come
sorgente locale e il nuovo stato autoritativo viene scritto separatamente in `_qb`.

## Perché KubiRAG si ferma

I progetti recenti separano due tipi di persistenza:

| Directory | Contenuto | Condivisione |
|---|---|---|
| `_qb/` | aggregati autoritativi e governance semantica | destinata al versionamento e alla condivisione |
| `.qb/` | configurazione locale, backend, cache, indici, log e recovery | locale, normalmente ignorata da Git |

Nel layout prototipo alcuni file autoritativi erano salvati sotto `.qb` e mancava il bootstrap
`qb-project.json`. KubiRAG non decide autonomamente quali file spostare: una copia indiscriminata potrebbe
pubblicare configurazioni locali, percorsi assoluti, dati sensibili o proiezioni ricostruibili.

## Scegliere la strategia

Sono disponibili due percorsi.




### A. Ricreazione del progetto

Usare questa strategia quando i documenti originali sono ancora disponibili e non serve preservare gli
ID o i metadati prodotti dal vecchio backend.

1. Creare un nuovo progetto in una directory vuota.
2. Creare il file `qb-project.json` descritto più avanti.
3. Avviare KubiRAG e reimportare i documenti originali.
4. Ricreare manualmente pin e configurazioni che si desidera conservare.
5. Conservare il vecchio progetto come archivio finché il nuovo non è stato verificato.

Non copiare l'intera vecchia `.qb` nel nuovo progetto: ricreerebbe proprio il layout che KubiRAG rifiuta.





### B. Adozione esplicita del backend esistente

Usare questa strategia per conservare i documenti e i metadati già presenti nel backend JSON o ArcadeDB.
È il percorso descritto nel resto della guida.

L'adozione crea, per ogni notebook, il manifest:

```text
_qb/notes/<notebook>/<notebook>-manifest.json
```

L'operazione non elimina né riscrive il backend legacy.

## Prima di iniziare una copia di sicurezza dei dati

1. Arrestare tutte le istanze KubiRAG che usano il progetto. È obbligatorio soprattutto per ArcadeDB,
   che ammette un solo processo scrittore.
2. Fare una copia completa del progetto, inclusa la directory nascosta `.qb`.
3. Verificare che la copia si apra e contenga i file attesi.
4. Eseguire i comandi seguenti dalla radice del progetto, non dalla directory del JAR se è diversa.

Negli esempi il JAR è indicato con un nome relativo. Se si trova altrove, sostituirlo con il suo percorso
assoluto o con un percorso relativo alla radice del progetto.

Esempio di archivio su Linux, macOS o WSL:

```bash
tar -czf ../progetto-prima-migrazione.tgz .
```

Su PowerShell:

```powershell
Compress-Archive -Path .\* -DestinationPath ..\progetto-prima-migrazione.zip
```

`Compress-Archive` può omettere file nascosti in alcune versioni di PowerShell. In quel caso copiare
esplicitamente anche `.qb` oppure usare uno strumento di backup che includa le directory nascoste.




### Inventario preliminare

I notebook locali sono normalmente le sottodirectory di `.qb/notes`:

```bash
find .qb/notes -mindepth 1 -maxdepth 1 -type d -printf '%f\n'
```

Su PowerShell:

```powershell
Get-ChildItem .qb\notes -Directory | Select-Object -ExpandProperty Name
```

Annotare:

- i nomi dei notebook da adottare;
- il backend configurato in `.qb/qb-rag.json` (`default` oppure `arcadedb`);
- eventuali cartelle notebook collegate tramite symlink o junction;
- il numero atteso di documenti per ogni notebook;
- i pin importanti, che potrebbero dover essere ricreati manualmente.

## Passo 1 — Dichiarare la directory autoritativa

Creare nella radice del progetto un file `qb-project.json` con questo contenuto:

```json
{
  "version": "1",
  "authorityDirectory": "_qb"
}
```

Il percorso è relativo alla radice del progetto. Non impostarlo a `.qb`, a una directory assoluta o a un
percorso contenente `..`.

Il solo bootstrap non migra alcun dato: permette a KubiRAG di usare `.qb` come sorgente/runtime locale e
`_qb` come nuova autorità.

## Passo 2 — Avviare un notebook legacy

Avviare il server indicando esplicitamente il nome di un notebook trovato nell'inventario:

```bash
java --add-modules jdk.incubator.vector \
  -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server <notebook>
```

Esempio:

```bash
java --add-modules jdk.incubator.vector \
  -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server research
```

Il nome del notebook può essere combinato con le opzioni del server, per esempio
`server research --port 7071`.

Prima dell'export verificare che sia stato caricato il backend corretto:

```bash
curl http://localhost:7070/documents
```

Controllare che la risposta contenga i documenti attesi. Se è vuota, non esportare: arrestare il server e
verificare nome del notebook, `.qb/qb-rag.json` e selezione del backend.

## Passo 3 — Adottare il backend

L'export iniziale richiede esplicitamente la revisione concettuale `0`:

```bash
curl -i -X POST http://localhost:7070/governance/export \
  -H 'If-Match: "0"' \
  -H 'X-QB-Actor: legacy-migration'
```

Su PowerShell usare `curl.exe` per evitare l'alias di `Invoke-WebRequest`:

```powershell
curl.exe -i -X POST http://localhost:7070/governance/export `
  -H "If-Match: `"0`"" `
  -H "X-QB-Actor: legacy-migration"
```

Il risultato atteso è HTTP `201 Created` con stato `MIGRATED`. L'operazione:

- legge i documenti dal backend attivo;
- legge le proprietà notebook legacy, quando disponibili;
- crea il manifest versione 1 e revisione 0 sotto `_qb`;
- non cancella i file JSON né il database ArcadeDB in `.qb`;
- non riscrive i documenti sorgente esterni.

Non ripetere l'export sullo stesso notebook: se il manifest esiste già, la risposta `409 Conflict` è una
protezione contro la sovrascrittura e non indica perdita di dati.

## Passo 4 — Verificare il risultato

Interrogare stato, health, issue e report di migrazione:

```bash
curl -i http://localhost:7070/governance
curl http://localhost:7070/governance/health
curl http://localhost:7070/governance/issues
curl http://localhost:7070/governance/migration
```

Verificare anche la presenza del file:

```text
_qb/notes/<notebook>/<notebook>-manifest.json
```

Controlli minimi sul JSON:

- `version` vale `"1"`;
- `name` coincide con il notebook;
- `revision` vale `0` subito dopo l'export;
- il numero degli elementi in `documents` coincide con quello atteso;
- i percorsi in `metadata` sono ancora validi oppure sono consapevolmente locali.

Per verificare le proiezioni derivate si può richiedere un rebuild usando l'ETag/revisione restituito dallo
status. Subito dopo il primo export la revisione è normalmente `0`:

```bash
curl -i -X POST http://localhost:7070/governance/rebuild \
  -H 'If-Match: "0"' \
  -H 'X-QB-Actor: legacy-migration'
```

Il rebuild riguarda lo stato ricostruibile del notebook attivo; non è un secondo export.

## Passo 5 — Ripetere per gli altri notebook

Per ogni notebook legacy:

1. arrestare il server;
2. riavviarlo con `server <altro-notebook>`;
3. controllare `/documents`;
4. eseguire una sola volta `POST /governance/export` con `If-Match: "0"`;
5. verificare status, issue e manifest.

Non copiare manualmente `<notebook>-documents.json` dentro `_qb`: il manifest ha un formato e invarianti
diversi dal vecchio snapshot del backend.

## Passo 6 — Versionare solo lo stato condivisibile

Dopo aver verificato tutti i notebook, controllare cosa verrebbe incluso nel versionamento:

```bash
git status --short --untracked-files=all
git check-ignore -v .qb/qb-rag.json
```

In generale sono candidati alla condivisione:

- `qb-project.json`;
- i manifest e la governance sotto `_qb`.

Devono restare locali:

- `.qb/qb-rag.json` e altre configurazioni personali;
- log e sessioni chat;
- embeddings, indici Lucene e altre proiezioni ricostruibili;
- database ArcadeDB, lock, staging e checkpoint locali;
- credenziali e percorsi assoluti specifici della macchina.

Revisionare comunque il contenuto di `_qb` prima di aggiungerlo a Git, soprattutto se i metadati dei
documenti contengono percorsi locali.

## Cosa non viene migrato dall'export notebook

`POST /governance/export` adotta l'aggregato notebook; non è una migrazione universale di `.qb`.

| Dato legacy | Esito |
|---|---|
| documenti del backend attivo | inclusi nel manifest |
| proprietà notebook | incluse quando disponibili |
| pin del vecchio JSON | non garantiti; verificarli e ricrearli |
| embeddings e indice full-text | restano locali e sono ricostruibili |
| database ArcadeDB | resta backend locale; non viene copiato in `_qb` |
| note Markdown e asset | restano nella posizione corrente salvo adozioni specifiche |
| `qb-evidence.json`, concept link/alias/log | restano locali nell'implementazione corrente |
| configurazione RAG, ruoli, voci e sessioni | restano locali |

Non spostare uno di questi file in `_qb` soltanto perché contiene dati importanti. Prima deve esistere una
classificazione e, per i dati autoritativi, un contratto del relativo produttore.

## Percorso alternativo dalla CLI interattiva

Dopo aver creato `qb-project.json`, il notebook iniziale può essere adottato dalla REPL con:

```text
/governance export
/governance status
```

Per un notebook diverso, `/restore <notebook>` può segnalare inizialmente che manca il manifest dopo aver
caricato il backend legacy. In tal caso eseguire subito `/governance export` e poi `/governance status`.
Per una migrazione di più notebook, la procedura server descritta sopra rende più evidente quale notebook
sia attivo e offre risposte HTTP verificabili.

## Risoluzione dei problemi

### L'errore `QB_UNSUPPORTED_PROTOTYPE_LAYOUT` continua

- verificare che `qb-project.json` sia nella radice del progetto;
- avviare il processo dalla stessa radice;
- controllare maiuscole, estensione e validità del JSON;
- non collocare il file dentro `.qb` o `_qb`.

### `428 Precondition Required`

Manca l'header `If-Match`. Per il primo export usare esattamente `If-Match: "0"`.

### `409 Conflict` durante l'export

Il manifest potrebbe esistere già, oppure la revisione richiesta non è quella iniziale. Interrogare
`GET /governance` e ispezionare `_qb/notes/<notebook>`; non cancellare il manifest per forzare l'export.

### `/documents` restituisce zero elementi

È stato aperto il notebook sbagliato oppure `.qb/qb-rag.json` seleziona un backend diverso da quello che
contiene i dati. Arrestare il processo e correggere la selezione prima di esportare.

### Health è `degraded`

Leggere `/governance/issues`. Risorse esterne mancanti o modificate possono essere riparabili; una versione
futura o un manifest invalido richiedono invece di fermarsi e usare una versione KubiRAG compatibile.

### Il progetto usa ArcadeDB

- fare il backup solo a processo arrestato;
- non avviare due istanze sullo stesso store;
- verificare che `documentBackend` in `.qb/qb-rag.json` sia `arcadedb` prima dell'avvio;
- controllare `/documents` prima dell'export;
- conservare il database locale anche dopo la creazione del manifest.

## Criteri di completamento

La conversione può considerarsi verificata quando:

- KubiRAG parte senza `QB_UNSUPPORTED_PROTOTYPE_LAYOUT`;
- ogni notebook necessario ha un manifest valido in `_qb`;
- numero e identità dei documenti sono coerenti con il backend legacy;
- `/governance/health` non contiene errori non compresi;
- un riavvio continua a esporre lo stesso stato;
- `.qb` è ancora disponibile come runtime locale e il backup precedente è conservato;
- `qb-project.json` e `_qb` sono stati revisionati prima dell'eventuale commit.
