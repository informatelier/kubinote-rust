# Manuale Sistema Vocale Kubirag

Questa guida descrive come utilizzare il sistema di interazione vocale integrato in Kubirag, basato su Sherpa-ONNX per riconoscimento vocale (ASR) e sintesi vocale (TTS).

## Indice

- [Panoramica](#panoramica)
- [Comandi Disponibili](#comandi-disponibili)
  - [/voice](#voice-avvia-interazione-vocale)
  - [/voice-config](#voice-config-configura-sistema-vocale)
- [Esempi d'Uso](#esempi-duso)
- [Configurazione Avanzata](#configurazione-avanzata)
- [Risoluzione Problemi](#risoluzione-problemi)

---

## Panoramica

Il sistema vocale Kubirag fornisce un'interazione hands-free con il sistema RAG (Retrieval-Augmented Generation):

### Funzionalità Principali

- **Riconoscimento Vocale (ASR)**: Trascrizione automatica del parlato in tempo reale
- **Sintesi Vocale (TTS)**: Generazione di risposte vocali tramite AI
- **Stati Interattivi**: 
  - `LISTENING`: In attesa di input vocale
  - `THINKING`: Elaborazione query con LLM
  - `SPEAKING`: Riproduzione risposta vocale
  - `IDLE`: Sessione inattiva

### Componenti

- **Sherpa-ONNX**: Libreria per elaborazione vocale
- **Modelli**: Download automatico al primo uso (~150MB)
- **Sessioni Vocali**: Gestione completa dell'interazione voce-testo-voce

---

## Comandi Disponibili

### `/voice` - Avvia Interazione Vocale

Avvia una sessione di interazione vocale per interrogare il sistema RAG senza usare la tastiera.

#### Sintassi

```
/voice
/voice --help
```

#### Comportamento

1. **Download Modelli**: Al primo avvio, il sistema scarica automaticamente i modelli necessari:
   - Modello ASR per riconoscimento vocale
   - Modello TTS per sintesi vocale
   - Dimensione: ~150MB totali

2. **Inizializzazione Sessione**: Crea una `VoiceSession` con callback per:
   - Cambi di stato (mostrati in console)
   - Trascrizione parziale (mostrata in tempo reale)
   - Trascrizione finale (salvata nel chat)
   - Risposte assistente (salvate nel chat)
   - Errori e livelli audio

3. **Interazione Loop**:
   - Senti il tuo comando quando in stato `LISTENING`
   - Elabora con LLM in stato `THINKING`
   - Risponde vocalmente in stato `SPEAKING`
   - Ripete finché interrotto

4. **Terminazione**: Premi `Ctrl+C` per uscire

#### Output Console

Durante l'interazione vedrai output come:
```
Starting voice interaction mode...
Press Ctrl+C to exit
[LISTENING] 
You: Come funziona il sistema?
[THINKING] 
Assistant: Il sistema utilizza Sherpa-ONNX per il riconoscimento...
```

---

### `/voice-config` - Configura Sistema Vocale

Configura le impostazioni per dispositivi audio, modelli e parametri TTS.

#### Sintassi

```
/voice-config [opzioni]
```

#### Opzioni

| Opzione | Descrizione |
|----------|-------------|
| `--show` | Mostra configurazione corrente |
| `--list-input` | Elenca dispositivi di input disponibili |
| `--list-output` | Elenca dispositivi di output disponibili |
| `--help` | Mostra questo messaggio di aiuto |

#### File di Configurazione

Le impostazioni sono salvate in:
```
~/.kubirag/models/sherpa-onnx/voice-config.json
```

#### Esempio Output `--show`

```
Current voice configuration:
  ASR Model Path: ~/.kubirag/models/sherpa-onnx/asr-model.onnx
  TTS Model Path: ~/.kubirag/models/sherpa-onnx/tts-model.onnx
  TTS Voice ID: 0
  Input Device: (default)
  Output Device: (default)
  Sample Rate: 16000 Hz
  Chunk Duration: 100 ms
  TTS Enabled: true
  Speaking Rate: 1.0
```

#### Esempio Output `--list-input`

```
Available input devices:
  - Microfono USB (1,0)
  - Built-in Microphone (0,0)
  - Webcam Microphone (2,0)
```

#### Esempio Output `--list-output`

```
Available output devices:
  - Headphones (1,0)
  - Speakers (0,0)
  - HDMI Output (2,0)
```

---

## Esempi d'Uso

### Esempio 1: Prima Configurazione

```bash
# Verifica dispositivi disponibili
/voice-config --list-input
/voice-config --list-output

# Mostra configurazione corrente
/voice-config --show
```

### Esempio 2: Interazione Vocale Base

```bash
# Avvia sessione vocale
/voice

# Durante la sessione:
[LISTENING] 
# Tu: "Qual è il progetto Kubirag?"

[THINKING]
# Sistema elabora...

[SPEAKING]
# Assistant: "Kubirag è un sistema RAG con interazione vocale..."

[Torna a LISTENING]
```

### Esempio 3: Flusso Completo Conversazione

```bash
# Inizia la sessione
/voice

[LISTENING]
You: Cosa puoi fare?

[THINKING]
Assistant: Posso rispondere a domande, generare testo, analizzare documenti...

[SPEAKING]
Assistant: (risposta vocale)

[LISTENING]
You: E l'architettura?

[THINKING]
Assistant: Il sistema usa Lucene per retrieval e llama per generation...

[SPEAKING]
Assistant: (risposta vocale)

# Ctrl+C per terminare
```

### Esempio 4: Risoluzione Problemi Dispositivi

```bash
# Se il microfono non funziona:
/voice-config --list-input
# Trova il nome corretto, poi modifica manualmente voice-config.json

# Se l'audio non esce:
/voice-config --list-output
# Trova il dispositivo corretto e aggiorna la configurazione
```

---

## Configurazione Avanzata

### File voice-config.json

Struttura del file di configurazione (posizione: `~/.kubirag/models/sherpa-onnx/voice-config.json`):

```json
{
  "asrModelPath": "path/to/asr-model.onnx",
  "ttsModelPath": "path/to/tts-model.onnx",
  "ttsVoiceId": 0,
  "inputDeviceName": "Microphone USB",
  "outputDeviceName": "Headphones",
  "sampleRate": 16000,
  "chunkDurationMs": 100,
  "enableTts": true,
  "speakingRate": 1.0
}
```

### Parametri Spiegati

| Parametro | Tipo | Descrizione | Default |
|-----------|-------|-------------|----------|
| `asrModelPath` | String | Percorso del modello ASR (riconoscimento vocale) | (auto) |
| `ttsModelPath` | String | Percorso del modello TTS (sintesi vocale) | (auto) |
| `ttsVoiceId` | Int | ID della voce per TTS multi-vocal | 0 |
| `inputDeviceName` | String | Nome dispositivo di input microfono | (default) |
| `outputDeviceName` | String | Nome dispositivo di output audio | (default) |
| `sampleRate` | Int | Frequenza di campionamento in Hz | 16000 |
| `chunkDurationMs` | Int | Durata chunk audio in ms | 100 |
| `enableTts` | Boolean | Abilita/disabilita sintesi vocale | true |
| `speakingRate` | Float | Velocità parlata (0.5-2.0) | 1.0 |

### Modelli Supportati

Il sistema supporta modelli nel formato ONNX compatibili con Sherpa-ONNX:

#### ASR (Riconoscimento Vocale)
- Whisper (multi-lingua)
- Paraformer (cinese/inglese)
- Zipformer (varie lingue)
- SenseVoice (multi-lingua)

#### TTS (Sintesi Vocale)
- VITS (multi-lingua)
- Matcha-TTS (cinese/inglese)
- Kokoro (giapponese)

### SessionManager Integration

Il sistema vocale si integra automaticamente con il `ChatSessionManager`:

- I comandi utente vocali vengono aggiunti alla chat come "User: ..."
- Le risposte vocali vengono aggiunte alla chat come "Assistant: ..."
- La cronologia conversazione viene mantenuta durante la sessione

---

## Risoluzione Problemi

### Problema 1: "No input devices found"

**Sintomi**: Errore all'avvio di `/voice`

**Soluzione**:
```bash
/voice-config --list-input
```
Assicurati che il microfono sia collegato e riconosciuto dal sistema operativo.

### Problema 2: "No output devices found"

**Sintomi**: Il sistema parla ma non senti niente

**Soluzione**:
```bash
/voice-config --list-output
```
Verifica che le casse/auricolari siano collegati e seleziona il dispositivo corretto.

### Problema 3: "Failed to initialize voice session"

**Sintomi**: Errore durante l'inizializzazione di `/voice`

**Possibili cause**:
- Modelli non scaricati (errore di rete o spazio disco)
- Percorsi modello errati in `voice-config.json`

**Soluzione**:
1. Verifica spazio disco sufficiente (>200MB libero)
2. Controlla connessione internet per download modelli
3. Elimina `~/.kubirag/models/sherpa-onnx/` e riprova

### Problema 4: "UnsatisfiedLinkError: no sherpa-onnx-jni"

**Sintomi**: Errore JNI al caricamento delle librerie native

**Soluzione**:
1. Verifica che le dipendenze Maven siano state scaricate:
   ```bash
   mvn dependency:tree | grep sherpa
   ```
2. Pulisci e ricostruisci:
   ```bash
   mvn clean package
   ```
3. Assicurati che il JAR finale contenga le librerie native

### Problema 5: Trascrizione Errata

**Sintomi**: Il sistema trascrive male o non capisce l'accento

**Soluzioni**:
1. Usa modelli specifici per la tua lingua
2. Aumenta `chunkDurationMs` in configurazione (es. 200ms)
3. Sposta il microfono più vicino alla bocca
4. Riduci rumore ambientale

### Problema 6: Voce Robotica

**Sintomi**: La TTS suona meccanica o poco naturale

**Soluzioni**:
1. Usa modelli TTS di alta qualità (VITS, Kokoro)
2. Regola `speakingRate` tra 0.9-1.1
3. Verifica che `ttsVoiceId` sia appropriato per la lingua
4. Aumenta sample rate (22000Hz per voci migliori)

### Problema 7: Download Modelli Lento

**Sintomi**: Il primo avvio impiega molto tempo

**Soluzione**:
I modelli (~150MB) vengono scaricati da GitHub releases al primo uso. Questo è normale. Per download più veloci:

1. Scarica manualmente i modelli da:
   https://github.com/k2-fsa/sherpa-onnx/releases

2. Salvali in: `~/.kubirag/models/sherpa-onnx/`

3. Riavvia `/voice`

---

## Note Tecniche

### Architettura

```
┌─────────────────────────────────────────────────────────────┐
│                   RAG System                     │
│  ┌─────────┐  ┌──────────┐  ┌─────────┐ │
│  │  Lucene │→│  Llama   │→│  Voice   │ │
│  │Vector DB│  │  LLM     │  │ Session  │ │
│  └─────────┘  └──────────┘  └────┬────┘ │
│                                    │        │
│                              ┌───────▼──────┐ │
│                              │ Sherpa-ONNX   │ │
│                              │ - ASR Engine  │ │
│                              │ - TTS Engine  │ │
│                              └───────────────┘ │
│                              ┌───────▼──────┐ │
│                        ┌───────▼───────▼────┐ │
│                        │  Audio   │  Audio   │ │
│                        │Capture   │Playback  │ │
│                        └──────────┴──────────┘ │
└─────────────────────────────────────────────────────┘
```

### Librerie Native

Il sistema usa librerie native platform-specific:

| Piattaforma | Libreria | Posizione |
|--------------|------------|-----------|
| Windows x64 | `sherpa-onnx-jni.dll` | JAR native |
| Linux x64 | `libsherpa-onnx-jni.so` | JAR native |
| Linux ARM64 | `libsherpa-onnx-jni.so` | JAR native |

Queste sono caricate automaticamente da Java tramite JNI.

### Performance

- **Latenza ASR**: <500ms per chunk
- **Latenza TTS**: 100-500ms per frase
- **Memoria**: ~500MB per sessione attiva
- **CPU**: 10-30% per thread di elaborazione

---

## Riferimenti

- **Sherpa-ONNX**: https://github.com/k2-fsa/sherpa-onnx
- **Documentazione API Java**: https://k2-fsa.github.io/sherpa/onnx/java-api/
- **Modelli Pre-addestrati**: https://github.com/k2-fsa/sherpa-onnx/releases

---

## Conclusione

Il sistema vocale Kubirag offre un'interazione hands-free efficiente con il sistema RAG. Per problemi o suggerimenti, consulta la documentazione ufficiale di Sherpa-ONNX o apri una issue nel repository Kubirag.