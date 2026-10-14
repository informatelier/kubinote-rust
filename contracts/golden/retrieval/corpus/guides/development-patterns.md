# Pattern di sviluppo KubiRAG

Pattern ricorrenti e vincoli architetturali scoperti durante lo sviluppo.
Da consultare prima di aggiungere nuove feature o modificare layer esistenti.

---

## 1. Dependency Injection manuale in RagTerminal / RagServer

Non esiste un DI container. Le dipendenze sono passate esplicitamente via costruttore.

**RagTerminal** (CLI): la lista `commands` in `RagTerminal.kt` è l'unico punto di wiring.
Ogni nuovo comando va aggiunto lì con le sue dipendenze già istanziate nel blocco `run {}`.

**RagServer** (web): le dipendenze (`documentStore`, `ragSystem`, `ragConfig`) sono
nel closure della funzione `run()`. I controller sono `object` Kotlin (singleton) e ricevono
le dipendenze come parametri delle funzioni, non nel costruttore.

```kotlin
// Pattern controller web (object singleton)
object CtrlFoo {
    fun doSomething(ctx: Context, dep: SomeDependency) { ... }
}
// Wiring in RagServer:
get("/foo") { ctx -> CtrlFoo.doSomething(ctx, documentStore) }
```

```kotlin
// Pattern comando CLI (class con costruttore)
class CmdFoo(private val dep: SomeDependency) : RagollamaCommand {
    override fun name() = "/foo"
    override fun execute(tokens: List<String>, chatMessages: List<String>): Result<String> { ... }
}
// Wiring in RagTerminal:
CmdFoo(documentStore)
```

---

## 2. Aggiunta stato attivo (active flag) nei controller REST

Quando un'entità ha uno stato "attivo" (es. ruoli, store), il flag non è nel modello
persistito — viene calcolato al momento della GET confrontando con `ragConfig`.

```kotlin
// Pattern: active calcolato on-the-fly nella GET
"active" to (role.roleName == ragConfig.currentRole)
```

**Corollario**: quando un'entità attiva viene modificata via PUT, bisogna propagare
la modifica a `ragConfig` e chiamare `storeConfig()`. Vedere `CtrlRoles.updateRole()`.

---

## 3. Simmetria index/query-time in Lucene

**Vincolo critico**: l'analyzer usato al momento dell'indicizzazione deve essere lo stesso
usato per la query sullo stesso campo. Cambiare analyzer richiede re-indicizzazione.

L'indice usa `ByteBuffersDirectory` (in-memory): viene ricostruito ad ogni avvio da
`kubirag-documents.json`. Un cambio di schema all'analyzer si propaga automaticamente
al primo riavvio — nessuna migrazione manuale necessaria.

**Pattern multilingua** (ADR-001):
- Campo `content` → `ItalianAnalyzer` (documenti IT e UNKNOWN)
- Campo `content_en` → `EnglishAnalyzer` (documenti EN con confidence ≥ 0.60)
- Query: clausole `SHOULD` su entrambi i campi con i rispettivi analyzer
- `PerFieldAnalyzerWrapper` passato a `IndexWriterConfig` gestisce il routing automaticamente

---

## 4. Language detection senza librerie esterne

`LanguageDetector` (in `lucene/`) usa conteggio di stopword italiane vs inglesi.
Soglia minima: 5 parole. Confidence threshold: 0.60 (costante `CONFIDENCE_THRESHOLD`).
Per testi brevi o tecnici → `UNKNOWN` → fallback a `ItalianAnalyzer`.

**Dove viene usato**:
- `RagLucene.addDocumentToWriter()` — sceglie il campo content corretto
- `MindmapService.generate()` — sceglie l'analyzer per tokenizzazione

---

## 5. Analyzer Lucene: tokenizzazione per uso non-search

Per estrarre termini da testo (mindmap, analisi) senza passare per l'indice, usare
`analyzer.tokenStream()` direttamente sul testo grezzo. I `TokenStream` non sono
thread-safe per singola istanza: usare sequenzialmente o istanziare uno per thread.

```kotlin
fun tokenize(text: String, analyzer: Analyzer): List<String> {
    val tokens = mutableListOf<String>()
    val stream = analyzer.tokenStream("content", text)
    val attr = stream.addAttribute(CharTermAttribute::class.java)
    stream.reset()
    while (stream.incrementToken()) { tokens.add(attr.toString()) }
    stream.end()
    stream.close()
    return tokens
}
```

`ItalianAnalyzer` e `EnglishAnalyzer` sono già in `lucene-analysis-common` (nessuna
dipendenza aggiuntiva). Sono istanziabili come singleton (`object MindmapService`).

---

## 6. Rendering Mermaid nel frontend Vue 3 (no build)

Il frontend usa Vue 3 CDN (`vue.global.prod.js`) + Mermaid CDN.
Mermaid deve essere inizializzato con `startOnLoad: false` per evitare il rendering
automatico di tutti i blocchi `pre.mermaid` sulla pagina.

```javascript
mermaid.initialize({ startOnLoad: false, theme: 'dark' });

// Render on demand (dentro una funzione Vue async):
const { svg } = await mermaid.render('unique-id-' + Date.now(), mermaidText);
container.innerHTML = svg;
```

**Attenzione**: l'ID passato a `mermaid.render()` deve essere **univoco per chiamata** —
Mermaid mantiene un registro interno e lancia errore se lo stesso ID viene riusato.
Usare `Date.now()` o un contatore come suffisso.

**Attenzione — timing con v-if/v-else e ref**: impostare lo stato che rende visibile
il container **prima** di `nextTick()`, non dopo. I ref Vue su nodi `v-else` sono `null`
finché il nodo non è nel DOM; `nextTick()` aggiorna il DOM rispetto allo stato corrente,
ma se lo stato ancora nasconde il nodo il ref rimane `null`.

```javascript
// SBAGLIATO: container è null perché loading è ancora true durante nextTick
mindmapData.value = data;
await nextTick();
mindmapLoading.value = false;   // troppo tardi

// CORRETTO: loading=false prima, così il v-else entra nel DOM
mindmapData.value = data;
mindmapLoading.value = false;   // ← il container diventa visibile
await nextTick();               // ← ora il ref è valorizzato
const container = mindmapContainer.value;  // ✓ non null
```

---

## 7. Modal Vue-controlled dentro #app

I modal dinamici (settings, viewer, mindmap) vivono dentro `<div id="app">` per
accedere alle ref Vue (`v-if`, `:class`, `@click.self`).

I modal statici (notebooks, team panel, role panel) vivono **fuori** da `#app` e
usano JS vanilla con `getElementById` / `classList`. Non usare `v-if` su questi.

```html
<!-- Modal Vue-controlled (dentro #app) -->
<div class="modal-overlay" :class="{ open: myModalOpen }" @click.self="myModalOpen = false">
    <div class="modal modal-foo">...</div>
</div>
```

Le dimensioni del modal si controllano tramite classe CSS dedicata (es. `.modal-mindmap`
per grafici wide, `.modal-viewer` per contenuto testuale).

---

## 8. Output comandi CLI: Result<String> con Markdown

I comandi restituiscono `Result<String>` — il testo viene renderizzato da
`MdPrettyPrinter` di kubicommons se attivo. Usare Markdown nei messaggi di output.

Per output lunghi (es. mappa Mermaid), racchiudere in backtick tripli con tipo:
```
\```mermaid
graph TD
  ...
\```
```

I blocchi di codice passano through senza trasformazione da `MdPrettyPrinter`.

---

## 9. Persist config: storeConfig() vs saveRoles()

Due meccanismi di persistenza separati:

| File | Contenuto | Come salvare |
|---|---|---|
| `qb-rag.json` | `RagConfig` (systemPrompt, currentRole, modelName, ecc.) | `ragConfig.storeConfig()` |
| `qb-roles.json` | Mappa dei ruoli `Map<String, RoleEntry>` | `CtrlRoles.saveRoles()` |

Quando si modifica un ruolo che è anche il ruolo attivo, vanno aggiornati **entrambi**
(vedi `CtrlRoles.updateRole()` e `CtrlRoles.deleteRole()`).

---

## 10. Re-index automatico al riavvio

L'indice Lucene (`ByteBuffersDirectory`) è completamente in-memory. Alla chiusura
non viene persistito — viene ricostruito da `DocumentStore` (che legge
`kubirag-documents.json`) ad ogni avvio tramite `ragSystem.restore()` / `openRag()`.

**Conseguenza**: qualsiasi modifica allo schema dell'indice (nuovi campi, cambio analyzer)
non richiede migration script — basta riavviare. I documenti sono la fonte di verità,
non l'indice.

**Eccezione**: `GlobalIndex` viene persistito su disco via `IndexBuilder.saveToDisk()` 
alla chiusura della sessione CLI. Contiene headings, tags, e link estratti.
