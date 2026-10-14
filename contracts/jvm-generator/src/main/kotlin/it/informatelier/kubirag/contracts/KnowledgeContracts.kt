package it.informatelier.kubirag.contracts

import it.informatelier.kubilang.governance.ResourceRef
import it.informatelier.kubilang.governance.ResourceUri
import it.informatelier.kubirag.concept.AuthoritativeConceptGraphRepository
import it.informatelier.kubirag.concept.AuthorityConceptEvidence
import it.informatelier.kubirag.concept.AuthorityConceptGraphFile
import it.informatelier.kubirag.concept.AuthorityEvidenceOrigin
import it.informatelier.kubirag.concept.ConceptAuditEvent
import it.informatelier.kubirag.concept.ConceptOrigin
import it.informatelier.kubirag.concept.ConceptOriginKind
import it.informatelier.kubirag.concept.GovernedConcept
import it.informatelier.kubirag.concept.GovernedConceptRelation
import it.informatelier.kubirag.ragollama.RagConfig
import kotlinx.serialization.json.Json
import java.io.File
import java.nio.file.Files

/**
 * Golden dei file di configurazione e conoscenza che KubiRAG scrive in F1:
 * - `config/`: `.qb/qb-rag.json` scritto da `RagConfig.storeConfig` (default e personalizzato);
 * - `concept/`: `qb-concepts.json` scritto da `AuthoritativeConceptGraphRepository.write`
 *   a partire da un grafo volutamente non canonico (`rich-input.json`).
 */
class KnowledgeContracts(private val outDir: File) {
    private val governance = Json { prettyPrint = true; encodeDefaults = true }

    fun writeAll() {
        writeConfigs()
        writeConceptGraph()
    }

    private fun writeConfigs() {
        val target = File(outDir, "config").apply { mkdirs() }
        val default = RagConfig()
        val custom = RagConfig().apply {
            ollamaBaseUrl = "http://ollama.local:11434/api"
            modelName = "qwen3:8b"
            loopModel = "qwen3:0.6b"
            similarityTrashold = 0.45
            ragMaxDocs = 5
            systemPrompt = "Rispondi in italiano, \"citando\" le fonti.\nUna riga per fonte."
            currentRole = "analista"
            hybridSearchLuceneWeight = 0.6f
            hybridSearchVectorWeight = 0.4f
            titleBoost = 2.5f
            chunkSize = 1024
            repeatPenalty = 1.0
            reRankingModel = "qwen3:0.6b"
            thinkingEnabled = false
            searchBackend = "arcadedb"
            version = "1.0.0-SNAPSHOT"
        }
        listOf("default" to default, "custom" to custom).forEach { (label, config) ->
            val dir = Files.createTempDirectory("kubirag-config").toFile()
            System.setProperty("QB_ROOT_PATH_OVERRIDE", dir.path + File.separator)
            config.storeConfig()
            File(dir, "qb-rag.json").copyTo(File(target, "$label-qb-rag.json"), overwrite = true)
            dir.deleteRecursively()
        }
        System.clearProperty("QB_ROOT_PATH_OVERRIDE")
    }

    private fun writeConceptGraph() {
        val target = File(outDir, "concept").apply { mkdirs() }
        val notebook = "4cad446f-a8b1-4c2f-a762-968052b4592c"
        val beta = "b896aad9-2e69-44ae-8b64-aaf2281f4af3"
        val alfa = "367de86b-ffce-478c-a36f-8da17d353f52"
        val input = AuthorityConceptGraphFile(
            id = "6e0de910-4426-48f0-afc5-b21dded49048",
            revision = 4,
            concepts = listOf(
                GovernedConcept(
                    id = beta, name = "beta Città", description = "Concetto con \"virgolette\"\te unicode 日本 🎉",
                    aliases = listOf("Zeta", "alias", "ZETA"),
                    relations = listOf(
                        GovernedConceptRelation("relatedTo", alfa, listOf("ev-2")),
                        GovernedConceptRelation("broader", alfa),
                        GovernedConceptRelation("relatedTo", alfa, listOf("ev-2")),
                        GovernedConceptRelation(
                            "seeAlso", "external",
                            objectRef = ResourceRef(ResourceUri("https://informatelier.it/concepts/x")),
                            predicateRef = ResourceRef(ResourceUri("http://www.w3.org/2004/02/skos/core#related"))
                        )
                    ),
                    evidence = listOf(
                        AuthorityConceptEvidence(beta, "doc-2", "x-manual", title = "Manuale"),
                        AuthorityConceptEvidence(beta, "doc-1", "ev-1", snippet = "estratto", origin = null, notebookId = notebook),
                        AuthorityConceptEvidence(beta, "doc-1", "ev-1", snippet = "duplicato"),
                        AuthorityConceptEvidence(beta, "doc-3", "ev-0", origin = AuthorityEvidenceOrigin.MANUAL,
                            sourceRef = ResourceRef(ResourceUri("urn:uuid:0f8fad5b-d9cb-469f-a165-70867728950e")))
                    ),
                    notebookIds = listOf(notebook, "00000000-0000-0000-0000-000000000001", notebook),
                    origins = listOf(ConceptOrigin(ConceptOriginKind.BOUND_IMPORT, "6ba7b810-9dad-11d1-80b4-00c04fd430c8", "agg", "ent")),
                    createdAt = "2026-09-14T07:24:07.839262900Z",
                    updatedAt = "2026-09-15T08:00:00Z"
                ),
                GovernedConcept(
                    id = alfa, name = "Alfa", description = "Primo",
                    createdAt = "2026-09-14T07:35:02.668636300Z"
                )
            ),
            events = listOf(ConceptAuditEvent("2026-09-14T07:35:02.668636300Z", "CREATE", alfa, "Concept created"))
        )
        File(target, "rich-input.json").writeText(governance.encodeToString(AuthorityConceptGraphFile.serializer(), input) + "\n")
        val dir = Files.createTempDirectory("kubirag-concepts").toFile()
        AuthoritativeConceptGraphRepository(authorityDir = dir).write(input)
        File(dir, "qb-concepts.json").copyTo(File(target, "rich-concepts.json"), overwrite = true)
        dir.deleteRecursively()
    }
}
