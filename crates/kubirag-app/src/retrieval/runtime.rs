use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use kubirag_domain::config::RagConfig;
use kubirag_domain::notebook::{RagDocument, RagNotebookManifest};
use kubirag_domain::retrieval::text::kotlin_lines;
use kubirag_domain::retrieval::{
    answer_prompt, apply_ranking_response, build_context, chunk_mean, context_block, cosine,
    estimated_context_chars, fuse_candidates, fuse_hybrid, is_artifact, pinned_context,
    pinned_prompt, rerank_prompt, Bm25DocStat, Bm25LevelStats, CacheStats, ChatTurn, ChunkConfig,
    ContextLevelStats, DocumentChunker, Embedding, EmbeddingProjectionDescriptor,
    EmbeddingProjectionStatus, EmbeddingSettings, FusionWeights, RerankLevelStats, RerankMode,
    RerankOutcome, RerankedDocStat, SearchOptions, SearchPipelineStats, SearchResult,
    SecondStageResult, CHUNK_MEAN_V1,
};
use serde_json::{json, Map, Value};

use super::{GenerateRequest, LlmError, LlmGateway, ProjectionStore, SearchEngine, VectorStore};
use crate::notebook::Clock;

const RERANK_TIMEOUT: Duration = Duration::from_millis(5000);
const SEPARATOR_WIDTH: usize = 60;

/// Esito della selezione del contesto: primo stadio, secondo stadio, documenti scelti.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextSelection {
    pub search_results: Vec<SearchResult>,
    pub second_stage: SecondStageResult,
    pub pinned_docs: Vec<RagDocument>,
    pub pinned_ids: Vec<String>,
    pub relevant_docs: Vec<RagDocument>,
    pub context: String,
    pub bm25_elapsed_ms: u128,
    pub cache_stats: CacheStats,
}

/// Una riga di `/lucene --rank`.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingRow {
    pub rank: usize,
    /// Posizione BM25 (0 = documento introdotto dal lato vettoriale).
    pub bm25_rank: usize,
    pub score: f32,
    pub path: String,
}

/// Il ranking della pipeline senza generazione (`searchRanking`).
#[derive(Debug, Clone, PartialEq)]
pub struct Ranking {
    pub mode: RerankMode,
    pub outcome: RerankOutcome,
    pub candidates: usize,
    pub rows: Vec<RankingRow>,
}

/// Le viste di retrieval del notebook attivo (`RAGSystem`): documenti materializzati, indice
/// BM25 dei chunk, vettori, e i casi d'uso che li combinano con l'LLM.
pub struct RetrievalRuntime {
    store_name: String,
    documents: IndexMap<String, RagDocument>,
    pinned: Vec<String>,
    search: Box<dyn SearchEngine>,
    vectors: Box<dyn VectorStore>,
    llm: Arc<dyn LlmGateway>,
    projections: Arc<dyn ProjectionStore>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl RetrievalRuntime {
    pub fn new(
        search: Box<dyn SearchEngine>,
        vectors: Box<dyn VectorStore>,
        llm: Arc<dyn LlmGateway>,
        projections: Arc<dyn ProjectionStore>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            store_name: String::new(),
            documents: IndexMap::new(),
            pinned: Vec::new(),
            search,
            vectors,
            llm,
            projections,
            clock,
        }
    }

    pub fn store_name(&self) -> &str {
        &self.store_name
    }

    pub fn documents(&self) -> impl Iterator<Item = &RagDocument> {
        self.documents.values()
    }

    pub fn document(&self, id: &str) -> Option<&RagDocument> {
        self.documents.get(id)
    }

    pub fn pinned_ids(&self) -> &[String] {
        &self.pinned
    }

    pub fn has_embedding(&self, id: &str) -> bool {
        self.vectors.get(id).is_some()
    }

    /// Apre un notebook: vettori locali dal disco, poi documenti e indice dal manifest.
    pub fn open(
        &mut self,
        manifest: &RagNotebookManifest,
        documents: Vec<RagDocument>,
    ) -> Result<(), String> {
        self.store_name = manifest.name().to_owned();
        self.documents.clear();
        self.vectors
            .replace_all(self.projections.load_embeddings(manifest.name())?);
        self.project(manifest, manifest, documents);
        Ok(())
    }

    /// `RagRuntimeProjection.rebuild` + `rebuildDerivedState`: dopo un commit il runtime
    /// riflette il manifest; gli embedding dei documenti tolti o cambiati sono invalidati,
    /// l'indice è ricostruito da zero (chunking di default, artefatti esclusi).
    pub fn rebuild(
        &mut self,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
        documents: Vec<RagDocument>,
    ) -> Result<(), String> {
        if !self.store_name.is_empty() && after.name() != self.store_name {
            return Err(format!(
                "Cannot project Notebook [{}] while active store is [{}]",
                after.name(),
                self.store_name
            ));
        }
        self.project(before, after, documents);
        let materialized: Vec<RagDocument> = self.documents.values().cloned().collect();
        self.projections
            .save_documents(&self.store_name, &materialized)?;
        self.persist_embeddings()
    }

    /// La parte in memoria di [`rebuild`](Self::rebuild): documenti, pin, vettori invalidati,
    /// indice. All'apertura non si scrive nulla, come `restore` in Kotlin: i file delle
    /// proiezioni cambiano solo dopo un commit o una ricostruzione esplicita.
    fn project(
        &mut self,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
        documents: Vec<RagDocument>,
    ) {
        self.store_name = after.name().to_owned();
        let after_ids: HashSet<&str> = after.documents().iter().map(|d| d.id.as_str()).collect();
        let mut before_docs: IndexMap<&str, &RagDocument> = before
            .documents()
            .iter()
            .map(|d| (d.id.as_str(), d))
            .collect();
        for stale in self.documents.values() {
            if !after_ids.contains(stale.id.as_str()) {
                before_docs.entry(stale.id.as_str()).or_insert(stale);
            }
        }
        let after_docs: IndexMap<&str, &RagDocument> = after
            .documents()
            .iter()
            .map(|d| (d.id.as_str(), d))
            .collect();
        let invalidated: Vec<String> = before_docs
            .keys()
            .filter(|id| !after_docs.contains_key(*id))
            .chain(
                after_docs
                    .iter()
                    .filter(|(id, doc)| before_docs.get(*id) != Some(doc))
                    .map(|(id, _)| id),
            )
            .map(|id| (*id).to_owned())
            .collect();

        self.documents = documents.into_iter().map(|d| (d.id.clone(), d)).collect();
        self.pinned = after
            .pinned_document_ids()
            .iter()
            .filter(|id| self.documents.contains_key(*id))
            .cloned()
            .collect();
        for id in invalidated {
            self.vectors.remove(&id);
        }
        self.reindex();
    }

    fn reindex(&mut self) {
        let chunker = DocumentChunker::default();
        let chunks: Vec<RagDocument> = self
            .documents
            .values()
            .filter(|d| !is_artifact(d))
            .flat_map(|d| chunker.chunk(d))
            .collect();
        self.search.index_all(&chunks);
    }

    fn persist_embeddings(&self) -> Result<(), String> {
        self.projections
            .save_embeddings(&self.store_name, &self.vectors.embeddings())
    }

    fn configure(&mut self, config: &RagConfig) {
        self.search.configure(SearchOptions {
            max_results: max_docs(config),
            title_boost: config.title_boost,
            query_expansion_enabled: config.query_expansion_enabled,
            diversification_enabled: config.diversification_enabled,
            phrase_query_enabled: config.phrase_query_enabled,
        });
    }

    fn path_of(&self, id: &str) -> String {
        self.documents
            .get(id)
            .and_then(|d| d.metadata.get("path"))
            .cloned()
            .unwrap_or_else(|| id.to_owned())
    }

    // ── Embedding ───────────────────────────────────────────────────────────

    pub fn embedding_status(&self, config: &RagConfig) -> EmbeddingProjectionStatus {
        EmbeddingProjectionStatus::evaluate(
            self.projections.read_profile(&self.store_name),
            settings(config),
        )
    }

    /// Un vettore per documento: media dei vettori dei chunk (`chunk-mean-v1`).
    pub fn index_document_embedding(
        &mut self,
        config: &RagConfig,
        document: &RagDocument,
    ) -> Result<usize, String> {
        let chunk_config = ChunkConfig::sized(
            usize::try_from(config.chunk_size).map_err(|e| e.to_string())?,
            usize::try_from(config.chunk_overlap).map_err(|e| e.to_string())?,
        )?;
        let chunks = DocumentChunker::new(chunk_config).chunk(document);
        let vectors = chunks
            .iter()
            .map(|chunk| {
                self.llm
                    .embed(&config.embedding_model_name, &chunk.content)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let averaged = chunk_mean(&vectors)?;
        let dimensions = averaged.len();
        let metadata: IndexMap<String, String> = [
            (
                "path".to_owned(),
                document.metadata.get("path").cloned().unwrap_or_default(),
            ),
            ("chunks".to_owned(), chunks.len().to_string()),
            ("algorithm".to_owned(), CHUNK_MEAN_V1.to_owned()),
        ]
        .into();
        self.vectors.add(Embedding {
            id: document.id.clone(),
            vector: averaged,
            metadata,
        });
        Ok(dimensions)
    }

    /// `/embed [force]`: embedding mancanti (o tutti) dei documenti non artefatto, con il
    /// profilo `BUILDING` durante il lavoro e `CURRENT`/`PARTIAL` alla fine.
    pub fn backfill_embeddings(
        &mut self,
        config: &RagConfig,
        force: bool,
    ) -> Result<(usize, usize), String> {
        let documents: Vec<RagDocument> = self
            .documents
            .values()
            .filter(|d| !is_artifact(d))
            .cloned()
            .collect();
        let base = EmbeddingProjectionDescriptor::building(
            &config.embedding_model_name,
            config.chunk_size,
            config.chunk_overlap,
            documents.len(),
        );
        self.projections.write_profile(&self.store_name, &base)?;
        let mut created = 0;
        let mut failures = Vec::new();
        let mut dimensions = None;
        for document in &documents {
            let existing = self.vectors.get(&document.id).is_some();
            if existing && !force {
                continue;
            }
            if existing {
                self.vectors.remove(&document.id);
            }
            match self.index_document_embedding(config, document) {
                Ok(dims) => {
                    dimensions = Some(dims);
                    created += 1;
                }
                Err(error) => failures.push(format!(
                    "{} ({}): {error}",
                    document.id,
                    document.metadata.get("path").map_or("?", String::as_str)
                )),
            }
        }
        self.persist_embeddings()?;
        let completed = base.completed(created, failures, dimensions, self.clock.now());
        self.projections
            .write_profile(&self.store_name, &completed)?;
        Ok((created, documents.len()))
    }

    /// Embedding lazy dei soli candidati che non ne hanno uno; gli errori non bloccano.
    fn ensure_candidate_embeddings(
        &mut self,
        config: &RagConfig,
        candidates: &[SearchResult],
    ) -> usize {
        let mut created = 0;
        for candidate in candidates {
            if self.vectors.get(&candidate.document_id).is_some() {
                continue;
            }
            let Some(document) = self.documents.get(&candidate.document_id).cloned() else {
                continue;
            };
            if self.index_document_embedding(config, &document).is_ok() {
                created += 1;
            }
        }
        created
    }

    // ── Secondo stadio ──────────────────────────────────────────────────────

    /// `applySecondStage`: modalità esplicita, altrimenti `embedding` se il rerank è attivo.
    /// Senza modalità esplicita, `embedding`/`hybrid` saltano se il profilo non è compatibile.
    pub fn apply_second_stage(
        &mut self,
        config: &RagConfig,
        question: &str,
        candidates: Vec<SearchResult>,
        requested: Option<RerankMode>,
    ) -> SecondStageResult {
        let mode = requested.unwrap_or(RerankMode::default_for(config.re_ranking_enabled));
        if requested.is_none()
            && matches!(mode, RerankMode::Embedding | RerankMode::Hybrid)
            && !self.embedding_status(config).compatible
        {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped);
        }
        let start = Instant::now();
        let mut result = match mode {
            RerankMode::None => {
                SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped)
            }
            RerankMode::Embedding => self.embedding_rerank(config, question, candidates),
            RerankMode::Hybrid => self.hybrid_fuse(config, question, candidates),
            RerankMode::Llm => self.llm_rerank(config, question, candidates),
        };
        result.elapsed_ms = start.elapsed().as_millis();
        result
    }

    fn weights(config: &RagConfig) -> FusionWeights {
        FusionWeights {
            lexical: config.hybrid_search_lucene_weight,
            vector: config.hybrid_search_vector_weight,
        }
    }

    fn embedding_rerank(
        &mut self,
        config: &RagConfig,
        question: &str,
        candidates: Vec<SearchResult>,
    ) -> SecondStageResult {
        let mode = RerankMode::Embedding;
        let total = candidates.len();
        if total <= 1 {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped)
                .with_embeddings(total, 0);
        }
        let Ok(query) = self.llm.embed(&config.embedding_model_name, question) else {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Error)
                .with_embeddings(total, 0);
        };
        let created = self.ensure_candidate_embeddings(config, &candidates);
        let available: IndexMap<String, f32> = candidates
            .iter()
            .filter_map(|c| {
                let vector = &self.vectors.get(&c.document_id)?.vector;
                (vector.len() == query.len())
                    .then(|| (c.document_id.clone(), cosine(&query, vector)))
            })
            .collect();
        if available.is_empty() {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped)
                .with_embeddings(total, created);
        }
        let fusion = fuse_candidates(&candidates, &available, Self::weights(config));
        SecondStageResult {
            outcome: RerankOutcome::comparing(&candidates, &fusion.results),
            results: fusion.results,
            mode,
            elapsed_ms: 0,
            embeddings_used: available.len(),
            embeddings_total: total,
            embeddings_created: created,
            vector_scores: available,
            fused_scores: fusion.fused_scores,
        }
    }

    fn hybrid_fuse(
        &mut self,
        config: &RagConfig,
        question: &str,
        candidates: Vec<SearchResult>,
    ) -> SecondStageResult {
        let mode = RerankMode::Hybrid;
        let total = candidates.len();
        let Ok(query) = self.llm.embed(&config.embedding_model_name, question) else {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Error)
                .with_embeddings(total, 0);
        };
        let created = self.ensure_candidate_embeddings(config, &candidates);
        let similar = match self.vectors.find_similar(
            &query,
            config.similarity_trashold,
            max_docs(config) * 2,
        ) {
            Ok(similar) => similar,
            Err(_) => {
                return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Error)
                    .with_embeddings(total, 0)
            }
        };
        if similar.is_empty() {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped)
                .with_embeddings(total, created);
        }
        let vector_scores: IndexMap<String, f32> = similar
            .iter()
            .map(|e| (e.id.clone(), cosine(&query, &e.vector)))
            .collect();
        let vector_results: Vec<SearchResult> = similar
            .iter()
            .enumerate()
            .map(|(index, e)| SearchResult {
                rank: index + 1,
                document_id: e.id.clone(),
                content: self
                    .documents
                    .get(&e.id)
                    .map(|d| d.content.clone())
                    .unwrap_or_default(),
                score: vector_scores[&e.id],
                explanation: "Vector similarity score".into(),
            })
            .collect();
        let results = fuse_hybrid(&candidates, &vector_results, Self::weights(config));
        SecondStageResult {
            outcome: RerankOutcome::comparing(&candidates, &results),
            mode,
            elapsed_ms: 0,
            embeddings_used: results
                .iter()
                .filter(|r| vector_scores.contains_key(&r.document_id))
                .count(),
            embeddings_total: results.len(),
            embeddings_created: created,
            fused_scores: results
                .iter()
                .map(|r| (r.document_id.clone(), r.score))
                .collect(),
            vector_scores,
            results,
        }
    }

    fn llm_rerank(
        &mut self,
        config: &RagConfig,
        question: &str,
        candidates: Vec<SearchResult>,
    ) -> SecondStageResult {
        let mode = RerankMode::Llm;
        if candidates.len() <= 1 {
            return SecondStageResult::passthrough(candidates, mode, RerankOutcome::Skipped);
        }
        let request = GenerateRequest {
            model: config.effective_re_ranking_model().to_owned(),
            prompt: rerank_prompt(question, &candidates),
            system: None,
            options: ollama_options(
                config,
                [
                    ("temperature", json!(0.1)),
                    ("top_p", json!(0.9)),
                    ("num_predict", json!(64)),
                ],
            ),
            format: Some("json".into()),
            think: Some(false),
            timeout: Some(RERANK_TIMEOUT),
            max_chars: max_chars(config),
        };
        match self.llm.generate(&request, &mut |_| {}) {
            Ok(response) => {
                let (results, outcome) = apply_ranking_response(&candidates, &response);
                SecondStageResult::passthrough(results, mode, outcome)
            }
            Err(LlmError::Timeout) => {
                SecondStageResult::passthrough(candidates, mode, RerankOutcome::Timeout)
            }
            Err(_) => SecondStageResult::passthrough(candidates, mode, RerankOutcome::Error),
        }
    }

    // ── Pipeline ────────────────────────────────────────────────────────────

    /// `selectSearchContext`: BM25 → secondo stadio → pinned + primi `ragMaxDocs` pertinenti.
    pub fn select_context(
        &mut self,
        config: &RagConfig,
        question: &str,
        requested: Option<RerankMode>,
    ) -> ContextSelection {
        self.configure(config);
        let start = Instant::now();
        let search_results = self.search.search(question, max_docs(config) * 2);
        let bm25_elapsed_ms = start.elapsed().as_millis();
        let cache_stats = self.search.cache_stats();
        let second_stage =
            self.apply_second_stage(config, question, search_results.clone(), requested);
        let pinned_docs: Vec<RagDocument> = self
            .pinned
            .iter()
            .filter_map(|id| self.documents.get(id))
            .filter(|d| !is_artifact(d))
            .cloned()
            .collect();
        let relevant_docs: Vec<RagDocument> = second_stage
            .results
            .iter()
            .filter(|r| !self.pinned.contains(&r.document_id))
            .filter_map(|r| self.documents.get(&r.document_id))
            .filter(|d| !is_artifact(d))
            .take(max_docs(config))
            .cloned()
            .collect();
        ContextSelection {
            context: build_context(&pinned_docs, &relevant_docs),
            search_results,
            second_stage,
            pinned_docs,
            pinned_ids: self.pinned.clone(),
            relevant_docs,
            bm25_elapsed_ms,
            cache_stats,
        }
    }

    /// `collectSearchStats`: i tre livelli di `/stat`, senza generazione.
    pub fn collect_stats(
        &mut self,
        config: &RagConfig,
        question: &str,
        requested: Option<RerankMode>,
    ) -> SearchPipelineStats {
        self.configure(config);
        let expanded_query = self.search.expanded_query(question);
        let selection = self.select_context(config, question, requested);
        let bm25 = Bm25LevelStats {
            candidates_found: selection.search_results.len(),
            results: selection
                .search_results
                .iter()
                .enumerate()
                .map(|(index, r)| Bm25DocStat {
                    rank: index + 1,
                    document_id: r.document_id.clone(),
                    score: r.score,
                    path: self.path_of(&r.document_id),
                    explanation: r.explanation.clone(),
                })
                .collect(),
            cache_stats: selection.cache_stats,
            elapsed_ms: selection.bm25_elapsed_ms,
        };
        let stage = &selection.second_stage;
        let rerank = RerankLevelStats {
            mode: stage.mode.label(),
            outcome: stage.outcome.name().to_owned(),
            results: stage
                .results
                .iter()
                .enumerate()
                .map(|(index, r)| RerankedDocStat {
                    new_rank: index + 1,
                    previous_rank: bm25_rank(&selection.search_results, &r.document_id),
                    document_id: r.document_id.clone(),
                    path: self.path_of(&r.document_id),
                    vector_score: stage.vector_scores.get(&r.document_id).copied(),
                    fused_score: stage.fused_scores.get(&r.document_id).copied(),
                })
                .collect(),
            elapsed_ms: stage.elapsed_ms,
            embeddings_used: stage.embeddings_used,
            embeddings_total: stage.embeddings_total,
            embeddings_created: stage.embeddings_created,
        };
        let context = ContextLevelStats {
            max_docs: max_docs(config),
            selected_doc_count: selection.relevant_docs.len(),
            pinned_doc_count: selection.pinned_docs.len(),
            estimated_context_chars: estimated_context_chars(
                &selection.pinned_docs,
                &selection.relevant_docs,
            ),
        };
        SearchPipelineStats {
            query: question.to_owned(),
            expanded_query,
            bm25,
            rerank,
            context,
        }
    }

    /// `searchRanking`: il ranking del secondo stadio senza artefatti (`/lucene --rank`).
    pub fn ranking(
        &mut self,
        config: &RagConfig,
        question: &str,
        requested: Option<RerankMode>,
    ) -> Ranking {
        self.configure(config);
        let candidates = self.search.search(question, max_docs(config) * 2);
        let stage = self.apply_second_stage(config, question, candidates.clone(), requested);
        let rows = stage
            .results
            .iter()
            .filter(|r| {
                self.documents
                    .get(&r.document_id)
                    .is_some_and(|d| !is_artifact(d))
            })
            .enumerate()
            .map(|(index, r)| RankingRow {
                rank: index + 1,
                bm25_rank: bm25_rank(&candidates, &r.document_id),
                score: r.score,
                path: self.path_of(&r.document_id),
            })
            .collect();
        Ranking {
            mode: stage.mode,
            outcome: stage.outcome,
            candidates: candidates.len(),
            rows,
        }
    }

    /// `queryLuceneAsyncLLM`: stampa pinned e documenti pertinenti, poi la risposta in
    /// streaming. Restituisce il testo della risposta.
    pub fn answer(
        &mut self,
        config: &RagConfig,
        question: &str,
        history: &[ChatTurn],
        system_override: Option<&str>,
        sink: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        match self.prepare_answer(config, question, history, system_override) {
            None => Ok(String::new()),
            Some(prepared) => prepared.run(self.llm.as_ref(), sink),
        }
    }

    /// La parte di [`answer`](Self::answer) che legge il notebook: selezione del contesto,
    /// righe che precedono la risposta e richiesta all'LLM. Separata dalla generazione perché
    /// il server HTTP non tenga bloccato il runtime per tutta la durata dello streaming.
    /// `None` per una domanda vuota, che non produce output.
    pub fn prepare_answer(
        &mut self,
        config: &RagConfig,
        question: &str,
        history: &[ChatTurn],
        system_override: Option<&str>,
    ) -> Option<PreparedAnswer> {
        if question.trim().is_empty() {
            return None;
        }
        let selection = self.select_context(config, question, None);
        let window = usize::try_from(config.chat_context_window_size).unwrap_or(0);
        let prompt = answer_prompt(&selection.context, history, window, question);
        let mut preamble = Vec::new();
        if !selection.pinned_docs.is_empty() {
            section(&mut preamble, "Pinned", "PINNED", &selection.pinned_docs);
        }
        if !selection.relevant_docs.is_empty() || selection.pinned_docs.is_empty() {
            section(&mut preamble, "Docs", "RELEVANT", &selection.relevant_docs);
        }
        answer_banner(&mut preamble);
        Some(PreparedAnswer {
            preamble,
            request: answer_request(config, prompt, system_override),
        })
    }

    /// `askPinnedAsyncLLM`: la chat senza retrieval, con i soli documenti pinned come
    /// contesto (anche nessuno). `None` per una domanda vuota.
    pub fn prepare_pinned_answer(
        &self,
        config: &RagConfig,
        question: &str,
        history: &[ChatTurn],
        system_override: Option<&str>,
    ) -> Option<PreparedAnswer> {
        if question.trim().is_empty() {
            return None;
        }
        let pinned: Vec<RagDocument> = self
            .pinned_documents()
            .into_iter()
            .filter(|d| !is_artifact(d))
            .collect();
        let context = context_block("PINNED", &pinned);
        let window = usize::try_from(config.chat_context_window_size).unwrap_or(0);
        let prompt = pinned_prompt(&pinned_context(&pinned), history, window, question);
        let mut preamble = vec![
            format!("Pinned[{}]:{}", pinned.len(), paths(&pinned)),
            "\n".to_owned(),
        ];
        preamble.extend(
            kotlin_lines(&context)
                .into_iter()
                .map(|line| format!("{line}\n")),
        );
        answer_banner(&mut preamble);
        Some(PreparedAnswer {
            preamble,
            request: answer_request(config, prompt, system_override),
        })
    }

    /// I documenti pinned materializzati, nell'ordine del manifest.
    pub fn pinned_documents(&self) -> Vec<RagDocument> {
        self.pinned
            .iter()
            .filter_map(|id| self.documents.get(id))
            .cloned()
            .collect()
    }

    /// Il gateway LLM, per generare fuori dal lock del runtime.
    pub fn llm(&self) -> Arc<dyn LlmGateway> {
        self.llm.clone()
    }

    /// `searchDocuments`: primo e secondo stadio senza generazione, artefatti esclusi,
    /// primi `max_results` (la barra di ricerca della SPA).
    pub fn search_documents(
        &mut self,
        config: &RagConfig,
        question: &str,
        max_results: usize,
    ) -> Vec<DocumentHit> {
        self.configure(config);
        let candidates = self.search.search(question, max_results * 2);
        let stage = self.apply_second_stage(config, question, candidates, None);
        stage
            .results
            .iter()
            .filter_map(|r| {
                let document = self.documents.get(&r.document_id)?;
                (!is_artifact(document)).then_some((r, document))
            })
            .take(max_results)
            .enumerate()
            .map(|(index, (r, document))| {
                let path = document.metadata.get("path").cloned();
                DocumentHit {
                    rank: index + 1,
                    document_id: r.document_id.clone(),
                    score: r.score,
                    filename: document
                        .metadata
                        .get("filename")
                        .cloned()
                        .or_else(|| {
                            path.as_deref()
                                .map(|p| p.rsplit('/').next().unwrap_or(p).to_owned())
                        })
                        .unwrap_or_else(|| r.document_id.clone()),
                    path: path.unwrap_or_default(),
                }
            })
            .collect()
    }
}

/// Un risultato della ricerca senza generazione (`GET /search`).
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentHit {
    pub rank: usize,
    pub document_id: String,
    pub score: f32,
    pub filename: String,
    pub path: String,
}

/// Una risposta pronta per la generazione: le righe da emettere prima (documenti e
/// intestazione) e la richiesta all'LLM, i cui frammenti seguono in streaming.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedAnswer {
    pub preamble: Vec<String>,
    pub request: GenerateRequest,
}

impl PreparedAnswer {
    /// Emette il preambolo e poi i frammenti della risposta; restituisce il testo generato.
    pub fn run(
        &self,
        llm: &dyn LlmGateway,
        sink: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        for line in &self.preamble {
            sink(line);
        }
        llm.generate(&self.request, sink)
    }
}

fn paths(docs: &[RagDocument]) -> String {
    docs.iter()
        .map(|d| d.metadata.get("path").map_or("null", String::as_str))
        .collect::<Vec<_>>()
        .join(" - ")
}

/// `Pinned[n]:<path> - <path>`, una riga vuota, poi il blocco di contesto riga per riga.
fn section(out: &mut Vec<String>, label: &str, block_label: &str, docs: &[RagDocument]) {
    out.push(format!("{label}[{}]:{}", docs.len(), paths(docs)));
    out.push("\n".to_owned());
    out.extend(
        kotlin_lines(&context_block(block_label, docs))
            .into_iter()
            .map(|line| format!("{line}\n")),
    );
}

fn answer_banner(out: &mut Vec<String>) {
    let rule = "=".repeat(SEPARATOR_WIDTH);
    out.push(format!("\n{rule}\n"));
    out.push("🤖 RISPOSTA:\n".to_owned());
    out.push(format!("{rule}\n"));
}

/// La generazione della risposta: `temperature` 0.3, `top_p` 0.9, thinking spento solo se
/// disattivato in configurazione, prompt di sistema del ruolo o della configurazione.
fn answer_request(
    config: &RagConfig,
    prompt: String,
    system_override: Option<&str>,
) -> GenerateRequest {
    GenerateRequest {
        model: config.model_name.clone(),
        prompt,
        system: system_override
            .map(str::to_owned)
            .or_else(|| config.system_prompt.clone()),
        options: ollama_options(config, [("temperature", json!(0.3)), ("top_p", json!(0.9))]),
        format: None,
        think: if config.thinking_enabled {
            None
        } else {
            Some(false)
        },
        timeout: None,
        max_chars: max_chars(config),
    }
}

fn bm25_rank(candidates: &[SearchResult], id: &str) -> usize {
    candidates
        .iter()
        .position(|c| c.document_id == id)
        .map_or(0, |index| index + 1)
}

fn max_docs(config: &RagConfig) -> usize {
    usize::try_from(config.rag_max_docs).unwrap_or(0)
}

fn max_chars(config: &RagConfig) -> usize {
    usize::try_from(config.stream_max_chars).unwrap_or(0)
}

fn settings(config: &RagConfig) -> EmbeddingSettings<'_> {
    EmbeddingSettings {
        model: &config.embedding_model_name,
        chunk_size: config.chunk_size,
        chunk_overlap: config.chunk_overlap,
    }
}

/// `RagConfig.ollamaOptions`: le opzioni della chiamata vincono; `repeat_penalty` e `num_ctx`
/// della configurazione si aggiungono solo se assenti.
pub fn ollama_options<const N: usize>(
    config: &RagConfig,
    base: [(&str, Value); N],
) -> Map<String, Value> {
    let mut options: Map<String, Value> =
        base.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    options
        .entry("repeat_penalty")
        .or_insert(json!(config.repeat_penalty));
    options
        .entry("num_ctx")
        .or_insert(json!(config.context_window));
    options
}
