# Manuale Deploy Kubirag (Docker e JAR nativo)

Questa guida descrive come costruire, pubblicare ed eseguire Kubirag per le diverse piattaforme supportate. Esistono **due metodi di deploy**:

- **Container Docker** — solo per **Linux** (`linux/amd64`, `linux/arm64`): le immagini Docker in questo progetto sono sempre Linux.
- **Fat JAR nativo** (`java -jar`) — per qualsiasi host, incluso **Windows**: è la via di deploy per le piattaforme non-Linux.

Tutti i comandi sono esposti tramite il `Makefile` alla radice del progetto.

## Indice

- [Panoramica](#panoramica)
- [Perché il deploy è platform-specific](#perché-il-deploy-è-platform-specific)
- [Piattaforme supportate](#piattaforme-supportate)
- [Prerequisiti](#prerequisiti)
- [Deploy via Docker (Linux)](#deploy-via-docker-linux)
  - [Build dell'immagine](#build-dellimmagine)
    - [linux/amd64 (default)](#linuxamd64-default)
    - [linux/arm64](#linuxarm64)
  - [Pubblicazione su registry](#pubblicazione-su-registry)
  - [Manifest multi-arch](#manifest-multi-arch)
  - [Esecuzione del container](#esecuzione-del-container)
- [Deploy senza Docker (fat JAR nativo)](#deploy-senza-docker-fat-jar-nativo)
  - [Windows x64](#windows-x64)
  - [Linux x64 / ARM64 bare-metal](#linux-x64--arm64-bare-metal)
  - [Note sul cross-build](#note-sul-cross-build)
- [Configurazione: variabili d'ambiente e volumi](#configurazione-variabili-dambiente-e-volumi)
- [Verifica del deploy](#verifica-del-deploy)
- [Risoluzione problemi](#risoluzione-problemi)

---

## Panoramica

Kubirag viene distribuito come **fat JAR** (con tutte le dipendenze) impacchettato in un'immagine basata su `eclipse-temurin:21-jre-jammy`. Il server espone l'API RAG sulla porta `7070`.

Il flusso di deploy ha tre passaggi, tutti orchestrati dal `Makefile`:

1. **Build del JAR** coerente con l'architettura di destinazione (`docker-jar`, invocato automaticamente).
2. **Build dell'immagine** Docker per quella piattaforma (`docker-build`).
3. **Pubblicazione** sul registry e, opzionalmente, composizione di un manifest multi-arch (`docker-push`, `docker-manifest`).

---

## Perché il deploy è platform-specific

Kubirag include due famiglie di **librerie native** selezionate **a build time** e impacchettate dentro il fat JAR:

- **sherpa-onnx** — riconoscimento e sintesi vocale (ASR/TTS).
- **Bytedeco OpenBLAS / arpack / javacpp** — backend di algebra lineare usato da SMILE (clustering K-Means per le mindmap).

Per contenere il peso del JAR, il `pom.xml` impacchetta **solo i nativi della piattaforma di build** (non tutte le piattaforme). La conseguenza diretta:

> ⚠️ **L'architettura dell'immagine Docker deve combaciare con i nativi presenti nel JAR.**
> Un JAR costruito con i nativi `linux-x86_64` infilato in un'immagine `arm64` non caricherà correttamente le librerie native.

Il `Makefile` garantisce questa coerenza: il target `docker-build` **ricostruisce il JAR** con i classifier giusti per la `PLATFORM` richiesta, poi costruisce l'immagine per la stessa architettura tramite `docker buildx`.

---

## Piattaforme supportate

| Target            | Metodo di deploy        | `sherpa.classifier` | `bytedeco.classifier`          | Note |
|-------------------|-------------------------|---------------------|--------------------------------|------|
| Linux x86_64      | Docker (`linux/amd64`) **o** JAR | `linux-x64`       | `linux-x86_64`                 | Default. Accelerazione OpenBLAS nativa completa. Suffisso tag immagine: `-amd64`. |
| Linux ARM64       | Docker (`linux/arm64`) **o** JAR | `linux-aarch64`   | `linux-x86_64` *(placeholder)* | Bytedeco non pubblica OpenBLAS `linux-arm64` per questa release di SMILE: il JAR imbarca il nativo x86_64 come segnaposto e **SMILE ricade sul backend pure-JVM**. La voce usa il nativo `linux-aarch64` corretto. Suffisso tag: `-arm64`. |
| Windows x64       | **Solo JAR** (`java -jar`) | `win-x64`        | `windows-x86_64`               | Niente Docker: le immagini sono Linux. Il JAR include le DLL native (OpenBLAS + sherpa + runtime MSVC); pesa ~147 MB. |
| macOS (x86_64/arm64) | **Non supportato**   | —                   | `macosx-x86_64` / `macosx-arm64` | Bytedeco ha i nativi macOS, ma sherpa-onnx in questa configurazione **non fornisce un nativo macOS**: la voce non funzionerebbe. Non documentato come target di deploy. |

> I target **Docker** del `Makefile` accettano solo `linux/amd64` e `linux/arm64`; un `PLATFORM` diverso (es. `windows/amd64`) interrompe con un errore esplicito. Per Windows si usa il fat JAR — vedi [Deploy senza Docker](#deploy-senza-docker-fat-jar-nativo).

---

## Prerequisiti

- **Docker** recente con **buildx** abilitato (incluso di default in Docker Desktop e nelle versioni moderne di Docker Engine). Verifica:
  ```bash
  docker buildx version
  ```
- **Java 21** e **Maven 3** sull'host (necessari alla build del JAR).
- Il progetto sibling **`kubicommons`** installato nel repo Maven locale (`mvn install` in `../kubicommons`), come da `CLAUDE.md`.
- Per il cross-build ARM da un host x86: il builder buildx deve poter emulare l'architettura target (QEMU). Su Docker Desktop è preconfigurato; su Linux puro:
  ```bash
  docker run --privileged --rm tonistiigi/binfmt --install all
  ```

---

## Deploy via Docker (Linux)

Questa sezione copre i target Linux distribuiti come immagini Docker. Per Windows e altri host non-Docker vedi [Deploy senza Docker](#deploy-senza-docker-fat-jar-nativo).

### Build dell'immagine

Il target `docker-build` esegue in sequenza: ricostruzione del fat JAR con i classifier della piattaforma → `docker buildx build` per quella stessa piattaforma → tag per-architettura → caricamento nel daemon locale (`--load`).

#### linux/amd64 (default)

```bash
make docker-build
```

Equivale a `make docker-build PLATFORM=linux/amd64`. Produce le immagini:

- `informatelier/kubirag:v1-amd64`
- `informatelier/kubirag:latest-amd64`

#### linux/arm64

```bash
make docker-build PLATFORM=linux/arm64
```

Produce:

- `informatelier/kubirag:v1-arm64`
- `informatelier/kubirag:latest-arm64`

> Il JAR viene ricostruito ad ogni `docker-build` con i `-D` corretti, quindi puoi costruire l'immagine ARM anche da un host x86: l'assemblaggio del JAR scarica e impacchetta solo i classifier nativi del target, non esegue codice di quell'architettura.

---

### Pubblicazione su registry

Dopo la build, pubblica i tag per-architettura sul Docker Hub (utente `informatelier`):

```bash
make docker-push                        # tag -amd64
make docker-push PLATFORM=linux/arm64   # tag -arm64
```

> Assicurati di aver effettuato il login al registry: `docker login`.

---

### Manifest multi-arch

Per esporre un singolo tag (`:latest`, `:v1`) che seleziona automaticamente l'architettura corretta al `pull`, combina le immagini per-arch — **già costruite e pushate** — in un manifest multi-arch:

```bash
# 1. costruisci e pubblica entrambe le architetture
make docker-build docker-push PLATFORM=linux/amd64
make docker-build docker-push PLATFORM=linux/arm64

# 2. componi e pubblica il manifest combinato
make docker-manifest
```

`docker-manifest` crea e pubblica:

- `informatelier/kubirag:v1` → punta a `v1-amd64` + `v1-arm64`
- `informatelier/kubirag:latest` → punta a `latest-amd64` + `latest-arm64`

> Questo approccio (immagini separate + manifest a valle) è necessario perché il fat JAR è mono-architettura: una build multi-platform a passata unica impacchetterebbe lo stesso JAR in entrambe le immagini, risultando errata per una delle due.

---

### Esecuzione del container

Il target `docker-run` avvia il server con i volumi e le variabili d'ambiente predefiniti, usando l'immagine della piattaforma corrente:

```bash
make docker-run                        # esegue :latest-amd64
make docker-run PLATFORM=linux/arm64   # esegue :latest-arm64
```

Comando equivalente esplicito:

```bash
docker run -d \
  --name kubirag \
  -p 7070:7070 \
  -e OLLAMA_BASE_URL=http://host.docker.internal:11434/api \
  -v kubirag-data:/data/qb \
  -v kubirag-output:/data/output \
  -v kubirag-logs:/var/log/kubirag \
  informatelier/kubirag:latest-amd64
```

Per fermare e rimuovere il container:

```bash
make docker-stop
```

---

## Deploy senza Docker (fat JAR nativo)

Per le piattaforme **non coperte da Docker** — in primis **Windows** — il deploy è il fat JAR eseguito direttamente con `java -jar`. Il principio è identico: il JAR va costruito con i classifier nativi del target, ma in questo caso si esegue Maven direttamente (i target Docker del `Makefile` non gestiscono Windows).

Il punto chiave: **assemblare un JAR per un'altra piattaforma non richiede di eseguirlo lì**. Maven scarica e impacchetta i classifier nativi richiesti senza eseguirne il codice, quindi puoi cross-compilare il JAR Windows da un host Linux (e viceversa) passando entrambi i classifier come `-D` (le system property hanno precedenza sui profili auto-attivati per OS).

### Windows x64

**1. Costruisci** il fat JAR con i nativi Windows (da qualsiasi host con Java 21 + Maven):

```bash
mvn clean package -DskipTests \
  -Dsherpa.classifier=win-x64 \
  -Dbytedeco.classifier=windows-x86_64
```

Produce `target/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar` (~147 MB) contenente le DLL native: `jniopenblas.dll`, `jnijavacpp.dll`, le runtime MSVC (`vcruntime140.dll`, `concrt140.dll`, `api-ms-win-*`) e `onnxruntime.dll` (sherpa).

**2. Copia** il JAR sull'host Windows e **esegui** con il modulo vettoriale incubator (come nel container):

```powershell
# CLI interattiva
java --add-modules jdk.incubator.vector -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar

# Server web (porta 7070)
java --add-modules jdk.incubator.vector -jar kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server
```

Requisiti runtime: **Java 21** sull'host Windows e **Ollama** raggiungibile (default `http://localhost:11434/api`). Le directory di stato seguono `QB_ROOT_PATH` / `QB_OUTPUT_PATH` / `QB_LOG_PATH` (vedi [Configurazione](#configurazione-variabili-dambiente-e-volumi)); senza override usano la cartella `.qb/` locale.

### Linux x64 / ARM64 bare-metal

Stesso schema, senza container. Su un host che combacia con quello di build basta `make build` (i profili auto-attivati per OS scelgono i classifier corretti), poi:

```bash
java --add-modules jdk.incubator.vector -jar target/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar server
```

Per cross-compilare un JAR Linux ARM64 da un host x86, forza i classifier:

```bash
mvn clean package -DskipTests \
  -Dsherpa.classifier=linux-aarch64 \
  -Dbytedeco.classifier=linux-x86_64   # placeholder: su ARM Smile usa il fallback JVM
```

### Note sul cross-build

- **Imposta sempre entrambi** i classifier (`sherpa.classifier` *e* `bytedeco.classifier`): selezionarne uno solo lascerebbe l'altra famiglia di nativi sul default dell'host, producendo un JAR misto e non funzionante sul target.
- **Verifica** quali nativi sono finiti nel JAR prima di distribuirlo:
  ```bash
  unzip -l target/kubirag-1.0.0-SNAPSHOT-jar-with-dependencies.jar \
    | grep -iE "\.(so|dll|dylib)$" | grep -iE "openblas|sherpa|onnxruntime"
  ```
- **Ripristina** il JAR host-native dopo un cross-build, altrimenti un successivo `make run-server` locale caricherebbe i nativi sbagliati:
  ```bash
  mvn clean package -DskipTests
  ```

---

## Configurazione: variabili d'ambiente e volumi

L'immagine definisce questi default (vedi `Dockerfile`):

| Variabile          | Default                                      | Descrizione |
|--------------------|----------------------------------------------|-------------|
| `QB_ROOT_PATH`     | `/data/qb/`                                  | Directory di configurazione e stato (`.qb/`). |
| `QB_OUTPUT_PATH`   | `/data/output/`                              | Output generati. |
| `QB_LOG_PATH`      | `/var/log/kubirag/`                          | Log applicativi. |
| `OLLAMA_BASE_URL`  | `http://host.docker.internal:11434/api`      | Endpoint Ollama. Da un container Linux verso Ollama sull'host può servire `--add-host=host.docker.internal:host-gateway`. |

Volumi dichiarati (persistenza tra riavvii):

- `/data/qb` — configurazione e store dei documenti/embeddings.
- `/data/output` — artefatti prodotti.
- `/var/log/kubirag` — log.

Il container avvia il server con `--add-modules jdk.incubator.vector` (richiesto dalle ottimizzazioni vettoriali) ed espone la porta `7070`.

---

## Verifica del deploy

1. **Architettura dell'immagine** — conferma che combaci con quella attesa:
   ```bash
   docker image inspect informatelier/kubirag:latest-amd64 --format '{{.Architecture}}'
   ```

2. **Avvio del server** — controlla i log all'avvio:
   ```bash
   make docker-run
   docker logs -f kubirag
   ```

3. **Endpoint RAG** — verifica che il server risponda sulla porta `7070`. L'endpoint `POST /chat/query` accetta un payload in stile Ollama e risponde in streaming (righe `data: {...}` in formato SSE):
   ```bash
   curl -N -X POST http://localhost:7070/chat/query \
     -H 'Content-Type: application/json' \
     -d '{
           "model": "llama3",
           "prompt": "ping",
           "stream": true,
           "options": { "temperature": 0.2, "max_tokens": 64 }
         }'
   ```
   Header opzionali: `Store-ID` (store da interrogare), `User-ID` e campo `sessionId` per la sessione di chat.

4. **Connettività Ollama** — assicurati che Ollama sia raggiungibile dall'interno del container all'URL configurato in `OLLAMA_BASE_URL`.

---

## Risoluzione problemi

| Sintomo | Causa probabile | Soluzione |
|---------|-----------------|-----------|
| `exec format error` all'avvio del container | Architettura immagine ≠ architettura host | Costruisci/pulla l'immagine per l'arch corretta (`PLATFORM=…`) o usa il manifest multi-arch. |
| Errori nel caricamento dei nativi sherpa (voce non funziona) | JAR costruito per un'arch diversa da quella dell'immagine | Ricostruisci con `make docker-build PLATFORM=…`: il JAR viene rigenerato con i classifier corretti. |
| Mindmap K-Means più lenta o log con fallback BLAS su ARM | Su `linux/arm64` SMILE usa il backend pure-JVM (manca OpenBLAS nativo) | Comportamento atteso. Per matrici TF-IDF piccole l'impatto è trascurabile. |
| `docker buildx: command not found` | buildx non disponibile | Aggiorna Docker o installa il plugin buildx. |
| Cross-build ARM da host x86 fallisce nella fase Docker | Emulazione QEMU non installata | Esegui `docker run --privileged --rm tonistiigi/binfmt --install all`. |
| `unauthorized` durante `docker-push` / `docker-manifest` | Non autenticato al registry | `docker login`. |
| Il container non raggiunge Ollama | `host.docker.internal` non risolvibile su Linux | Aggiungi `--add-host=host.docker.internal:host-gateway` o imposta `OLLAMA_BASE_URL` con l'IP dell'host. |
| Su Windows: `UnsatisfiedLinkError` su `jniopenblas`/`onnxruntime` | JAR costruito con classifier Linux, o solo uno dei due classifier impostato | Ricostruisci con **entrambi** `-Dsherpa.classifier=win-x64 -Dbytedeco.classifier=windows-x86_64`. |
| Su Windows: errore caricamento DLL MSVC (`vcruntime140`) | Manca il Visual C++ Redistributable di sistema | Le DLL runtime sono incluse nel JAR; se persiste, installa il **Microsoft Visual C++ Redistributable** x64. |

---

## Riferimenti

- `Makefile` — target Docker (`docker-jar`, `docker-build`, `docker-push`, `docker-manifest`, `docker-run`, `docker-stop`) e bare-JAR (`build`, `run-cli`, `run-server`, `run-server-port`).
- `Dockerfile` — definizione dell'immagine runtime.
- `pom.xml` — proprietà `sherpa.classifier` e `bytedeco.classifier`, profili di piattaforma.
- `CLAUDE.md` — sezione *Build & Run* (nota sul JAR platform-specific).
- `doc/manuals/voice-system-manual.md` — dettagli sul sistema vocale (nativi sherpa-onnx).
