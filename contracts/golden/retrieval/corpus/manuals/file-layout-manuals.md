# File and directory layouts

## il file radice: qb-project.json

## file di lavoro: .qb

Contiene configurazione, cache, sessioni, lock e recovery locali. È normalmente ignorata da Git.
Le copie legacy di `qb-terms.json` e `qb-facts.json` vengono migrate automaticamente; i byte
originali restano in `.qb/.migration-checkpoints/authority-relocation/`.

## file da versionare: _qb

È la directory autoritativa scelta da `qb-project.json` (predefinita `_qb`) e contiene gli aggregati
collaborativi, inclusi `qb-terms.json`, `qb-facts.json` e `qb-vocabularies.json`. Versionare
`qb-project.json` e `_qb/`, non l'intera `.qb/`.

### Notebook RAG

Ogni notebook completo vive in `_qb/notes/<notebook>/`:

```text
<notebook>-manifest.json
notes/       # Markdown modificabile e versionabile
assets/      # immagini editoriali gestite
resources/   # copie portabili delle sorgenti locali indicizzate
```

Documenti JSON, embedding e ArcadeDB restano in `.qb/notes/<notebook>/` e sono ricostruibili.
Wiki, slide HTML e libri PDF vengono invece generati in una cartella relativa al workspace indicata
dall'utente (`wiki/`, `slides/` e `books/` sono i valori predefiniti). `_qb/` non è una destinazione
ammessa: un output diventa versionato solo quando l'utente sceglie e committa esplicitamente una
cartella di pubblicazione. L'ultima destinazione di slide, libri e proiezioni Markdown delle note è
un binding macchina-locale sotto `.qb/rag/notebooks/publish-bindings/`, usato esclusivamente per
generare o servire l'output e mai come autorità del notebook.

Le stesse destinazioni sono disponibili da terminale:

```text
/wiki generate --output public/wiki
/slides generate --output public/slides
/book generate --output public/books --title "Project handbook"
```

Un checkout che contiene solo `_qb` può quindi elencare e attivare i notebook; le proiezioni locali
vengono rigenerate dal manifest.

Il layout notebook precedente non viene migrato automaticamente: nella fase interna
`1.0.0-SNAPSHOT` i notebook di test vanno ricreati o importati con `Import folder`/`/store-import`.
I bundle Share usano esclusivamente `kubi-notebook-bundle/v2` e non includono le proiezioni locali.

## file di persistenza aggregati: contratti interni in essere

## migrazioni di aggregati

La rilocazione da `.qb` a `_qb` è automatica, bloccata da lock, ripetibile e accompagnata da un
checkpoint byte-esatto. Se sorgente e destinazione divergono, `QB_AUTHORITATIVE_ARTIFACT_CONFLICT`
interrompe l'avvio senza sovrascrivere o cancellare alcun file.

## bounds di aggregati

## file in formato standard: import/export
