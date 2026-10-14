//! Telemetria della pipeline di ricerca mostrata da `/stat` (`SearchPipelineStats`).

use super::CacheStats;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchPipelineStats {
    pub query: String,
    pub expanded_query: String,
    pub bm25: Bm25LevelStats,
    pub rerank: RerankLevelStats,
    pub context: ContextLevelStats,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bm25LevelStats {
    pub candidates_found: usize,
    pub results: Vec<Bm25DocStat>,
    pub cache_stats: CacheStats,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bm25DocStat {
    pub rank: usize,
    pub document_id: String,
    pub score: f32,
    pub path: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RerankLevelStats {
    /// Modalità effettiva: `none|embedding|hybrid|llm`.
    pub mode: String,
    /// Esito: `REORDERED/UNCHANGED/TIMEOUT/PARSE_FAILED/ERROR/SKIPPED`.
    pub outcome: String,
    pub results: Vec<RerankedDocStat>,
    pub elapsed_ms: u128,
    pub embeddings_used: usize,
    pub embeddings_total: usize,
    pub embeddings_created: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RerankedDocStat {
    pub new_rank: usize,
    /// 0 = documento introdotto dal lato vettoriale (hybrid), assente in BM25.
    pub previous_rank: usize,
    pub document_id: String,
    pub path: String,
    pub vector_score: Option<f32>,
    pub fused_score: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLevelStats {
    pub max_docs: usize,
    pub selected_doc_count: usize,
    pub pinned_doc_count: usize,
    pub estimated_context_chars: usize,
}
