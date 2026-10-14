# Guida Comandi Voice, Role e Ask

Questa guida spiega come utilizzare i comandi relativi a voci, ruoli e domande all'AI.

## Comandi Role (Gestione Ruoli)

I ruoli permettono di definire prompt di sistema personalizzati, pattern di file associati e configurazioni vocali.

### /role-add - Aggiungi un nuovo ruolo

Crea un nuovo ruolo con prompt, pattern di file opzionali e configurazione vocale.

**Sintassi:**
```
/role-add <roleName> <rolePrompt> [--files <pattern1> <pattern2> ...] [--voice <voiceName>]
```

**Esempi:**
```bash
# Ruolo semplice con solo prompt
/role-add developer "You are an expert Kotlin developer helping with code reviews."

# Ruolo con pattern di file
/role-add dba "You are a database expert specializing in SQL optimization." --files "*.sql" "docs/schema/*.md"

# Ruolo con configurazione vocale
/role-add teacher "You are a teacher explaining concepts clearly." --voice paola

# Ruolo completo con tutto
/role-add dddexpert "You are a Domain Driven Design Expert." --files "*.md" "docs/*.md" --voice miro
```

### /role-list - Elenca i ruoli

Mostra tutti i ruoli con i loro dettagli.

**Sintassi:**
```
/role-list
```

**Output di esempio:**
```
Available roles (4):
--------------------------------------------------
1. developer
   Prompt: You are an expert Kotlin developer helping...
   
2. dddexpert
   Prompt: You are a Domain Driven Design Expert...
   Files: *.md, docs/*.md
   Voice: miro

3. teacher
   Prompt: You are a teacher explaining concepts clearly...
   Voice: paola
```

### /role-use - Attiva un ruolo

Attiva un ruolo impostando il system prompt e, se configurato, la voce.

**Sintassi:**
```
/role-use <roleName>
```

**Esempi:**
```bash
# Attiva il ruolo dddexpert
/role-use dddexpert

# Output:
# Now using role 'dddexpert' with system prompt:
# You are a Domain Driven Design Expert.
# 
# File patterns associated with this role:
#   - *.md
#   - docs/*.md
# 
# Use /role-pinfiles to load and pin these files
# Voice configuration 'miro' applied.
```

### /set-role - Modifica un attributo del ruolo

Modifica un singolo attributo di un ruolo esistente.

**Sintassi:**
```
/set-role <roleName> <option> <value>
```

**Opzioni:**
```
--prompt <text>          Set role prompt
--voice <name>           Set voice configuration
--add-files <patterns>   Add file patterns (space-separated)
--remove-files <patterns> Remove file patterns (space-separated)
--clear-files             Clear all file patterns
--clear-voice            Clear voice configuration
```

**Esempi:**
```bash
# Cambia la configurazione vocale
/set-role dddexpert --voice paola

# Aggiungi nuovi pattern di file (nota i pattern con wildcard tra virgolette)
/set-role dddexpert --add-files "*.sql" "docs/api/*.md"

# Rimuovi pattern di file
/set-role dddexpert --remove-files "*.sql"

# Aggiorna il prompt
/set-role dddexpert --prompt "You are a DDD expert with focus on bounded contexts."

# Rimuovi la configurazione vocale
/set-role dddexpert --clear-voice

# Rimuovi tutti i pattern di file
/set-role dddexpert --clear-files
```

### /role-files - Mostra i file del ruolo corrente

Mostra quali file verrebbero caricati dai pattern del ruolo corrente (dry-run).

**Sintassi:**
```
/role-files
```

**Output di esempio:**
```
Files that would be loaded for role 'dddexpert':
============================================================
📁 Pattern: *.md
Found 5 file(s):
• README.md (1 KB)
C:\project\README.md
• architecture.md (3 KB)
C:\project\architecture.md
• design.md (2 KB)
C:\project\docs\design.md

📁 Pattern: docs/*.md
Found 3 file(s):
• api.md (4 KB)
C:\project\docs\api.md

============================================================
Total: 8 file(s) would be loaded
Use /role-pinfiles to actually load and pin these files
```

### /role-pinfiles - Carica e pina i file del ruolo

Carica effettivamente i file definiti nei pattern del ruolo corrente nel sistema RAG.

**Sintassi:**
```
/role-pinfiles
```

**Flusso di lavoro consigliato:**
1. `/role-use dba` - Attiva il ruolo
2. `/role-files` - Preview dei file che verranno caricati
3. `/role-pinfiles` - Carica effettivamente i file

### /role-setfiles - Imposta i pattern di file del ruolo corrente

Modifica i pattern di file del ruolo corrente.

**Sintassi:**
```
/role-setfiles <pattern1> <pattern2> ...
```

**Esempio:**
```bash
# Imposta nuovi pattern per il ruolo corrente
/role-setfiles "*.sql" "docs/*.md" "config/*.yml"
```

---

## Comandi Voice (Configurazioni Vocali)

Le configurazioni vocali permettono di definire impostazioni TTS (Text-to-Speech) diverse per diversi ruoli o lingue.

### /voice-config-add - Aggiungi una configurazione vocale

Crea una nuova configurazione vocale con tutte le impostazioni TTS.

**Sintassi:**
```
/voice-config-add <name> [options]
```

**Opzioni:**
```
--asr-model <path>         ASR model path
--tts-model <path>         TTS model path
--tts-voice-id <id>        TTS voice ID (default: 0)
--input-device <name>      Input device name
--output-device <name>     Output device name
--sample-rate <hz>         Sample rate in Hz (default: 16000)
--chunk-duration <ms>      Chunk duration in ms (default: 100)
--speaking-rate <rate>     Speaking rate (default: 1.0)
--tts / --no-tts           Enable/disable TTS (default: enabled)
```

**Esempi:**
```bash
# Configurazione semplice con solo modello TTS
/voice-config-add italian --tts-model "C:/models/vits-piper-it_IT-paola-medium/it_IT-paola-medium.onnx"

# Configurazione con velocità personalizzata
/voice-config-add fast-italian --tts-model "C:/models/paola.onnx" --speaking-rate 1.3

# Configurazione con voice ID specifico
/voice-config-add kitten --tts-model "C:/models/kitten/model.fp16.onnx" --tts-voice-id 7
```

### /voice-config-list - Elenca le configurazioni vocali

Mostra tutte le configurazioni vocali salvate con quella attiva marcata.

**Sintassi:**
```
/voice-config-list
```

**Output di esempio:**
```
Available voice configurations:
  - paola
      TTS Model: C:/Users/nicol/Works/models/vits-piper-it_IT-paola-medium/it_IT-paola-medium.onnx
      Speaking Rate: 1.0
      TTS: enabled
  
  - miro [CURRENT]
      TTS Model: C:/Users/nicol/Works/models/vits-piper-it_IT-miro-high/it_IT-miro-high.onnx
      Speaking Rate: 1.3
      TTS: enabled

  - tjiho
      TTS Model: C:/Users/nicol/Works/models/vits-piper-fr_FR-tjiho-model1/fr_FR-tjiho-model1.onnx
      Speaking Rate: 0.7
      TTS: enabled

Use '/voice-config-use <name>' to apply a configuration
```

### /voice-config-use - Applica una configurazione vocale

Applica una configurazione vocale come quella attiva.

**Sintassi:**
```
/voice-config-use <configName>
```

**Esempio:**
```bash
/voice-config-use miro

# Output:
# Applied voice configuration: miro
#   ASR Model: (default)
#   TTS Model: C:/Users/nicol/Works/models/vits-piper-it_IT-miro-high/it_IT-miro-high.onnx
#   TTS Voice ID: 0
#   Input Device: (default)
#   Output Device: (default)
#   Sample Rate: 16000 Hz
#   Speaking Rate: 1.3
#   TTS Enabled: true
```

### /voice-config-remove - Rimuove una configurazione vocale

Rimuove permanentemente una configurazione vocale salvata.

**Sintassi:**
```
/voice-config-remove <configName>
```

**Esempio:**
```bash
/voice-config-remove old-config
```

### /voice-config - Configurazione voce interattiva

Configura le impostazioni vocali con opzioni di visualizzazione e test.

**Sintassi:**
```
/voice-config [options]
```

**Opzioni:**
```
--show             Show current voice configuration
--list-input       List available input devices
--list-output      List available output devices
--help             Show help message
```

**Esempi:**
```bash
# Mostra configurazione corrente
/voice-config --show

# Elenca dispositivi di input
/voice-config --list-input

# Elenca dispositivi di output
/voice-config --list-output
```

**Output di esempio:**
```
Current voice configuration:
  ASR Model Path: (default)
  TTS Model Path: C:/models/paola.onnx
  TTS Voice ID: 0
  Input Device: (default)
  Output Device: (default)
  Sample Rate: 16000 Hz
  Chunk Duration: 100 ms
  TTS Enabled: true
  Speaking Rate: 1.0
```

### /voice - Modalità interazione vocale

Avvia la modalità di interazione vocale con riconoscimento e sintesi vocale.

**Sintassi:**
```
/voice
```

**Come funziona:**
1. Il sistema scarica i modelli vocali necessari al primo uso (~150MB)
2. Parla la tua domanda quando lo stato è LISTENING
3. Il sistema elabora la tua voce e risponde vocalmente
4. Premi Ctrl+C per uscire

**Stati:**
- LISTENING: In attesa del tuo input vocale
- THINKING: Elaborazione della query con l'LLM
- SPEAKING: Pronuncia della risposta
- IDLE: Sessione vocale inattiva

---

## Comando Ask (Domande all'AI)

### /ask - Fai una domanda all'AI

Invia una domanda all'AI con contesto RAG e cronologia chat. Supporta output vocale.

**Sintassi:**
```
/ask [--voice] <question>
```

**Esempi:**
```bash
# Domanda semplice (solo testo)
/ask What are the benefits of using RAG architecture?

# Domanda con output vocale
/ask --voice Spiegami i benefici dell'architettura RAG

# Domande complesse con ruoli
/role-use dddexpert
/ask --voice Come posso definire un bounded context in un sistema microservizi?

# Chat con storia e voce
/ask --voice Qual è la differenza tra aggregate e entity?
/ask E l'aggregate root?
```

**Integrazione con Ruoli:**
Quando usi `/ask --voice` con un ruolo attivo che ha configurato una `voiceName`, la configurazione vocale del ruolo viene applicata automaticamente prima di generare l'audio.

**Esempio di flusso completo:**
```bash
# 1. Crea un ruolo con voce associata
/role-add dddvoice "Sei un esperto di Domain Driven Design. Rispondi in italiano in modo semplice." --voice paola

# 2. Attiva il ruolo
/role-use dddvoice

# 3. Fai domande con risposta vocale
/ask --voice Cos'è un Value Object in DDD?

# 4. Il sistema usa automaticamente la voce "paola" per la risposta
```

---

## Scenari di Utilizzo Comuni

### Scenario 1: Assistente multilingua

```bash
# Crea ruoli per diverse lingue
/role-add assistant-it "Sei un assistente italiano. Rispondi in italiano." --voice paola
/role-add assistant-fr "Tu es un assistant français. Réponds en français." --voice siwis
/role-add assistant-en "You are an English assistant. Answer in English." --voice kitten-en

# Usa il ruolo appropriato per ogni lingua
/role-use assistant-it
/ask --voice Come stai oggi?

/role-use assistant-fr
/ask --voice Comment allez-vous?

/role-use assistant-en
/ask --voice How are you?
```

### Scenario 2: Expert tecnico con documentazione

```bash
# Crea ruolo con file associati e voce
/role-add dba "Sei un esperto di database. Analizza i documenti SQL." --files "*.sql" "docs/schema/*.md" --voice miro

# Attiva il ruolo
/role-use dba

# Verifica quali file verranno caricati
/role-files

# Carica i file nel contesto
/role-pinfiles

# Fai domande con risposta vocale
/ask --voice Quali sono le tabelle principali e come sono correlate?
```

### Scenario 3: Aggiornamento dinamico della voce

```bash
# Crea ruolo senza voce iniziale
/role-add teacher "Sei un insegnante che spiega concetti in modo semplice."

# Usa il ruolo
/role-use teacher

# Aggiungi una voce più lenta per spiegazioni
/set-role teacher --voice paola

# Prova una voce più veloce
/set-role teacher --voice miro

# Torna a risposta solo testo
/set-role teacher --clear-voice

/ask Spiegami la ricorsione
```

### Scenario 4: Configurazioni multiple per lo stesso ruolo

```bash
# Crea configurazioni vocali diverse per lo stesso contenuto
/voice-config-add narration-slow --tts-model "C:/models/paola.onnx" --speaking-rate 0.8
/voice-config-add narration-normal --tts-model "C:/models/paola.onnx" --speaking-rate 1.0
/voice-config-add narration-fast --tts-model "C:/models/paola.onnx" --speaking-rate 1.3

# Crea il ruolo
/role-add narrator "Sei un narratore che legge il testo chiaramente." --voice narration-normal

# Cambia velocità a seconda del contesto
/set-role narrator --voice narration-slow  # Per concetti complessi
/set-role narrator --voice narration-fast  # Per riassunti rapidi
```

---

## File di Configurazione

### qb-roles.json
Contiene tutti i ruoli definiti con prompt, pattern di file e configurazioni vocali.

**Esempio:**
```json
{
  "dddvoice": {
    "roleName": "dddvoice",
    "rolePrompt": "Sei un esperto di Domain Driven Design. Rispondi in italiano.",
    "filePatterns": ["*.md", "docs/*.md"],
    "voiceName": "paola"
  },
  "dba": {
    "roleName": "dba",
    "rolePrompt": "You are a database expert.",
    "filePatterns": ["*.sql"],
    "voiceName": "miro"
  }
}
```

### qb-voice-configs.json
Contiene tutte le configurazioni vocali salvate.

**Esempio:**
```json
{
  "configs": {
    "paola": {
      "asrModelPath": "",
      "ttsModelPath": "C:/models/vits-piper-it_IT-paola-medium/it_IT-paola-medium.onnx",
      "ttsVoiceId": 0,
      "inputDeviceName": "",
      "outputDeviceName": "",
      "sampleRate": 16000,
      "chunkDurationMs": 100,
      "enableTts": true,
      "speakingRate": 1.0,
      "modelBaseUrl": "https://huggingface.co/k2-fsa/sherpa-onnx/resolve/main/"
    },
    "miro": {
      "asrModelPath": "",
      "ttsModelPath": "C:/models/vits-piper-it_IT-miro-high/it_IT-miro-high.onnx",
      "ttsVoiceId": 0,
      "inputDeviceName": "",
      "outputDeviceName": "",
      "sampleRate": 16000,
      "chunkDurationMs": 100,
      "enableTts": true,
      "speakingRate": 1.3,
      "modelBaseUrl": "https://huggingface.co/k2-fsa/sherpa-onnx/resolve/main/"
    }
  }
}
```

### ~/.kubirag/models/sherpa-onnx/voice-config.json
Contiene la configurazione vocale attualmente attiva (applicata da `/voice-config-use` o `/role-use`).

---

## Comandi Correlati

### /chat-new / /chat / /chat-list / /chat-remove
Gestione delle sessioni di chat separate.

### /pin / /unpin / /pinned
Gestione dei documenti pinnati nel sistema RAG.

---

## Note Importanti

1. **Primo uso vocale**: I modelli vocali (~150MB) vengono scaricati al primo utilizzo.

2. **Pattern di file con wildcard**: Quando usi pattern con `*`, racchiudili tra virgolette per evitare espansione della shell:
   ```
   /role-add test "Test" --files "*.md" "docs/*.yml"
   ```

3. **Configurazione vocale automatica**: Quando usi `/ask --voice` con un ruolo che ha `voiceName` impostato, la configurazione vocale viene applicata automaticamente.

4. **Persistenza**: Tutte le configurazioni (ruoli, voci) sono persistenti e vengono salvate automaticamente.

5. **TAB completion**: Molti comandi supportano TAB completion per i nomi di ruoli, configurazioni vocali, ecc.

---

## Troubleshooting

### Modello TTS non trovato
```
TTS model not found at C:/models/paola.onnx
```
**Soluzione**: Verifica che il percorso del modello sia corretto e il file esista.

### Configurazione vocale non applicata
```
Warning: Voice configuration 'miro' not found.
```
**Soluzione**: Verifica che la configurazione vocale esista con `/voice-config-list`.

### Nessun output vocale
Verifica che:
1. TTS sia abilitato (`enableTts: true`)
2. Il percorso del modello TTS sia corretto
3. I dispositivi audio siano configurati correttamente

### Ruolo non trovato
```
Role 'dddexpert' not found.
Available roles: developer, dba
```
**Soluzione**: Verifica il nome del ruolo con `/role-list`.
