# Evidence Guide

Questa guida spiega come usare le evidence di KubiRAG: suggerimenti locali estratti da note,
sorgenti e metadati di enrichment per alimentare il lavoro successivo in KubiTerm.

Le evidence sono il primo passo concreto verso un'esperienza Obsidian-like: KubiRAG scopre concetti
e relazioni dai contenuti Markdown e dalle sorgenti caricate, li rende revisionabili, e li esporta in
un formato strutturato. Non sono ancora il sistema completo di editing dei concept link.

---

## A cosa servono

Le evidence rispondono a domande operative come:

- quali concetti compaiono nelle mie note;
- quali `[[wiki-links]] Obsidian` sono presenti nei documenti caricati;
- quali entita e relazioni sono state suggerite dall'enrichment AI;
- quali suggerimenti sono utili per costruire o aggiornare l'ontologia in KubiTerm;
- quali suggerimenti sono rumore e vanno scartati.

KubiRAG resta il livello di discovery. La validazione semantica, la tassonomia governata,
l'ontologia e le relazioni ufficiali restano responsabilita di KubiTerm.

---

## Stato della funzionalita

| Funzionalita | Stato | Note |
|---|---:|---|
| Scoperta da tag Markdown | Disponibile | Include hashtag e tag riconosciuti da `TagExtractor` |
| Scoperta da `[[wiki-links]]` | Disponibile | Compatibile con note Obsidian gia linkate |
| Scoperta da `aiEntities` | Disponibile | Richiede documenti arricchiti con `/enrich` |
| Scoperta da `aiRelations` | Disponibile | Produce evidence con relazione suggerita |
| Inbox web per review | Disponibile | Pannello Evidence nella Ragboard |
| Reject dei suggerimenti rumorosi | Disponibile | Da web UI o CLI |
| Export JSON per KubiTerm | Disponibile | File `.qb/qb-evidence.json` |
| Creazione automatica concept note | Non ancora | Prevista dalla proposta concept links |
| Inserimento automatico `[[concept]]` nelle note | Non ancora | Prevista dalla proposta concept links |
| Editing grafico delle relazioni tra concetti | Non ancora | Appartiene soprattutto a KubiTerm |
| Validazione ontologica | Non in KubiRAG | Responsabilita KubiTerm |

---

## Flusso consigliato

1. Carica una vault Obsidian o una cartella di documenti.
2. Esegui enrichment se vuoi anche entita e relazioni AI.
3. Esegui discovery delle evidence.
4. Revisiona l'inbox Evidence.
5. Scarta i falsi positivi.
6. Esporta o lascia il file `.qb/qb-evidence.json` pronto per KubiTerm.

Esempio:

```text
/load-all /home/me/Obsidian/knowledge
/enrich
/evidence discover
/evidence list
/evidence reject ev-123456789abc
/evidence export
```

---

## Uso da Ragboard

Avvia il server:

```bash
make run-server
```

Apri la Ragboard nel browser e usa la card **Evidence** nello Studio o il pannello **Evidence**
nella colonna laterale.

Azioni disponibili:

- **Discover**: scandisce i documenti caricati e aggiorna l'inbox delle evidence.
- **Export**: rende disponibile il file evidence per il passaggio a KubiTerm.
- **Reject**: scarta una evidence rumorosa o non utile.

Il pannello mostra:

- concept suggerito;
- tipo di segnale, per esempio `tag`, `wiki-link`, `enrichment-entity`;
- confidenza stimata;
- titolo o path della sorgente;
- eventuale relazione suggerita.

La UI non modifica l'ontologia e non riscrive le note Markdown. Serve a fare triage dei suggerimenti
scoperti da KubiRAG.

---

## Uso da CLI

### Discovery

```text
/evidence discover
```

Scandisce i documenti attualmente caricati nel `DocumentStore` e aggiorna `.qb/qb-evidence.json`.
La discovery e idempotente: se rilanci il comando sugli stessi documenti, le evidence gia presenti
non vengono duplicate.

### Lista

```text
/evidence list
```

Mostra le evidence pending.

Puoi filtrare per stato:

```text
/evidence list --status pending
/evidence list --status accepted
/evidence list --status rejected
/evidence list --status exported
```

Puoi limitare il numero di risultati:

```text
/evidence list --limit 10
```

### Stato

```text
/evidence status
```

Mostra path del file e conteggi:

- totale;
- pending;
- accepted;
- exported;
- rejected.

### Reject

```text
/evidence reject <evidence-id>
```

Marca una evidence come `rejected`. Questo e utile quando un tag, un link o un'entita AI produce
rumore.

Le evidence marcate `accepted` sono quelle accettate dalla review dei concept con:

```text
/concept accept <evidence-id>
```

In quel caso KubiRAG crea un link manuale concept -> sorgente e la evidence esce dall'inbox pending.

### Export

```text
/evidence export
```

Senza path esplicito, usa il file standard:

```text
.qb/qb-evidence.json
```

Con path esplicito, scrive un file separato con le evidence pending:

```text
/evidence export /home/me/kubiterm/inbox/qb-evidence.json
```

Quando esporti verso un file diverso da quello standard, le evidence pending vengono marcate come
`exported`.

---

## Formato del file

Il file evidence e JSON leggibile, versionato in modo semplice:

```json
{
  "version": "1",
  "producer": "kubirag",
  "evidence": [
    {
      "id": "ev-123456789abc",
      "concept": "rag evaluation",
      "relation": {
        "subject": "RAG notes",
        "predicate": "related",
        "object": "rag evaluation"
      },
      "sourceType": "note",
      "sourceId": "doc-id",
      "sourcePath": "/home/me/Obsidian/knowledge/RAG.md",
      "sourceTitle": "RAG",
      "snippet": "Short supporting text...",
      "confidence": 0.72,
      "status": "pending",
      "signal": "enrichment-relation",
      "createdAt": "2026-06-13T09:15:00Z"
    }
  ]
}
```

Campi principali:

- `concept`: concetto candidato.
- `relation`: relazione candidata, se disponibile.
- `sourceType`: `note` o `document`.
- `sourcePath`: path locale della sorgente.
- `snippet`: evidenza testuale a supporto.
- `confidence`: stima tecnica del segnale, non validazione semantica.
- `status`: `pending`, `exported`, `rejected`.
- `signal`: origine del suggerimento.

---

## Relazione con Obsidian

Le evidence sono Obsidian-compatible perche leggono segnali Markdown gia presenti nella vault:

- tag;
- link wiki-style `[[concept]]`;
- testo delle note;
- metadati prodotti da KubiRAG.

Pero non sono ancora l'esperienza completa "edit by concept link" tipica di Obsidian.

Gia disponibile:

- KubiRAG riconosce `[[concept]]`;
- KubiRAG trasforma link, tag e enrichment in evidence;
- l'utente puo revisionare e scartare suggerimenti;
- il risultato puo alimentare KubiTerm.

Ancora da implementare:

- aprire una vista concept con tutte le note e sorgenti collegate;
- collegare o scollegare manualmente una nota a un concept;
- creare o aggiornare concept note Markdown;
- inserire in modo assistito `[[concept]]` dentro una nota;
- mostrare backlink e sorgenti nella stessa UI di editing del concept;
- sincronizzare decisioni validate da KubiTerm verso KubiRAG.

Quindi questa funzionalita e un primo passo solido, non la feature Obsidian-like completa.

---

## Caso d'uso

Una persona sta studiando data mesh in una vault Obsidian. Ha note sparse:

- `Data Mesh.md`;
- `Domain Ownership.md`;
- `Semantic Layer.md`;
- appunti importati da PDF e articoli.

Nelle note ci sono tag come `#data-mesh` e link come `[[domain ownership]]`.
Dopo `/enrich`, KubiRAG aggiunge anche entita e relazioni suggerite.

L'utente esegue:

```text
/evidence discover
```

Nel pannello Evidence vede suggerimenti come:

- `data mesh` da tag;
- `domain ownership` da wiki-link;
- `semantic layer` da enrichment;
- relazione `Data Mesh related domain ownership`.

L'utente scarta i suggerimenti deboli e lascia pending quelli utili. KubiTerm potra poi leggere le
evidence e trasformarle in termini governati, relazioni tipate e vincoli ontologici.

In una fase successiva, KubiRAG potra usare lo stesso insieme di evidence per creare una pagina
Markdown `Concepts/data mesh.md` con link alle note e alle sorgenti, mantenendo il workflow
compatibile con Obsidian.

---

## Quando usare evidence e quando usare KubiTerm

Usa KubiRAG Evidence quando:

- vuoi scoprire concetti dai materiali locali;
- vuoi raccogliere prove testuali;
- vuoi fare triage veloce dei suggerimenti;
- vuoi mantenere il workflow Markdown/Obsidian.

Usa KubiTerm quando:

- vuoi decidere il nome canonico di un termine;
- vuoi gestire sinonimi e definizioni ufficiali;
- vuoi modellare relazioni semantiche;
- vuoi validare o pubblicare un'ontologia;
- vuoi governare la rete concettuale.

La separazione intenzionale e questa:

```text
KubiRAG  -> evidence, note graph, sorgenti, Obsidian compatibility
KubiTerm -> termini validati, relazioni governate, ontologie
```
