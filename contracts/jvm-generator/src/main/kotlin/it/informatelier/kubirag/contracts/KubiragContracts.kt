package it.informatelier.kubirag.contracts

import it.informatelier.kubilang.governance.ModelIssue
import it.informatelier.kubilang.governance.ModelIssueSeverity
import it.informatelier.kubilang.governance.OperationChannel
import it.informatelier.kubilang.governance.OperationStatus
import it.informatelier.kubilang.governance.ResourceRef
import it.informatelier.kubilang.governance.ResourceUri
import it.informatelier.kubirag.lucene.DocumentChunkingService
import it.informatelier.kubirag.lucene.LuceneSearchEngine
import it.informatelier.kubirag.lucene.SearchOptimizationConfig
import it.informatelier.kubirag.ragollama.NotebookProperties
import it.informatelier.kubirag.ragollama.NotebookResourceRef
import it.informatelier.kubirag.ragollama.RagDocument
import it.informatelier.kubirag.ragollama.RagNotebookAuditEvent
import it.informatelier.kubirag.ragollama.RagNotebookAuditJournal
import it.informatelier.kubirag.ragollama.RagNotebookAuditSnapshot
import it.informatelier.kubirag.ragollama.RagNotebookCommandType
import it.informatelier.kubirag.ragollama.RagNotebookManifest
import it.informatelier.kubirag.ragollama.RagNotebookManifestReadResult
import it.informatelier.kubirag.ragollama.RagNotebookManifestRepository
import it.informatelier.kubirag.ragollama.ResourceOwnership
import it.informatelier.kubirag.ragollama.stableNotebookId
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.File
import java.nio.file.Files
import kotlin.system.exitProcess

/**
 * Generatore dei golden file di KubiRAG.
 *
 * `generate <goldenDir>` scrive, con il codice Kotlin di riferimento:
 * - `notebook/`: manifest scritti da `RagNotebookManifestRepository`, classificazione in lettura,
 *   `stableNotebookId`, audit journal del notebook (O1-KR1, O1-KR4);
 * - `retrieval/lucene-baseline.json`: risultati BM25 attuali per `retrieval/queries.json`
 *   sul corpus `retrieval/corpus/` (O2-KR1).
 */
fun main(args: Array<String>) {
    if (args.size != 2 || args[0] != "generate") {
        System.err.println("usage: generate <goldenDir>")
        exitProcess(2)
    }
    val golden = File(args[1])
    NotebookContracts(File(golden, "notebook")).writeAll()
    RetrievalBaseline(File(golden, "retrieval")).write()
    println("golden files written to ${golden.absolutePath}")
}

/** Formato dei file di vettori: un contenitore stabile, non un contratto. */
private val vectorJson = Json { prettyPrint = true; prettyPrintIndent = "  " }

private fun obj(vararg entries: Pair<String, JsonElement>) = JsonObject(linkedMapOf(*entries))
private fun str(value: String?) = value?.let(::JsonPrimitive) ?: JsonNull

private fun File.writeVectors(element: JsonElement) {
    parentFile.mkdirs()
    writeText(vectorJson.encodeToString(JsonElement.serializer(), element), Charsets.UTF_8)
}

private class NotebookContracts(private val outDir: File) {
    private val scratch = Files.createTempDirectory("kubirag-contracts").toFile()

    fun writeAll() {
        writeManifests()
        writeReadClassification()
        writeStableIds()
        writeAuditJournal()
        scratch.deleteRecursively()
    }

    private fun repository(root: File) = RagNotebookManifestRepository(qbRoot = root, localRoot = File(root, "local"))

    /** Scrive con il repository reale e copia il risultato: il byte-exact è quello del writer. */
    private fun writeManifests() {
        // I nomi dei golden restano ASCII: i nomi di file non ASCII cambiano forma su macOS (NFD).
        val manifests = mapOf(
            "empty" to RagNotebookManifest(id = "4cad446f-a8b1-4c2f-a762-968052b4592c", name = "empty"),
            "rich" to richManifest()
        )
        manifests.forEach { (label, manifest) ->
            val root = File(scratch, "manifest-${manifest.name}")
            val store = repository(root).storeDir(manifest.name)
            File(store, "notes").mkdirs()
            File(store, "notes/intro.md").writeText("# Introduzione\n\nNota gestita dal notebook.\n")
            repository(root).write(manifest)
            repository(root).manifestFile(manifest.name)
                .copyTo(File(outDir, "manifests/$label-manifest.json"), overwrite = true)
        }
    }

    private fun richManifest() = RagNotebookManifest(
        id = "9f1c2b3a-4d5e-4f60-8a7b-0c1d2e3f4a5b",
        name = "città-notebook",
        revision = 7,
        documents = listOf(
            RagDocument(
                id = "doc-intro",
                content = "# Introduzione\nCittà, perché… \"citato\"\ttab\nnuova riga 日本 🎉 \u0001",
                metadata = linkedMapOf("filename" to "intro.md", "checkSum" to "3f2a", "path" to "obsoleto.md"),
                resourceRef = ResourceRef(ResourceUri("urn:uuid:0f8fad5b-d9cb-469f-a165-70867728950e"))
            ),
            RagDocument(
                id = "doc-web",
                content = "Pagina web importata",
                metadata = linkedMapOf("filename" to "page.html", "group_name" to "web")
            ),
            RagDocument(id = "doc-bare", content = "")
        ),
        pinnedDocumentIds = listOf("doc-web", "doc-intro"),
        properties = NotebookProperties(
            author = "Nicola",
            title = "Notebook di prova",
            subtitle = "Golden file F0",
            description = "Manifest con tutti i campi valorizzati",
            coverImage = "assets/cover.png",
            backgroundImage = "https://informatelier.it/bg.jpg",
            accentColor = "#c0ffee"
        ),
        resources = listOf(
            NotebookResourceRef(
                id = "11111111-1111-4111-8111-111111111111",
                path = "notes/intro.md",
                ownership = ResourceOwnership.MANAGED,
                kind = "document-source:doc-intro"
            ),
            NotebookResourceRef(
                id = "22222222-2222-4222-8222-222222222222",
                path = "  https://informatelier.it/kubirag/page.html  ",
                ownership = ResourceOwnership.MANAGED,
                kind = "document-source:doc-web"
            ),
            NotebookResourceRef(
                id = "33333333-3333-4333-8333-333333333333",
                path = "BOUND://glossary/terms.md",
                kind = "bound-source"
            )
        )
    )

    private fun writeReadClassification() {
        val id = "4cad446f-a8b1-4c2f-a762-968052b4592c"
        val cases = linkedMapOf(
            "current-minimal" to """{"version":"1","id":"$id","name":"n"}""",
            "current-unknown-keys" to """{"version":"1","id":"$id","name":"n","future":{"x":1}}""",
            "legacy-missing-id" to """{"version":"1","name":"n"}""",
            "unsupported-version" to """{"version":"2","id":"$id","name":"n"}""",
            "version-as-number" to """{"version":1,"id":"$id","name":"n"}""",
            "missing-version" to """{"id":"$id","name":"n"}""",
            "root-array" to """[]""",
            "malformed" to """{""",
            "uppercase-id" to """{"version":"1","id":"${id.uppercase()}","name":"n"}""",
            "blank-name" to """{"version":"1","id":"$id","name":"  "}""",
            "negative-revision" to """{"version":"1","id":"$id","name":"n","revision":-1}""",
            "orphan-pin" to """{"version":"1","id":"$id","name":"n","pinnedDocumentIds":["x"]}""",
            "duplicate-pin" to """{"version":"1","id":"$id","name":"n","documents":[{"id":"x","content":""}],"pinnedDocumentIds":["x","x"]}""",
            "duplicate-document" to """{"version":"1","id":"$id","name":"n","documents":[{"id":"x","content":""},{"id":"x","content":""}]}""",
            "blank-document-id" to """{"version":"1","id":"$id","name":"n","documents":[{"id":" ","content":""}]}""",
            "duplicate-resource" to """{"version":"1","id":"$id","name":"n","resources":[{"id":"r","path":"https://a.b/c"},{"id":"r","path":"https://a.b/d"}]}""",
            "invalid-resource-uri" to """{"version":"1","id":"$id","name":"n","documents":[{"id":"x","content":"","resourceRef":{"uri":"relative"}}]}"""
        )
        val vectors = cases.map { (case, text) ->
            val root = File(scratch, "read-$case")
            val repository = repository(root)
            repository.manifestFile("n").apply { parentFile.mkdirs(); writeText(text) }
            val (kind, detail) = when (val result = repository.read("n")) {
                is RagNotebookManifestReadResult.Current -> "Current" to null
                is RagNotebookManifestReadResult.LegacyMissingIdentity -> "LegacyMissingIdentity" to null
                is RagNotebookManifestReadResult.UnsupportedVersion -> "UnsupportedVersion" to result.version
                is RagNotebookManifestReadResult.Invalid -> "Invalid" to result.diagnostic
                RagNotebookManifestReadResult.Missing -> "Missing" to null
            }
            obj("case" to str(case), "input" to str(text), "expected" to str(kind), "detail" to str(detail))
        }
        File(outDir, "read-classification.json").writeVectors(obj(
            "rule" to str("RagNotebookManifestRepository.read: classificazione del manifest autoritativo"),
            "vectors" to JsonArray(vectors)
        ))
    }

    private fun writeStableIds() {
        val names = listOf("kubirag", "a", "Città di Bolzano", "日本語", "notebook with spaces", "emoji 🎉")
        File(outDir, "stable-notebook-ids.json").writeVectors(obj(
            "rule" to str("stableNotebookId(name) = UUID.nameUUIDFromBytes(\"qb-rag-notebook\\u001f<name>\")"),
            "vectors" to JsonArray(names.map { obj("name" to str(it), "expected" to str(stableNotebookId(it))) })
        ))
    }

    private fun writeAuditJournal() {
        val root = File(scratch, "audit")
        val file = File(root, "governance/rag/notebooks/9f1c2b3a-4d5e-4f60-8a7b-0c1d2e3f4a5b.json")
        val journal = RagNotebookAuditJournal(file, File(root, "local"))
        val notebookId = "9f1c2b3a-4d5e-4f60-8a7b-0c1d2e3f4a5b"
        val empty = RagNotebookAuditSnapshot(notebookId, 0, emptyMap(), emptyList(), emptyList())
        val one = RagNotebookAuditSnapshot(
            notebookId, 1,
            linkedMapOf("doc-intro" to "a3f1c2"),
            listOf("doc-intro"),
            listOf("11111111-1111-4111-8111-111111111111")
        )
        journal.append(RagNotebookAuditEvent(
            id = "53387d9a-04bd-4fb7-b0e4-9062468f148c",
            command = RagNotebookCommandType.ADD_AND_PIN_DOCUMENT,
            targetId = "doc-intro",
            actor = "api-web",
            channel = OperationChannel.API_WEB,
            correlationId = "e80aa074-82ab-42b9-875e-faec057e7cd5",
            timestamp = "2026-09-02T18:04:37.234346400Z",
            expectedRevision = 0,
            previousRevision = 0,
            currentRevision = 1,
            status = OperationStatus.APPLIED,
            issues = listOf(ModelIssue(
                "RAG_RESOURCE_MISSING", ModelIssueSeverity.WARNING,
                "resources/11111111-1111-4111-8111-111111111111/path",
                "Registered resource is missing: .qb\\notes\\città.md", true
            )),
            before = empty,
            after = one
        ))
        journal.append(RagNotebookAuditEvent(
            id = "7d0c5e8b-1f2a-4b3c-9d4e-5f6a7b8c9d0e",
            command = RagNotebookCommandType.UNPIN,
            targetId = "doc-intro",
            actor = "nicola",
            channel = OperationChannel.REPL,
            correlationId = "1b2c3d4e-5f60-4718-8293-a4b5c6d7e8f9",
            timestamp = "2026-09-03T08:00:00Z",
            expectedRevision = 0,
            previousRevision = 1,
            currentRevision = 1,
            status = OperationStatus.CONFLICTED,
            diagnostic = "RAG Notebook revision conflict: expected 0, actual 1",
            before = one,
            after = one
        ))
        file.copyTo(File(outDir, "audit/notebook-audit.json"), overwrite = true)
    }
}

private class RetrievalBaseline(private val outDir: File) {
    private val config = SearchOptimizationConfig()
    private val maxResults = 10

    fun write() {
        val corpusDir = File(outDir, "corpus")
        val sources = corpusDir.walkTopDown().filter { it.isFile && it.extension == "md" }
            .sortedBy { it.relativeTo(corpusDir).invariantSeparatorsPath }.toList()
        require(sources.isNotEmpty()) { "Corpus vuoto: ${corpusDir.absolutePath}" }
        val chunker = DocumentChunkingService()
        val chunks = sources.flatMap { source ->
            val path = source.relativeTo(corpusDir).invariantSeparatorsPath
            chunker.chunkDocument(RagDocument(
                id = path,
                content = source.readText(Charsets.UTF_8),
                metadata = linkedMapOf("filename" to source.name, "path" to path)
            ))
        }
        val queries = Json.parseToJsonElement(File(outDir, "queries.json").readText())
            .jsonObject.getValue("queries").jsonArray.map { it.jsonObject }

        val engine = LuceneSearchEngine()
        engine.configure(config)
        engine.indexAll(chunks)
        val results = queries.map { query ->
            val text = query.getValue("text").jsonPrimitive.content
            val hits = runBlocking { engine.search(text, maxResults) }
            obj(
                "id" to query.getValue("id"),
                "text" to str(text),
                "expandedQuery" to str(engine.expandedQuery(text)),
                "results" to JsonArray(hits.map { hit ->
                    obj(
                        "rank" to JsonPrimitive(hit.rank),
                        "chunkId" to str(hit.documentId),
                        "score" to JsonPrimitive(hit.score)
                    )
                })
            )
        }
        engine.close()

        File(outDir, "lucene-baseline.json").writeVectors(obj(
            "engine" to str("kubirag LuceneSearchEngine (BM25, ItalianAnalyzer/EnglishAnalyzer, espansione query, diversificazione)"),
            "config" to obj(
                "maxResults" to JsonPrimitive(maxResults),
                "titleBoost" to JsonPrimitive(config.titleBoost),
                "queryExpansionEnabled" to JsonPrimitive(config.queryExpansionEnabled),
                "phraseQueryEnabled" to JsonPrimitive(config.phraseQueryEnabled),
                "phraseQueryBoost" to JsonPrimitive(config.phraseQueryBoost),
                "diversificationEnabled" to JsonPrimitive(config.diversificationEnabled)
            ),
            "corpus" to obj(
                "sources" to JsonPrimitive(sources.size),
                "chunks" to JsonPrimitive(chunks.size),
                "chunking" to str("DocumentChunkingService() default; chunkId = <path>_chunk_<n>, o <path> se non spezzato")
            ),
            "queries" to JsonArray(results)
        ))
    }
}
