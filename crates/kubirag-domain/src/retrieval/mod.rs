//! **Retrieval** (bounded context): dal corpus del notebook al contesto dell'LLM.
//!
//! Primo stadio BM25 sui chunk, secondo stadio (`none | embedding | hybrid | llm`) che
//! riordina o fonde, selezione del contesto (pinned prima, artefatti esclusi) e prompt di
//! risposta. Qui ci sono solo le regole pure; indice, vettori e LLM sono porte dell'app.

mod chunking;
mod context;
mod embedding;
mod fusion;
mod language;
mod llm_rerank;
mod search;
mod stats;
pub mod text;

pub use chunking::{ChunkConfig, DocumentChunker, CHUNK_INDEX, PARENT_CHUNK_COUNT, PARENT_ID};
pub use context::{
    answer_prompt, build_context, context_block, context_entry, estimated_context_chars,
    is_artifact, is_artifact_document, pinned_context, pinned_prompt, ChatTurn,
};
pub use embedding::{
    chunk_mean, Embedding, EmbeddingProjectionDescriptor, EmbeddingProjectionState,
    EmbeddingProjectionStatus, EmbeddingSettings, CHUNK_MEAN_V1,
};
pub use fusion::{
    cosine, diversify, fuse_candidates, fuse_hybrid, normalize, CandidateFusion, FusionWeights,
};
pub use language::{detect_language, Language, LanguageDetection, CONFIDENCE_THRESHOLD};
pub use llm_rerank::{
    apply_ranking_response, parse_ranking_indices, reorder_by_indices, rerank_prompt,
    RankingOverflow,
};
pub use search::{
    CacheStats, RerankMode, RerankOutcome, SearchOptions, SearchResult, SecondStageResult,
};
pub use stats::{
    Bm25DocStat, Bm25LevelStats, ContextLevelStats, RerankLevelStats, RerankedDocStat,
    SearchPipelineStats,
};
