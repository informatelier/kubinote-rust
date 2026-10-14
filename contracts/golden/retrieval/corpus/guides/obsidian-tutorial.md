# Tutorial Obsidian + KubiRAG

Questa guida mostra come usare una vault Obsidian con KubiRAG per importare documenti Markdown,
interrogarli con il RAG locale, modificare note e riportare gli output in Obsidian.

KubiRAG non fornisce un plugin Obsidian nativo: la compatibilita avviene tramite file Markdown su
disco. Questo e sufficiente per lavorare con una vault reale, perche Obsidian legge e scrive file
`.md` ordinari.

---

## Cosa funziona

| Workflow | Supporto | Come |
|---|---:|---|
| Importare una vault | Si | `/load-all <vault>` indicizza tutti i `.md` |
| Importare una nota singola | Si | `/load <file.md>` |
| Rileggere modifiche fatte in Obsidian | Si | `/refresh` |
| Creare/modificare note Markdown | Si | comandi `/note-*` o editor web `/markboard` |
| Usare una cartella Obsidian come notebook KubiRAG | Si | `/store-link <vault> <nome>` |
| Tag, heading, link e backlink | Parziale | `GlobalIndex`, `/index`, `/tags`, `/navigate` |
| Export esplicito verso Obsidian | No | i file prodotti sono gia `.md` apribili in Obsidian |

---

## Prerequisiti

1. Una vault Obsidian esistente, per esempio:

```bash
/home/me/Obsidian/knowledge
```

2. KubiRAG compilato:

```bash
make build
```

3. Ollama attivo, se vuoi usare domande RAG e embedding:

```bash
ollama serve
```

---

## Avvio rapido da CLI

Avvia KubiRAG in modalita CLI:

```bash
make run-cli
```

Importa tutte le note Markdown della vault:

```text
/load-all /home/me/Obsidian/knowledge
```

Poi fai una domanda:

```text
Quali sono i concetti principali nei miei appunti su Kubernetes?
```

KubiRAG indicizza il contenuto dei file `.md`; Obsidian resta il tuo editor principale.

---

## Collegare una vault come notebook

Se vuoi che le note create da KubiRAG finiscano direttamente nella vault Obsidian, collega la vault
come store/notebook:

```text
/store-link /home/me/Obsidian/knowledge knowledge
```

Il comando crea un collegamento in `.qb/notes/knowledge`:

- su Linux/macOS usa un symlink;
- su Windows usa una directory junction;
- la cartella originale non viene copiata;
- `/store-unlink knowledge` rimuove solo il collegamento, non cancella la vault.

Dopo il link, lo store `knowledge` diventa attivo. Le note create da KubiRAG vengono scritte come
file `.md` nella root della vault collegata.

Puoi verificare gli store:

```text
/store
```

Oppure vedere i notebook collegati:

```text
/store-unlink
```

Senza argomento, `/store-unlink` elenca gli store e mostra quelli collegati.

---

## Importare i file della vault

Per indicizzare tutta la vault:

```text
/load-all /home/me/Obsidian/knowledge
```

`/load-all` carica ricorsivamente i file `.md`. KubiRAG salta la directory `.qb` e rispetta
`.qbignore` quando presente.

Per importare una nota singola:

```text
/load /home/me/Obsidian/knowledge/Projects/RAG.md
```

Per PDF o file testuali esterni:

```text
/load /home/me/Documents/paper.pdf
/load /home/me/Documents/notes.txt
```

I PDF possono essere spezzati in chunk di pagine durante l'import.

---

## Usare tag, heading e link in stile Obsidian

KubiRAG costruisce un indice globale dei documenti caricati. L'indice riconosce:

- heading Markdown `#`, `##`, `###`;
- tag inline come `#rag` o `#kubernetes`;
- tag YAML frontmatter come `tags: [rag, kubernetes]`;
- link Markdown locali come `[nota](altra-nota.md)`;
- link wiki-style come `[[altra nota]]`;
- riferimenti a UUID di documenti KubiRAG.

Comandi utili:

```text
/index
/tags
/tags kubernetes
/navigate <document-id>
```

`/index` mostra documenti, heading, conteggi di tag e link.

`/tags` elenca i tag trovati. Con un argomento mostra i documenti associati a quel tag.

`/navigate <document-id>` mostra backlink, forward link, tag e heading del documento.

Limite importante: i link Obsidian vengono usati per navigazione nell'indice KubiRAG, ma KubiRAG non
riscrive automaticamente i link nella vault.

---

## Modificare note

### Modifica da Obsidian

Apri la vault in Obsidian e modifica normalmente i file `.md`.

Poi, in KubiRAG, esegui:

```text
/refresh
```

`/refresh` rilegge i file sorgente con checksum cambiato e aggiorna gli indici.

### Modifica da KubiRAG CLI

Per creare una nota:

```text
/note-add idea-rag # Idea RAG
```

Per sovrascriverla:

```text
/note-edit idea-rag # Idea RAG aggiornata
```

Per aggiungere testo in fondo:

```text
/note-append idea-rag - nuovo punto
```

Per vedere le note:

```text
/note-list
/note-show idea-rag
```

Se lo store attivo e una vault collegata, questi comandi scrivono file `.md` direttamente dentro la
vault, quindi Obsidian li vede subito.

### Modifica da editor web

Avvia il server:

```bash
make run-server
```

Apri:

```text
http://localhost:7070/markboard
```

Markboard e l'editor Markdown integrato. Supporta anteprima live, Mermaid e formule LaTeX. Le note
salvate da Markboard sono file `.md` nello store attivo.

---

## Salvare risposte AI come note Obsidian

Dopo una conversazione in CLI, puoi salvare l'ultima risposta dell'assistente:

```text
/note-save-last sintesi-kubernetes
```

Oppure salvare tutta la chat:

```text
/note-save-chat ricerca-kubernetes-2026
```

Se stai usando uno store collegato alla vault, i file saranno visibili in Obsidian come:

```text
sintesi-kubernetes.md
ricerca-kubernetes-2026.md
```

Puoi poi aprirli, rinominarli, spostarli o collegarli da Obsidian.

---

## Usare le note come contesto RAG

Una nota creata in KubiRAG non diventa automaticamente contesto pinnato. Per indicizzarla e pinnarla:

```text
/note-pin sintesi-kubernetes
```

Questo comando:

1. legge il file `.md`;
2. lo aggiunge come sorgente RAG se non e gia presente;
3. lo aggiunge ai documenti pinned.

Per vedere i documenti pinned:

```text
/pinned
```

---

## Generare una wiki apribile in Obsidian

KubiRAG puo generare una wiki Markdown dai metadati di enrichment:

```text
/wiki generate
```

Output principale:

```text
wiki/
├── README.md
├── _sidebar.md
├── log.md
└── sources/
    ├── documento-a.md
    └── documento-b.md
```

La cartella `wiki/` contiene Markdown standard. Puoi aprirla direttamente come vault Obsidian o
copiarla dentro una vault esistente.

Nota: la wiki viene esclusa dall'indicizzazione RAG tramite `.qbignore`, per evitare che contenuto
generato dall'AI rientri come fonte primaria.

---

## Workflow consigliato

### 1. Obsidian come sorgente principale

```text
1. Scrivi e organizza note in Obsidian.
2. In KubiRAG: /load-all <vault>
3. Fai domande al RAG.
4. Modifica note in Obsidian.
5. In KubiRAG: /refresh
```

Questo e il workflow piu semplice: Obsidian resta il luogo di scrittura, KubiRAG e lo strato di
ricerca e domanda/risposta.

### 2. Vault collegata come notebook KubiRAG

```text
1. /store-link <vault> knowledge
2. /load-all <vault>
3. Usa /note-save-last o /note-save-chat per salvare output AI.
4. Apri le nuove note in Obsidian.
5. Usa /note-pin per promuovere note importanti a contesto RAG.
```

Questo workflow e piu integrato: KubiRAG puo scrivere direttamente nella vault.

### 3. Wiki separata

```text
1. Importa documenti.
2. Arricchisci i documenti con /enrich.
3. /wiki generate
4. Apri wiki/ in Obsidian o nel browser via /wiki/.
```

Questo workflow e utile quando vuoi un artefatto leggibile e versionabile generato dal corpus.

---

## Web UI

Con il server attivo:

```bash
make run-server
```

Apri:

```text
http://localhost:7070/ragboard
```

Da Ragboard puoi:

- importare file `.md`, `.txt`, `.pdf`;
- vedere e modificare note;
- creare cartelle note;
- aprire Markboard;
- pinnare note;
- generare wiki e altri artefatti.

Per l'editor diretto:

```text
http://localhost:7070/markboard
```

---

## Limiti e attenzioni

- Non esiste un plugin Obsidian nativo.
- KubiRAG non mantiene una sincronizzazione live con Obsidian: dopo modifiche esterne usa
  `/refresh`.
- `/load-all` indicizza i `.md` trovati nella directory, ma non importa asset Obsidian come immagini
  o allegati come contenuto RAG.
- I file generati da KubiRAG sono Markdown standard, ma non sempre usano convenzioni Obsidian come
  `[[link]]`.
- Prima di collegare una vault grande, valuta `.qbignore` per escludere cartelle non utili come
  archivi, template pesanti o note private.

Esempio `.qbignore` dentro la root della vault:

```gitignore
.obsidian/**
Templates/**
Archive/private/**
wiki/**
```

---

## Ricetta completa

Esempio con vault in `/home/me/Obsidian/knowledge`:

```text
/store-link /home/me/Obsidian/knowledge knowledge
/load-all /home/me/Obsidian/knowledge
/tags
/index
Quali note parlano di RAG locale e hardware piccolo?
/note-save-last rag-locale-hardware
/note-pin rag-locale-hardware
/refresh
```

Poi apri Obsidian: troverai `rag-locale-hardware.md` nella vault.
