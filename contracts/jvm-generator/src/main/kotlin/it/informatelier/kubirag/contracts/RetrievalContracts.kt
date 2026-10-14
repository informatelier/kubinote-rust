package it.informatelier.kubirag.contracts

import it.informatelier.kubilang.governance.ResourceRef
import it.informatelier.kubilang.governance.ResourceUri
import it.informatelier.kubirag.lucene.ChunkConfig
import it.informatelier.kubirag.lucene.DocumentChunkingService
import it.informatelier.kubirag.lucene.LanguageDetector
import it.informatelier.kubirag.lucene.LuceneSearchEngine
import it.informatelier.kubirag.lucene.SearchOptimizationConfig
import it.informatelier.kubirag.ragollama.DocumentStore
import it.informatelier.kubirag.ragollama.Embedding
import it.informatelier.kubirag.ragollama.EmbeddingProjectionDescriptor
import it.informatelier.kubirag.ragollama.EmbeddingProjectionState
import it.informatelier.kubirag.ragollama.EmbeddingProjectionStore
import it.informatelier.kubirag.ragollama.InMemoryVectorStore
import it.informatelier.kubirag.ragollama.RagDocument
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.apache.lucene.analysis.Analyzer
import org.apache.lucene.analysis.en.EnglishAnalyzer
import org.apache.lucene.analysis.it.ItalianAnalyzer
import org.apache.lucene.analysis.tokenattributes.CharTermAttribute
import org.apache.lucene.analysis.tokenattributes.PositionIncrementAttribute
import java.io.File
import java.nio.file.Files

/**
 * Golden del bounded context Retrieval (F2), scritti dal codice Kotlin e da Lucene 10.4:
 * - `analyzers.json`: stopword e token di `ItalianAnalyzer`/`EnglishAnalyzer` (termine e
 *   posizione) su casi limite, query e chunk del corpus;
 * - `language.json`: `LanguageDetector` sui chunk e su casi limite;
 * - `chunks.json`: `DocumentChunkingService` con la configurazione di default e una ridotta;
 * - `lucene-scores.json`: tutti i documenti trovati da `LuceneSearchEngine` per ogni query del
 *   corpus, con il punteggio BM25 esatto (O2-KR1);
 * - `lucene-synthetic.json`: corpus piccolo con titoli, documenti inglesi, frasi tra virgolette,
 *   sinonimi e query di sole stopword;
 * - `projections/`: i file runtime `<store>-documents.json`, `<store>-embeddings.json` e il
 *   descrittore portabile `embedding-profile.json`.
 */
class RetrievalContracts(private val outDir: File) {
    private val corpusDir = File(outDir, "corpus")
    private val json = Json { prettyPrint = true; prettyPrintIndent = "  " }
    private val italian = ItalianAnalyzer()
    private val english = EnglishAnalyzer()

    fun writeAll() {
        val chunks = corpusChunks()
        writeAnalyzers(chunks)
        writeLanguage(chunks)
        writeChunks()
        writeCorpusScores(chunks)
        writeSynthetic()
        writeProjections()
    }

    /** `bench`: latenza di riferimento di Lucene per O2-KR2, dipende dalla macchina. */
    fun writeBenchmark() {
        val chunks = corpusChunks()
        val texts = queries().map { it.getValue("text").jsonPrimitive.content }
        val coldStart = System.nanoTime()
        val engine = LuceneSearchEngine()
        engine.configure(SearchOptimizationConfig(maxResults = 10))
        engine.indexAll(chunks)
        runBlocking { engine.search(texts.first(), 6) }
        val coldMs = (System.nanoTime() - coldStart) / 1e6
        repeat(50) { texts.forEach { runBlocking { engine.search(it, 6) } } }
        val samples = (1..40).flatMap { texts.map { text ->
            val start = System.nanoTime()
            runBlocking { engine.search(text, 6) }
            (System.nanoTime() - start) / 1e6
        } }.sorted()
        engine.close()
        write("lucene-latency.json", obj(
            "engine" to str("kubirag LuceneSearchEngine, JVM ${System.getProperty("java.version")}"),
            "machine" to str("${System.getProperty("os.name")} ${System.getProperty("os.arch")}, ${Runtime.getRuntime().availableProcessors()} cpu"),
            "queries" to JsonPrimitive(samples.size),
            "maxResults" to JsonPrimitive(6),
            "coldStartMs" to JsonPrimitive(coldMs),
            "p50Ms" to JsonPrimitive(samples[samples.size / 2]),
            "p95Ms" to JsonPrimitive(samples[(samples.size * 95) / 100])
        ))
    }

    private fun corpusChunks(): List<RagDocument> {
        val chunker = DocumentChunkingService()
        return corpusFiles().flatMap { source ->
            val path = source.relativeTo(corpusDir).invariantSeparatorsPath
            chunker.chunkDocument(RagDocument(
                id = path,
                content = source.readText(Charsets.UTF_8),
                metadata = linkedMapOf("filename" to source.name, "path" to path)
            ))
        }
    }

    private fun corpusFiles() = corpusDir.walkTopDown().filter { it.isFile && it.extension == "md" }
        .sortedBy { it.relativeTo(corpusDir).invariantSeparatorsPath }.toList()

    private fun queries() = Json.parseToJsonElement(File(outDir, "queries.json").readText())
        .jsonObject.getValue("queries").jsonArray.map { it.jsonObject }

    private val edgeCases = listOf(
        "L'editor dell'applicazione e un'altra nell'area",
        "l’uso dell’indice con l’apostrofo tipografico",
        "Perché è già più facile: città, università, così",
        "PROGRAMMAZIONE Programmazione programmazione",
        "qb-rag.json kubirag-1.0.0-SNAPSHOT.jar chunk_index file.md",
        "Nicola's notebook and the users' guides, it’s fine",
        "running runs ran easily generalization connected relational",
        "3.14 2026-10-14 v1.2.3 1,000 100%",
        "http://localhost:11434/api e-mail mario.rossi@example.com",
        "日本語のテキスト 中文 한국어",
        "emoji 🎉 test 👍🏽 ok",
        "  spazi   multipli\tcon\ttab\ne righe  ",
        "il lo la di da in con su per tra fra",
        "the a an and or but of to",
        "İstanbul ÀÉÎÕÜ Straße",
        "a\"quoted\" (paren) [brackets] {braces} ¿qué?",
        "x".repeat(300)
    )

    private fun tokens(analyzer: Analyzer, text: String): List<Pair<String, Int>> {
        val out = mutableListOf<Pair<String, Int>>()
        analyzer.tokenStream("content", text).use { stream ->
            val term = stream.addAttribute(CharTermAttribute::class.java)
            val increment = stream.addAttribute(PositionIncrementAttribute::class.java)
            stream.reset()
            var position = -1
            while (stream.incrementToken()) {
                position += increment.positionIncrement
                out += term.toString() to position
            }
            stream.end()
        }
        return out
    }

    private fun positioned(tokens: List<Pair<String, Int>>) =
        JsonArray(tokens.map { (term, position) -> JsonArray(listOf(str(term), JsonPrimitive(position))) })

    private fun writeAnalyzers(chunks: List<RagDocument>) {
        val samples = edgeCases + queries().map { it.getValue("text").jsonPrimitive.content }
        write("analyzers.json", obj(
            "italianStopwords" to JsonArray(italian.stopwordSet.map { String(it as CharArray) }.sorted().map(::str)),
            "englishStopwords" to JsonArray(english.stopwordSet.map { String(it as CharArray) }.sorted().map(::str)),
            "samples" to JsonArray(samples.map { text ->
                obj(
                    "text" to str(text),
                    "italian" to positioned(tokens(italian, text)),
                    "english" to positioned(tokens(english, text))
                )
            }),
            // Chunk del corpus: termini separati da spazio (lo StandardTokenizer non ne produce).
            "chunks" to JsonArray(chunks.map { chunk ->
                val it = tokens(italian, chunk.content)
                val en = tokens(english, chunk.content)
                obj(
                    "id" to str(chunkId(chunk)),
                    "italian" to str(it.joinToString(" ") { it.first }),
                    "italianLastPosition" to JsonPrimitive(it.lastOrNull()?.second ?: -1),
                    "english" to str(en.joinToString(" ") { it.first }),
                    "englishLastPosition" to JsonPrimitive(en.lastOrNull()?.second ?: -1)
                )
            })
        ))
    }

    private fun chunkId(chunk: RagDocument) = chunk.id

    private fun writeLanguage(chunks: List<RagDocument>) {
        val samples = edgeCases.map { it to it } + chunks.map { chunkId(it) to it.content }
        write("language.json", obj(
            "minWords" to JsonPrimitive(5),
            "confidenceThreshold" to JsonPrimitive(LanguageDetector.CONFIDENCE_THRESHOLD),
            "cases" to JsonArray(samples.map { (id, text) ->
                val detection = LanguageDetector.detect(text)
                obj(
                    "id" to str(id),
                    "text" to str(if (id == text) text else null),
                    "language" to str(detection.language.name),
                    "confidence" to JsonPrimitive(detection.confidence)
                )
            })
        ))
    }

    private fun writeChunks() {
        val configs = listOf(
            "default" to ChunkConfig(),
            "small" to ChunkConfig(chunkSize = 120, overlap = 20, minChunkSize = 40)
        )
        write("chunks.json", obj(
            "configs" to JsonArray(configs.map { (label, config) ->
                val chunker = DocumentChunkingService(config)
                obj(
                    "label" to str(label),
                    "chunkSize" to JsonPrimitive(config.chunkSize),
                    "overlap" to JsonPrimitive(config.overlap),
                    "minChunkSize" to JsonPrimitive(config.minChunkSize),
                    "sources" to JsonArray(corpusFiles().map { source ->
                        val path = source.relativeTo(corpusDir).invariantSeparatorsPath
                        val document = RagDocument(path, source.readText(Charsets.UTF_8), linkedMapOf("path" to path))
                        obj(
                            "path" to str(path),
                            "chunks" to JsonArray(chunker.chunkDocument(document).map { chunk ->
                                val words = chunker.tokenize(chunk.content)
                                obj(
                                    "id" to str(chunk.id),
                                    "metadata" to JsonObject(chunk.metadata.mapValues { str(it.value) }),
                                    "words" to JsonPrimitive(words.size),
                                    "first" to str(words.firstOrNull()),
                                    "last" to str(words.lastOrNull()),
                                    "chars" to JsonPrimitive(chunk.content.length)
                                )
                            })
                        )
                    })
                )
            })
        ))
    }

    private fun writeCorpusScores(chunks: List<RagDocument>) {
        val engine = LuceneSearchEngine()
        engine.configure(SearchOptimizationConfig(maxResults = 1000))
        engine.indexAll(chunks)
        write("lucene-scores.json", obj(
            "note" to str("Tutti i documenti padre trovati (search con maxResults=1000, diversificazione attiva)"),
            "queries" to JsonArray(queries().map { query ->
                scored(engine, query.getValue("id").jsonPrimitive.content, query.getValue("text").jsonPrimitive.content)
            })
        ))
        engine.close()
    }

    private fun scored(engine: LuceneSearchEngine, id: String, text: String): JsonObject {
        val hits = runBlocking { engine.search(text, 1000) }
        return obj(
            "id" to str(id),
            "text" to str(text),
            "expandedQuery" to str(engine.expandedQuery(text)),
            "results" to JsonArray(hits.map { hit ->
                obj("documentId" to str(hit.documentId), "score" to JsonPrimitive(hit.score))
            })
        )
    }

    private fun writeSynthetic() {
        val documents = listOf(
            RagDocument("it-title", "Il notebook raccoglie le note markdown e le sorgenti del progetto, con concetti ed evidenze collegati.",
                linkedMapOf("title" to "Guida al notebook", "path" to "it-title.md")),
            RagDocument("it-meta-title", "La ricerca usa un indice BM25 con espansione della query e diversificazione dei risultati.",
                linkedMapOf("metaTitle" to "Ricerca e indice", "path" to "it-meta-title.md")),
            RagDocument("it-elision", "L'editor dell'applicazione mostra l'anteprima delle note markdown nell'area di lavoro.",
                linkedMapOf("path" to "it-elision.md")),
            RagDocument("en-guide", "The search engine ranks the documents with BM25 and then the embedding model reorders the candidates of the query.",
                linkedMapOf("title" to "Search guide", "path" to "en-guide.md")),
            RagDocument("en-possessive", "Nicola's notebook keeps the user's notes and the team's sources in one place for the whole project.",
                linkedMapOf("path" to "en-possessive.md")),
            RagDocument("mixed-short", "Kotlin JVM database server",
                linkedMapOf("path" to "mixed-short.md")),
            RagDocument("it-programming", "La programmazione in Kotlin sulla JVM usa il linguaggio con le coroutine e un database per il server applicativo.",
                linkedMapOf("path" to "it-programming.md")),
            RagDocument("it-repeat", "note note note markdown markdown sorgenti",
                linkedMapOf("path" to "it-repeat.md"))
        )
        val queries = listOf(
            "notebook note markdown",
            "\"note markdown\"",
            "\"note markdown\" sorgenti",
            "ricerca indice",
            "search engine embedding",
            "the user's notebook",
            "kotlin database",
            "linguaggio di programmazione",
            "Guida",
            "il lo la di",
            "editor anteprima",
            "the of and",
            "qb-rag.json"
        )
        val engine = LuceneSearchEngine()
        engine.configure(SearchOptimizationConfig(maxResults = 100))
        engine.indexAll(documents)
        write("lucene-synthetic.json", obj(
            "documents" to JsonArray(documents.map { document ->
                obj(
                    "id" to str(document.id),
                    "content" to str(document.content),
                    "metadata" to JsonObject(document.metadata.mapValues { str(it.value) })
                )
            }),
            "queries" to JsonArray(queries.mapIndexed { index, text -> scored(engine, "s-${index + 1}", text) })
        ))
        engine.close()
    }

    private fun writeProjections() {
        val store = "kubirag"
        val scratch = Files.createTempDirectory("kubirag-projections").toFile()
        val target = File(outDir, "projections").apply { mkdirs() }
        try {
            System.setProperty("QB_ROOT_PATH_OVERRIDE", scratch.path + File.separator)
            System.setProperty("QB_AUTHORITY_PATH_OVERRIDE", File(scratch, "_qb").path)
            val documents = DocumentStore()
            documents.addDocument(RagDocument(
                id = "6f1c2a52-8a43-4c1e-9d6b-0d1f6c1a9e01",
                content = "Città e \"virgolette\"\nseconda riga\tcon tab",
                metadata = linkedMapOf("filename" to "città.md", "path" to "notes/città.md", "metaTitle" to "", "checkSum" to "d41d8cd98f00b204e9800998ecf8427e"),
                resourceRef = ResourceRef(ResourceUri.uuid("6f1c2a52-8a43-4c1e-9d6b-0d1f6c1a9e01"))
            ))
            documents.addDocument(RagDocument(id = "plain", content = ""))
            documents.persist(store)
            File(scratch, "notes/$store/$store-documents.json").copyTo(File(target, "$store-documents.json"), overwrite = true)

            val vectors = InMemoryVectorStore()
            vectors.addEmbedding(Embedding("6f1c2a52-8a43-4c1e-9d6b-0d1f6c1a9e01",
                listOf(0.1f, -0.25f, 1.0E-4f, 3.4028235E38f, 0f, 1.0f / 3.0f),
                linkedMapOf("path" to "notes/città.md", "chunks" to "1", "algorithm" to "chunk-mean-v1")))
            vectors.addEmbedding(Embedding("plain", listOf(0.5f, 0.5f)))
            vectors.persist(store)
            File(scratch, "notes/$store/$store-embeddings.json").copyTo(File(target, "$store-embeddings.json"), overwrite = true)

            EmbeddingProjectionStore.write(store, EmbeddingProjectionDescriptor(
                model = "qwen3:4b", vectorDimensions = 6, chunkSize = 768, chunkOverlap = 128,
                state = EmbeddingProjectionState.CURRENT, completedAt = "2026-10-14T10:00:00Z",
                totalDocuments = 3, completedDocuments = 2, failedDocuments = 1,
                failures = listOf("plain (?): Embedder returned an empty vector")
            ))
            File(scratch, "_qb/notes/$store/embedding-profile.json").copyTo(File(target, "embedding-profile.json"), overwrite = true)
            EmbeddingProjectionStore.write(store, EmbeddingProjectionDescriptor(
                model = "qwen3:4b", chunkSize = 768, chunkOverlap = 128,
                state = EmbeddingProjectionState.BUILDING, totalDocuments = 3
            ))
            File(scratch, "_qb/notes/$store/embedding-profile.json").copyTo(File(target, "embedding-profile-building.json"), overwrite = true)
        } finally {
            System.clearProperty("QB_ROOT_PATH_OVERRIDE")
            System.clearProperty("QB_AUTHORITY_PATH_OVERRIDE")
            scratch.deleteRecursively()
        }
    }

    private fun write(name: String, element: JsonElement) {
        val file = File(outDir, name)
        file.parentFile.mkdirs()
        file.writeText(json.encodeToString(JsonElement.serializer(), element), Charsets.UTF_8)
    }

    private fun obj(vararg entries: Pair<String, JsonElement>) = JsonObject(linkedMapOf(*entries))
    private fun str(value: String?): JsonElement = value?.let(::JsonPrimitive) ?: kotlinx.serialization.json.JsonNull
}
