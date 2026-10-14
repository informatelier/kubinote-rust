# Note Relation Editor Guide

Questa guida descrive il primo editor web Obsidian-like per relazioni e metadati delle note in
KubiRAG.

L'obiettivo e permettere una cura locale del knowledge graph Markdown senza introdurre un'ontologia
governata. KubiRAG modifica file `.md` in modo compatibile con Obsidian; KubiTerm resta il posto per
validare termini e relazioni formali.

---

## Cosa permette di editare

Da Ragboard, nella lista **Notes**, ogni nota Markdown ha un'azione **Relations**.

Il pannello mostra:

- metadati frontmatter semplici;
- wiki links presenti nella nota;
- wiki links gestiti da KubiRAG;
- backlinks da altre note;
- source links gestiti verso documenti/sorgenti RAG.

Puoi modificare:

- metadata come righe `key=value`;
- wiki links gestiti come chip, con autocompletamento sui nomi e inserimento libero;
- source links con relazione (`references`, `supports`, `contradicts`, `cites`, `explains`).

Quando salvi, KubiRAG aggiorna:

- il frontmatter iniziale della nota;
- un blocco Markdown gestito per link e source links.

Il contenuto libero della nota resta preservato.

---

## Blocco gestito

Le relazioni editate da Ragboard vengono scritte in un blocco controllato:

```markdown
<!-- kubirag:note-relations:start -->
## Note Relations

### Wiki Links

- [[Data Mesh]]
- [[Semantic Layer]]

### Source Links

- `supports` [paper.pdf](/sources/paper.pdf) - doc-1
<!-- kubirag:note-relations:end -->
```

KubiRAG puo riscrivere questo blocco quando salvi dal relation editor. Il testo fuori dal blocco non
viene riscritto.

Le sezioni vuote non vengono materializzate: in particolare, aggiungere soltanto un wiki link non
crea un `Source Link` vuoto.

---

## Metadati

Nel pannello metadata usa una riga per campo:

```text
status=draft
owner=data team
domain=analytics
```

Il salvataggio produce frontmatter Markdown:

```markdown
---
domain: analytics
owner: data team
status: draft
---
```

Questa prima versione supporta frontmatter semplice `key: value`. YAML complesso, liste annidate e
oggetti non sono ancora un target dell'editor visuale.

---

## Outgoing links

Come il pannello omonimo di Obsidian, **Outgoing links** mostra tutti i collegamenti interni letti
dalla nota. Wikilink (`[[Nota]]`) e link Markdown (`[testo](Nota.md)`) sono trattati allo stesso modo,
anche quando sono scritti manualmente fuori dal blocco gestito. Le destinazioni ripetute sono
raggruppate e mostrano il numero di occorrenze.

I link risolti aprono la nota, quelli non ancora creati avviano la creazione e quelli ambigui
mostrano i percorsi candidati. Embed, URL esterni e link presenti nel codice non fanno parte
dell'elenco.

La sezione separata **KubiRAG managed links** contiene invece la lista editabile che viene scritta
nel blocco controllato.

Questo significa:

- i link inline scritti manualmente non vengono rimossi;
- i link gestiti sono aggiunti e mantenuti nel blocco `Note Relations`;
- i link gestiti compaiono anche in Outgoing links, perché sono normali link Markdown;
- Obsidian li vede come link normali.

Il target puo includere il percorso relativo al notebook (`[[Filosofia/Schopenhauer]]`). Quando due
note hanno lo stesso nome, l'autocompletamento propone il percorso minimo che le distingue. Per
mostrare un testo diverso dal nome file si usa la sintassi Obsidian
`[[Filosofia/Arthur Schopenhauer|Schopenhauer]]`.

La rinomina da Markboard mostra prima il numero di link e note interessati. Dopo la conferma aggiorna
wikilink e link Markdown locali, preservando il testo visibile: `[[vecchio]]` diventa
`[[nuovo|vecchio]]`. I riferimenti ambigui devono essere resi espliciti prima della rinomina.

---

## Source Links

Un source link collega la nota a una sorgente RAG. Puoi inserire:

- document id;
- filename;
- path.

Se KubiRAG trova una sorgente corrispondente, normalizza il link usando id, filename e path noti.

Esempio:

```text
relation: supports
source: doc-123
```

Nel Markdown diventa:

```markdown
- `supports` [source-title](/path/to/source.pdf) - doc-123
```

Questi link sono locali alla nota e servono alla navigazione/editing Obsidian-like. Non sono ancora
relazioni ontologiche governate.

---

## API

```text
GET /notes/{name}/relations
PUT /notes/{name}/relations
POST /notes/{name}/rename
```

Payload:

```json
{
  "metadata": {
    "status": "draft",
    "owner": "data team"
  },
  "wikiLinks": ["Data Mesh", "Semantic Layer"],
  "sourceLinks": [
    {
      "sourceId": "doc-123",
      "relationType": "supports",
      "sourceTitle": "paper.pdf",
      "sourcePath": "/sources/paper.pdf"
    }
  ]
}
```

Per note in sottocartelle, gli endpoint accettano `?path=Cartella/Nota.md`. La rinomina e in due fasi:
la richiesta con `apply=false` restituisce anteprima e `planId`; la conferma usa
`{"newName":"Nuovo nome","apply":true,"planId":"..."}`. Se il notebook cambia nel frattempo,
il server restituisce `409` e richiede una nuova anteprima.

La risposta di `GET /notes/{name}/relations` include `outgoingLinks`, con destinazione, riferimento
canonico, stato (`resolved`, `missing`, `ambiguous`), numero di occorrenze, formati e possibili
percorsi candidati. I precedenti campi `wikiLinks` e `managedWikiLinks` restano disponibili per
compatibilita.

---

## Stato della feature

Disponibile ora:

- editor web metadati semplici;
- editor web wiki links gestiti;
- editor web note -> source links;
- lettura backlinks note -> note;
- vista Outgoing links per wikilink e link Markdown;
- blocco gestito non distruttivo.

Non ancora disponibile:

- editor visuale inline del Markdown;
- rimozione automatica di link inline non gestiti;
- ricerca e conversione delle unlinked mentions;
- YAML complesso;
- relazione typed note -> note separata dai wiki links;
- propagazione automatica verso KubiTerm.
