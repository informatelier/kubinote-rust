# Concept Notes Guide

Questa guida descrive il flusso corrente dei concept in KubiRAG: raccolta delle evidence,
promozione in concetti governati, collegamento alle sorgenti e generazione di note Markdown
compatibili con Obsidian.

Una concept note non è l'archivio autoritativo del concetto. È una proiezione leggibile e
modificabile dello stato governato, utile per affiancare alle informazioni prodotte da KubiRAG il
ragionamento umano.

---

## Modello dei dati

KubiRAG distingue tre livelli:

1. le evidence scoperte vengono conservate nella inbox locale `.qb/qb-evidence.json` con stato
   `pending`, `accepted`, `exported` o `rejected`;
2. la promozione di una evidence crea o aggiorna un concept governato;
3. `/concept note` materializza il concept come file Markdown nel notebook attivo.

L'autorità dei concept KubiRAG è l'aggregate versionato:

```text
_qb/qb-concepts.json
```

Contiene identità UUID, descrizioni, alias, relazioni, evidence accettate, provenienza ed eventi di
audit. I vecchi file `.qb/qb-concept-links.json`, `.qb/qb-concept-aliases.json` e
`.qb/qb-concept-log.json` non sono più gli store autoritativi del runtime normale.

`_qb/qb-terms.json` resta invece di proprietà di KubiTerm. KubiRAG può leggerlo come sorgente di
evidence candidate, ma non lo modifica e non usa un termine KubiTerm come identità del concept.

---

## Cosa sono le concept note

Nel notebook attivo una concept note si trova in:

```text
_qb/notes/<notebook>/notes/Concepts/<concept>.md
```

Nella UI e nei risultati CLI il path può essere mostrato in forma assoluta o relativa alla
configurazione corrente. Se la cartella delle note è aperta come vault Obsidian, il file appare come
una normale nota sotto `Concepts/`.

I caratteri `/` e `\` presenti nel nome del concept vengono sostituiti con `_` nel nome del file.

Una nuova nota contiene:

- frontmatter YAML con tipo, nome e UUID;
- titolo del concept;
- blocco gestito con alias, conteggi, sorgenti, snippet ed eventuali relazioni suggerite.

Nel blocco entrano le evidence locali attive, cioè tutte quelle non `rejected`; la presenza nella
nota non implica quindi, da sola, che ogni elemento sia già stato promosso.

Esempio:

```markdown
---
type: concept
concept: "rag evaluation"
concept-id: 8d8d7454-24ab-4f06-93b5-b4bfc38241cb
kubi-page-id: 01868f55-c7ce-402f-8137-d7830ec6c31f
created-by: kubirag
---

# rag evaluation

<!-- kubirag:concept:start -->
## KubiRAG Evidence

- Concept: [[rag evaluation]]
- Aliases: rag eval
- Evidence count: 3
- Manual links: 1
- Updated: 2026-09-14

### Evaluation paper

- Signal: `wiki-link`
- Confidence: 70%
- Source path: `/vault/RAG.md`

> This note links rag evaluation with faithfulness and retrieval recall.
<!-- kubirag:concept:end -->
```

Gli UUID dell'esempio sono illustrativi. `concept-id` identifica il concept nell'aggregate;
`kubi-page-id` identifica stabilmente la pagina Markdown. Questi campi vengono aggiunti quando
KubiRAG crea una nuova concept note; l'aggiornamento di un file esistente sostituisce soltanto il
blocco gestito e non riscrive il frontmatter.

### Blocco gestito

KubiRAG aggiorna solo il contenuto compreso tra:

```markdown
<!-- kubirag:concept:start -->
...
<!-- kubirag:concept:end -->
```

Il testo scritto sopra o sotto i marker viene preservato. Se un file esistente non contiene il
blocco, KubiRAG lo aggiunge in fondo senza eliminare il contenuto presente.

Non modificare manualmente il blocco gestito se vuoi conservare le modifiche: la successiva
rigenerazione lo sostituirà.

---

## Flusso consigliato

1. Carica note o documenti ed esegui l'enrichment, se necessario.
2. Scopri le evidence nelle sorgenti desiderate.
3. Accetta le evidence valide e rigetta il rumore.
4. Controlla il concept governato, le sorgenti e le relazioni.
5. Crea o aggiorna la concept note.
6. Completa la nota in Markboard o Obsidian, fuori dal blocco gestito.

Esempio CLI:

```text
/load-all /home/me/Obsidian/knowledge
/enrich
/evidence discover --scope all
/evidence list --status pending
/evidence accept ev-abc123
/concept list
/concept show rag evaluation
/concept note rag evaluation
/concept export
```

La discovery da sola non rende il concept governato: una evidence `pending` deve essere promossa.
In alternativa puoi creare esplicitamente il concept dalla CLI:

```text
/concept create rag evaluation --description Evaluation methods for retrieval-augmented generation
```

`/concept note` richiede un concept già presente in `_qb/qb-concepts.json`; non crea un concept
governato implicito.

---

## Uso da Ragboard

La UI separa la revisione delle evidence dalla gestione dei concept.

### Pannello Evidence

Il pannello **Evidence** permette di:

- scegliere la vista del notebook corrente o dell'intero workspace;
- cercare evidence in tutte le sorgenti, nelle sole note, nei documenti indicizzati o nel catalogo
  KubiTerm;
- aggiungere manualmente una URL come evidence `pending`;
- aprire la sorgente;
- accettare o rigettare una evidence.

Accettare una evidence la promuove nell'aggregate `_qb/qb-concepts.json`. La scansione del catalogo
KubiTerm è di sola lettura.

### Pannello Concepts

Il pannello **Concepts** offre una vista a lista e una mappa. La lista mostra conteggi di evidence,
alias, relazioni in uscita e in ingresso e backlinks Markdown. Il dettaglio permette di:

- creare, aggiornare e aprire la concept note in Markboard;
- aggiungere o rimuovere un collegamento manuale a una sorgente;
- inserire `[[concept]]` in una nota;
- aggiungere o rimuovere alias;
- aprire sorgenti e backlinks;
- navigare relazioni accettate in uscita e in ingresso;
- accettare o rigettare relazioni suggerite;
- rimuovere il concept governato.

La mappa supporta tre livelli:

- **Accepted relations**: relazioni governate;
- **Suggested relations**: aggiunge le relazioni candidate ancora pending;
- **Affinity**: per il concept selezionato aggiunge le affinità calcolate dalle sorgenti condivise.

I pulsanti **OKF export** e **OKF import** gestiscono bundle interoperabili. Non equivalgono alla
creazione delle normali concept note sotto `Concepts/`.

La UI non crea ancora un concept autoritativo vuoto: per farlo usa `/concept create` dalla CLI,
oppure promuovi una evidence.

---

## Uso da CLI

### Creare un concept governato

```text
/concept create rag evaluation --description Evaluation methods for retrieval-augmented generation
```

Se il nome o un suo alias esiste già, il comando non sostituisce la descrizione. Per aggiornarla:

```text
/concept create rag evaluation --description Updated description --overwrite
```

### Creare una relazione governata

Soggetto e oggetto devono già esistere:

```text
/concept relate rag evaluation --predicate measures --object faithfulness
```

La relazione è diretta e viene salvata per UUID nell'aggregate autoritativo.

### Rinominare un concept

```text
/concept rename rag evaluation --to rag quality evaluation
```

La rinomina mantiene invariato l'UUID e conserva il nome precedente come alias, così i wiki-link
esistenti continuano a risolversi. Se esiste una concept note, KubiRAG la sposta al nuovo nome,
aggiorna il campo `concept`, il titolo generato e il blocco gestito, preservando la prosa manuale.
La rinomina viene rifiutata se il nuovo nome è già usato da un altro concept o alias oppure se al
path di destinazione esiste già una concept note.

### Elencare i concept

```text
/concept list
/concept list --limit 100
```

Il limite predefinito CLI è 30. Se la lista è troncata, il comando mostra il totale e suggerisce il
valore necessario per visualizzare tutti i concept. L'ordinamento privilegia i concept più
connessi.

Per ogni concept vengono mostrati i conteggi di evidence, alias, backlinks dalle note, relazioni in
uscita e in ingresso, note sorgente e documenti sorgente.

### Mostrare il dettaglio

```text
/concept show rag evaluation
/concept rag evaluation
```

Il dettaglio include descrizione, evidence, collegamenti manuali, alias, backlinks Markdown,
relazioni accettate, relazioni in ingresso, suggerimenti e stato della concept note.

### Collegare e scollegare una sorgente

```text
/concept link rag evaluation --source doc-123 --relation supports
/concept link rag evaluation --source note-abc --type note --relation mentions --title Evaluation
/concept unlink rag evaluation --source doc-123
```

Il concept deve essere già governato. Il link viene registrato come evidence manuale
nell'aggregate `_qb/qb-concepts.json`; `unlink` rimuove il collegamento alla sorgente indicata, ma
non cancella la sorgente e non modifica i file Markdown.

### Alias

```text
/concept alias rag evaluation --alias rag eval
/concept unalias rag evaluation --alias rag eval
```

Gli alias sono attributi del concept governato. La risoluzione è case-insensitive; un alias viene
ricondotto al nome canonico anche nei collegamenti e nella ricerca dei backlinks.

Gli alias KubiRAG non modificano `_qb/qb-terms.json` e non sostituiscono la governance terminologica
di KubiTerm.

### Backlinks e inserimento di wiki-link

KubiRAG considera backlink una nota Markdown che contiene il wiki-link esatto al nome canonico o a
uno dei suoi alias, per esempio:

```markdown
[[rag evaluation]]
```

I backlinks vengono ricalcolati leggendo ricorsivamente la cartella delle note; non sono salvati
come relazioni governate. La concept note di un concept non viene contata come backlink di se
stessa.

Per inserire un link in una nota presente nella radice del notebook:

```text
/concept insert rag evaluation --note meeting-notes
```

Il comando usa un blocco separato:

```markdown
<!-- kubirag:concept-links:start -->
## Related Concepts

- [[rag evaluation]]
<!-- kubirag:concept-links:end -->
```

L'operazione è idempotente: non duplica un link già presente, aggiorna il blocco se esiste e
preserva il resto della nota. Il nome passato a `--note` identifica un file `.md` nella radice
delle note, non un percorso arbitrario.

### Revisionare relazioni suggerite

```text
/concept suggest
/concept suggest rag evaluation --limit 30
/concept accept ev-abc123
/concept reject ev-abc123
```

I suggerimenti sono evidence `pending` che contengono una relazione, ad esempio prodotta da
`aiRelations`. Accettare promuove la evidence nell'aggregate governato; la relazione tipizzata
viene creata quando entrambi i concept coinvolti esistono. Rigettare mantiene l'elemento nella
inbox con stato `rejected`.

Per promuovere una evidence generica puoi usare anche:

```text
/evidence accept ev-abc123
```

### Creare o aggiornare le note

```text
/concept note rag evaluation
/concept export
/concept export --limit 100
```

`note` materializza un singolo concept. `export` crea o aggiorna una nota per ogni concept
governato e, senza `--limit`, non applica un tetto implicito. Entrambi aggiornano soltanto il blocco
`kubirag:concept` dei file esistenti.

### Rimuovere i concept

```text
/concept remove rag evaluation
/concept clear --confirm
```

`remove` rigetta le evidence locali associate e rimuove il concept dall'aggregate insieme ai suoi
alias, alle evidence governate e alle relazioni collegate. `clear` svuota l'intero aggregate ed è
protetto da `--confirm`.

In entrambi i casi KubiRAG lascia intatte le concept note e i wiki-link nelle note sorgenti, perché
possono contenere lavoro editoriale dell'utente.

### Import ed export OKF

```text
/concept okf-export
/concept okf-export --dir target/kubirag-okf
/concept okf-import --dir target/kubirag-okf
```

L'export produce un bundle Open Knowledge Format 0.1 con un file per concept, stub delle sorgenti,
`index.md` e `log.md`. La directory predefinita è `okf/` accanto alla directory `notes/` del
notebook. L'import non sovrascrive concept note già presenti ed è idempotente. Nel runtime
autoritativo corrente non crea però concept mancanti: collega i file OKF a concept già governati e
riporta gli altri tra gli elementi `skipped`.

---

## API HTTP

Endpoint disponibili:

```text
GET    /concepts?limit=100&notebookId=<id>&boundId=<id>
GET    /concepts/graph?detail=accepted|suggested|affinity&focus=<id-or-name>
GET    /concepts/suggestions?concept=rag%20evaluation&limit=100
GET    /concepts/{concept}
GET    /concepts/{concept}/evidence
DELETE /concepts/{concept}

POST   /concepts/suggestions/{id}/accept
POST   /concepts/suggestions/{id}/reject
POST   /concepts/note
POST   /concepts/link
POST   /concepts/unlink
POST   /concepts/insert
POST   /concepts/alias
POST   /concepts/unalias
POST   /concepts/rename
POST   /concepts/export
POST   /concepts/okf/export
POST   /concepts/okf/import?dir=<server-path>
```

`GET /concepts` restituisce un array e comunica il totale non troncato nell'header
`X-Total-Count`. I parametri `notebookId` e `boundId` sono supportati dalla lista, dal dettaglio,
dall'elenco evidence del concept e dalla mappa. Per `detail=affinity`, `focus` è obbligatorio.

La creazione esplicita di concept e relazioni autoritative è attualmente disponibile dalla CLI con
`/concept create` e `/concept relate`; non esistono endpoint HTTP equivalenti.

Payload principali. Per creare o aggiornare una nota:

```json
{
  "concept": "rag evaluation"
}
```

Per collegare o scollegare una sorgente:

```json
{
  "concept": "rag evaluation",
  "sourceId": "doc-123",
  "relationType": "supports",
  "sourceType": "document",
  "sourceTitle": "Evaluation paper",
  "sourcePath": "/vault/sources/evaluation.md"
}
```

Per `unlink` sono necessari soltanto `concept` e `sourceId`.

Per inserire un wiki-link:

```json
{
  "concept": "rag evaluation",
  "noteName": "meeting-notes"
}
```

Per aggiungere o rimuovere un alias:

```json
{
  "concept": "rag evaluation",
  "alias": "rag eval"
}
```

Per rinominare un concept:

```json
{
  "concept": "rag evaluation",
  "newName": "rag quality evaluation"
}
```

L'export delle concept note accetta un limite opzionale:

```text
POST /concepts/export?limit=100
```

Senza `limit` esporta tutti i concept. Una risposta tipica è:

```json
{
  "total": 12,
  "created": 4,
  "updated": 8,
  "paths": [
    "_qb/notes/kubirag/notes/Concepts/rag evaluation.md"
  ]
}
```

---

## Pubblicare una Wiki navigabile

Le concept note del notebook e il sito Wiki sono due proiezioni diverse. Per produrre uno snapshot
Wiki portabile con permalink UUID:

```text
/wiki page-ids --apply
/wiki generate --content notes,concepts --output src/site/resources/wiki --overwrite
```

Il primo comando serve solo quando nuove note non hanno ancora un UUID e assegna quelli mancanti;
la generazione convalida comunque tutte le identità senza modificare le note. Il secondo comando
genera note, pagine concept, sidebar e `wiki-manifest.json`. Lo snapshot pubblicabile non contiene path
locali assoluti. Revisiona e versiona sia `_qb/qb-concepts.json` sia lo snapshot generato.

---

## Limiti attuali

- Una concept note è una proiezione: la prosa libera non viene riletta nell'aggregate governato.
- I backlinks Markdown affiancano le relazioni governate, ma non le sostituiscono.
- La creazione esplicita di concept e relazioni autoritative è disponibile solo dalla CLI; la
  rinomina è disponibile sia dalla CLI sia dal pannello Concepts.
- La rimozione di un concept non elimina automaticamente note o wiki-link, per evitare perdita di
  contenuto editoriale.
- L'integrazione con KubiTerm è intenzionalmente a senso unico per la discovery: KubiRAG legge i
  termini come candidate evidence senza modificarne lo store.
