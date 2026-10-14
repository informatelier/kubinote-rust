# Guida all'Installazione di KubiRAG

Questa guida ti accompagna dall'installazione al primo utilizzo di **KubiRAG**, passo dopo passo. È pensata per l'utente finale: non serve essere sviluppatori.

## Cos'è KubiRAG

KubiRAG è un assistente che risponde alle tue domande **basandosi sui tuoi documenti** (RAG, *Retrieval-Augmented Generation*). Gira **in locale** sul tuo computer appoggiandosi a [Ollama](https://ollama.com) per il modello di intelligenza artificiale: i tuoi documenti e le tue domande **non lasciano la tua macchina**.

Puoi usarlo in due modi:

- **CLI** — un terminale interattivo in cui carichi documenti e fai domande.
- **Server web** — un'interfaccia browser sulla porta `7070`.

## Indice

- [Cosa ti serve](#cosa-ti-serve)
- [Passo 1 — Installa Java 21](#passo-1--installa-java-21)
- [Passo 2 — Installa Ollama e scarica un modello](#passo-2--installa-ollama-e-scarica-un-modello)
- [Passo 3 — Ottieni KubiRAG](#passo-3--ottieni-kubirag)
- [Passo 4 — Avvia KubiRAG](#passo-4--avvia-kubirag)
- [Passo 5 — Verifica che funzioni](#passo-5--verifica-che-funzioni)
- [Dove vengono salvati i tuoi dati](#dove-vengono-salvati-i-tuoi-dati)
- [Risoluzione problemi](#risoluzione-problemi)
- [Prossimi passi](#prossimi-passi)

---   

## Cosa ti serve

| Componente | Versione | A cosa serve |
|------------|----------|--------------|
| **Java**   | 21       | Esegue KubiRAG. |
| **Ollama** | recente  | Fornisce il modello AI in locale. |
| **KubiRAG**| —        | Il file applicazione (`.jar`). |

> KubiRAG è distribuito come **fat JAR** specifico per piattaforma (Windows, Linux x64, Linux ARM64). Assicurati di usare il JAR corretto per il tuo sistema operativo — vedi [Passo 3](#passo-3--ottieni-kubirag).

---

## Passo 1 — Installa Java 21

KubiRAG richiede **Java 21**. Se non sai se è installato, apri un terminale ed esegui:

```bash
java -version
```

Se vedi una versione `21.x.x` sei a posto. Altrimenti installa una distribuzione OpenJDK 21, ad esempio [Eclipse Temurin](https://adoptium.net/temurin/releases/?version=21):

- **Windows** — scarica l'installer `.msi` da Adoptium e segui la procedura guidata.
- **macOS** — `brew install --cask temurin@21` (con [Homebrew](https://brew.sh)).
- **Linux (Debian/Ubuntu)** — `sudo apt install openjdk-21-jre`.

Verifica di nuovo con `java -version`.

---

## Passo 2 — Installa Ollama e scarica un modello

KubiRAG ha bisogno di un modello AI servito da **Ollama**.

1. **Installa Ollama** dal sito ufficiale: <https://ollama.com/download> (Windows, macOS, Linux).

2. **Avvia Ollama.** Su Windows/macOS parte automaticamente in background. Su Linux:
   ```bash
   ollama serve
   ```
   Ollama ascolta su `http://localhost:11434`.

3. **Scarica un modello** (la prima volta richiede qualche minuto):
   ```bash
   ollama pull llama3
   ```

> Puoi usare qualsiasi modello disponibile in Ollama (`llama3`, `mistral`, `qwen2`, …). Lo selezionerai in KubiRAG con il comando `/model`.

---

## Passo 3 — Ottieni KubiRAG

Hai due possibilità.

### A) Hai ricevuto il file `.jar` (consigliato per l'utente finale)

Salva il file in una cartella a tua scelta, ad esempio `kubirag/`. Il nome sarà simile a:

```
kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar
```

> **Importante:** il JAR contiene librerie native specifiche del sistema operativo. Usa il JAR costruito per la **tua** piattaforma (Windows / Linux x64 / Linux ARM64). Un JAR Windows non funziona su Linux e viceversa.

### B) Vuoi costruirlo dai sorgenti

Servono Java 21 e Maven 3. Nella cartella del progetto:

```bash
make build
```

Il JAR verrà creato in `target/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar`, con i nativi della piattaforma su cui costruisci. Per dettagli su build per piattaforme diverse e deploy Docker, vedi `doc/manuals/docker-deploy-manual.md`.

---

## Passo 4 — Avvia KubiRAG

Apri un terminale nella cartella che contiene il `.jar`.

### Modalità CLI (terminale interattivo)

```bash
java --add-modules jdk.incubator.vector -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar
```

Dopo qualche secondo comparirà il prompt di KubiRAG:

```
>
```

### Modalità Server web

```bash
java --add-modules jdk.incubator.vector -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server
```

Apri poi il browser su <http://localhost:7070>.

Per importare documenti all'avvio, usa `--load`. Una directory viene letta solo al primo livello;
aggiungi `--recursive` per includere le sottodirectory. I formati predefiniti sono `.md`, `.txt` e
`.pdf`:

```bash
java --add-modules jdk.incubator.vector \
  -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server \
  --load ./docs --recursive
```

`--load` può essere ripetuto. Per includere codice sorgente, specifica le estensioni consentite:

```bash
java --add-modules jdk.incubator.vector \
  -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server research \
  --load ./src --recursive --include md,java,kt,py --port 7071
```

I riavvii sono idempotenti: i file invariati vengono saltati e quelli modificati aggiornano i documenti
esistenti conservando, quando possibile, identificativi e pin. La scansione rispetta `.qbignore` oppure
`.gitignore`.

> Il parametro `--add-modules jdk.incubator.vector` abilita le ottimizzazioni vettoriali ed è necessario all'avvio.
>
> Se hai i sorgenti, puoi usare anche le scorciatoie `make cli` e `make server`.

---

## Passo 5 — Verifica che funzioni

Se hai avviato la **CLI**, dopo qualche secondo vedi il prompt:

```
>
```

A questo punto l'installazione è completa: KubiRAG è in esecuzione e collegato a Ollama. Puoi chiudere la sessione digitando `exit` (o `Ctrl+C`).

Se hai avviato il **Server web**, apri <http://localhost:7070> nel browser: dovresti vedere l'interfaccia di KubiRAG.

> **Come si usa adesso?** Caricare documenti, fare domande e gestire gli archivi sono argomenti del *quickstart* (in arrivo). Nel frattempo, le guide elencate in [Prossimi passi](#prossimi-passi) coprono i comandi disponibili.

---

## Dove vengono salvati i tuoi dati

I progetti recenti separano stato condivisibile e stato locale:

| Percorso | Contenuto |
|----------|-----------|
| `qb-project.json` | Bootstrap condiviso che seleziona la directory autoritativa. |
| `_qb/` | Manifest degli aggregati e governance destinati alla condivisione. |
| `.qb/` | Configurazione locale, backend, sessioni, cache, indici, log e recovery. |

Il valore predefinito di `authorityDirectory` è `_qb`. `.qb` resta normalmente ignorata da Git; non
copiarne indiscriminatamente il contenuto in `_qb`.

KubiTerm instrada automaticamente gli aggregati condivisibili `qb-terms.json` e `qb-facts.json`
verso la directory autoritativa. Se trova le vecchie copie sotto `.qb`, le valida, ne conserva una
copia byte-esatta in `.qb/.migration-checkpoints/authority-relocation/`, le pubblica sotto `_qb` e
rimuove le sorgenti legacy. Se entrambe le copie esistono ma divergono, si arresta con
`QB_AUTHORITATIVE_ARTIFACT_CONFLICT` senza modificarle: la riconciliazione deve essere esplicita.

Se un progetto precedente si arresta con `QB_UNSUPPORTED_PROTOTYPE_LAYOUT`, seguire la
[guida alla migrazione del workspace prototipo](prototype-workspace-migration-guide.md).

`QB_ROOT_PATH` cambia la root locale. Gli override della root autoritativa sono destinati a diagnostica e
test; per un progetto condiviso usare `qb-project.json`.

---

## Risoluzione problemi

| Problema | Causa probabile | Soluzione |
|----------|-----------------|-----------|
| `UnsupportedClassVersionError` all'avvio | Java troppo vecchio | Installa **Java 21** (Passo 1) e verifica con `java -version`. |
| Le risposte non arrivano / errore di connessione | Ollama non è in esecuzione o non raggiungibile | Avvia Ollama (`ollama serve`) e verifica che risponda su `http://localhost:11434`. |
| `model not found` | Il modello non è stato scaricato | Esegui `ollama pull <nome-modello>` e seleziona lo stesso nome con `/model`. |
| `UnsatisfiedLinkError` o errori sui file nativi (`.dll` / `.so`) | JAR di una piattaforma diversa dalla tua | Usa il JAR costruito per il tuo sistema operativo (vedi [Passo 3](#passo-3--ottieni-kubirag)). |
| La porta `7070` è occupata (modalità server) | Un altro processo usa la porta | Chiudi l'altro processo o avvia su un'altra porta: `... server --port 7071`. |
| Ollama è su un'altra macchina/porta | URL diverso dal default | Imposta `OLLAMA_BASE_URL` (es. `http://192.168.1.10:11434/api`) prima di avviare. |
| `QB_UNSUPPORTED_PROTOTYPE_LAYOUT` | Progetto creato col vecchio layout autoritativo sotto `.qb` | Non spostare i file a mano: segui la [guida di migrazione](prototype-workspace-migration-guide.md). |

---

## Prossimi passi

Una volta installato, approfondisci le funzionalità:

- `doc/guides/voice-role-ask-guide.md` — comandi vocali, ruoli e domande all'AI.
- `doc/guides/tuning-guide.md` — messa a punto della ricerca e delle risposte.
- `doc/guides/ralph-loop-quickstart.md` — esecuzione automatica di task.
- `doc/guides/prototype-workspace-migration-guide.md` — adozione esplicita di un vecchio progetto `.qb`.
- `doc/manuals/docker-deploy-manual.md` — deploy avanzato (Docker, multi-piattaforma, server).
