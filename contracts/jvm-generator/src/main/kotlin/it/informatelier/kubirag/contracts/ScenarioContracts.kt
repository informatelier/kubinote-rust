package it.informatelier.kubirag.contracts

import it.informatelier.kubilang.governance.OperationChannel
import it.informatelier.kubilang.governance.OperationContext
import it.informatelier.kubilang.governance.OperationResult
import it.informatelier.kubirag.ragollama.NotebookProperties
import it.informatelier.kubirag.ragollama.RagDocument
import it.informatelier.kubirag.ragollama.RagNotebookApplicationService
import it.informatelier.kubirag.ragollama.RagNotebookAuditJournal
import it.informatelier.kubirag.ragollama.RagNotebookManifest
import it.informatelier.kubirag.ragollama.RagNotebookManifestManager
import it.informatelier.kubirag.ragollama.RagNotebookManifestRepository
import it.informatelier.kubirag.ragollama.RagNotebookProjection
import it.informatelier.kubirag.ragollama.ResourceOwnership
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import java.io.File
import java.nio.file.Files

/** Timestamp fisso che sostituisce `Instant.now()` negli eventi: il contratto non è l'orologio. */
const val SCENARIO_TIMESTAMP = "2026-10-14T00:00:00Z"

/**
 * Esegue gli scenari di `notebook/scenarios/scenarios.json` con il servizio applicativo
 * Kotlin reale e scrive, per ogni scenario, `expected/<nome>/`:
 * - `manifest.json`: il manifest finale, byte per byte come lo scrive il repository;
 * - `audit.json`: il journal, con id evento `event-<n>` e timestamp [SCENARIO_TIMESTAMP];
 * - `results.json`: l'`OperationResult` di ogni comando, con lo stesso id evento.
 *
 * Gli id evento e i timestamp sono casuali in Kotlin; la versione Rust li riceve da porte
 * (`IdGenerator`, `Clock`) e nei test usa la stessa numerazione.
 */
class ScenarioContracts(private val outDir: File) {
    private val json = Json { ignoreUnknownKeys = true }
    private val governance = Json { prettyPrint = true; encodeDefaults = true }
    private val vectors = Json { prettyPrint = true; prettyPrintIndent = "  "; encodeDefaults = true }

    fun writeAll() {
        val scenarios = json.parseToJsonElement(File(outDir, "scenarios.json").readText())
            .jsonObject.getValue("scenarios").jsonArray.map { it.jsonObject }
        scenarios.forEach(::run)
    }

    private fun run(scenario: JsonObject) {
        val name = scenario.string("name")
        val notebook = scenario.getValue("notebook").jsonObject
        val root = Files.createTempDirectory("kubirag-scenario").toFile()
        val repository = RagNotebookManifestRepository(qbRoot = root, localRoot = File(root, "local"))
        val notebookName = notebook.string("name")
        val store = repository.storeDir(notebookName)
        scenario["files"]?.jsonObject?.forEach { (path, content) ->
            File(store, path).apply { parentFile.mkdirs(); writeText(content.jsonPrimitive.content) }
        }
        repository.write(RagNotebookManifest(id = notebook.string("id"), name = notebookName))
        val manager = RagNotebookManifestManager(repository).also { it.load(notebookName) }
        val auditFile = File(store, "$notebookName-audit.json")
        val service = RagNotebookApplicationService(manager, RagNotebookProjection.None, RagNotebookAuditJournal(auditFile))

        val results = mutableListOf<Pair<Int, OperationResult>>()
        scenario.getValue("steps").jsonArray.forEachIndexed { index, element ->
            val step = element.jsonObject
            if (step.containsKey("file")) {
                val file = File(store, step.string("file"))
                if (step["delete"]?.jsonPrimitive?.boolean == true) file.delete() else file.writeText(step.string("write"))
            } else {
                results += index to execute(service, step, index)
            }
        }

        val target = File(outDir, "expected/$name").apply { mkdirs() }
        repository.manifestFile(notebookName).copyTo(File(target, "manifest.json"), overwrite = true)
        val (audit, ids) = normalizedAudit(auditFile)
        File(target, "audit.json").writeText(audit)
        File(target, "results.json").writeText(vectors.encodeToString(JsonElement.serializer(), JsonObject(linkedMapOf(
            "scenario" to JsonPrimitive(name),
            "results" to JsonArray(results.map { (index, result) ->
                val encoded = vectors.encodeToJsonElement(OperationResult.serializer(), result).jsonObject
                JsonObject(linkedMapOf<String, JsonElement>("step" to JsonPrimitive(index)) + encoded.mapValues { (key, value) ->
                    if (key == "auditEventId" && value is JsonPrimitive && value.isString) JsonPrimitive(ids.getValue(value.content)) else value
                })
            })
        ))))
        root.deleteRecursively()
    }

    /** Sostituisce id evento e timestamp, lasciando invariato il resto dei byte. */
    private fun normalizedAudit(file: File): Pair<String, Map<String, String>> {
        val root = json.parseToJsonElement(file.readText()).jsonObject
        val events = root.getValue("events").jsonArray.map { it.jsonObject }
        val ids = events.mapIndexed { index, event -> event.string("id") to "event-${index + 1}" }.toMap()
        val normalized = JsonObject(linkedMapOf(
            "version" to root.getValue("version"),
            "events" to JsonArray(events.map { event ->
                JsonObject(event.mapValues { (key, value) ->
                    when (key) {
                        "id" -> JsonPrimitive(ids.getValue(value.jsonPrimitive.content))
                        "timestamp" -> JsonPrimitive(SCENARIO_TIMESTAMP)
                        else -> value
                    }
                })
            })
        ))
        return governance.encodeToString(JsonElement.serializer(), normalized) to ids
    }

    private fun execute(service: RagNotebookApplicationService, step: JsonObject, index: Int): OperationResult {
        val context = OperationContext(
            actor = step["actor"]?.jsonPrimitive?.content ?: "nicola",
            channel = OperationChannel.valueOf(step["channel"]?.jsonPrimitive?.content ?: "CLI"),
            expectedRevision = step.getValue("expectedRevision").jsonPrimitive.long,
            correlationId = "corr-$index"
        )
        val ownership = ResourceOwnership.valueOf(step["ownership"]?.jsonPrimitive?.content ?: "EXTERNAL")
        return when (val command = step.string("command")) {
            "addDocument" -> service.addDocument(document(step), context, ownership)
            "addAndPinDocument" -> service.addAndPinDocument(document(step), context, ownership)
            "addDocumentsAndSetPins" -> service.addDocumentsAndSetPins(documents(step), strings(step, "pinIds").toSet(), context, ownership)
            "replaceDocument" -> service.replaceDocument(document(step), context, ownership)
            "replaceDocuments" -> service.replaceDocuments(
                strings(step, "removeIds").toSet(), documents(step), context, ownership,
                step["preserveReplacementPins"]?.jsonPrimitive?.boolean ?: false
            )
            "removeDocument" -> service.removeDocument(step.string("id"), context)
            "removeDocuments" -> service.removeDocuments(strings(step, "ids").toSet(), context)
            "pin" -> service.pin(step.string("id"), context)
            "unpin" -> service.unpin(step.string("id"), context)
            "setPins" -> service.setPins(strings(step, "ids").toSet(), step.getValue("pinned").jsonPrimitive.boolean, context)
            "updateProperties" -> service.updateProperties(
                json.decodeFromJsonElement(NotebookProperties.serializer(), step.getValue("properties")), context
            )
            "refreshExternal" -> service.refreshExternal(document(step), context)
            "rebuildProjections" -> service.rebuildProjections(context)
            "undo" -> service.undoUnavailable(context)
            "reconcile" -> service.reconcile(
                json.decodeFromJsonElement(RagNotebookManifest.serializer(), step.getValue("candidate")), context
            )
            else -> error("Unknown scenario command $command")
        }
    }

    private fun document(step: JsonObject) = json.decodeFromJsonElement(RagDocument.serializer(), step.getValue("document"))
    private fun documents(step: JsonObject) = step.getValue("documents").jsonArray.map { json.decodeFromJsonElement(RagDocument.serializer(), it) }
    private fun strings(step: JsonObject, key: String) = step.getValue(key).jsonArray.map { it.jsonPrimitive.content }
    private fun JsonObject.string(key: String) = getValue(key).jsonPrimitive.content
}
