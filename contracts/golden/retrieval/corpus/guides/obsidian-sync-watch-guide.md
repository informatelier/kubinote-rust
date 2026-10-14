# Obsidian Sync Watch Guide

KubiRAG ora usa un meccanismo a due layer per lavorare con file Markdown modificati fuori dalla UI,
per esempio da Obsidian.

```text
watch leggero -> marca note dirty
refresh manuale -> rilegge, reindicizza, ricrea pending evidence
```

Il watcher non promuove relazioni, non valida ontologie e non sovrascrive note. Serve solo a
intercettare che qualcosa e cambiato.

---

## Ragboard

Nel pannello **Notes** sono disponibili:

- **Watch**: avvia il polling leggero dei file `.md`;
- **Stop watch**: ferma il polling;
- **Scan**: forza un controllo immediato;
- **Refresh changed**: esegue il refresh manuale sui cambiamenti rilevati.

Quando KubiRAG rileva note modificate, mostra un badge:

```text
N changed
```

e una lista breve delle note dirty.

---

## CLI

Il comando:

```text
/refresh
```

ora rilegge i documenti modificati, reindicizza e ricrea le evidence pending.

Il comportamento resta non distruttivo:

- le evidence `accepted` restano accepted;
- i link manuali restano invariati;
- gli alias restano invariati;
- le note Markdown non vengono riscritte dal refresh.

---

## API

```text
GET  /notes/watch/status
POST /notes/watch/start
POST /notes/watch/stop
POST /notes/watch/scan
POST /notes/watch/refresh
```

`/notes/watch/refresh` esegue:

1. refresh dei documenti indicizzati;
2. reindex dei documenti cambiati;
3. evidence discovery sul document store aggiornato;
4. reset della lista dirty.

---

## Perche non aggiornare tutto automaticamente

Obsidian puo salvare spesso e produrre piu eventi ravvicinati. Inoltre una modifica Markdown e una
fonte derivata, non una decisione semantica governata.

Per questo KubiRAG separa:

- **detection**: automatica e leggera;
- **recompute**: esplicito;
- **curation**: manuale tramite link, accept/reject e concept editor.
