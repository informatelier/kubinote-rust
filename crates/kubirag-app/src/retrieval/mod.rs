//! **Retrieval**: casi d'uso della ricerca e della risposta sopra il notebook attivo.
//!
//! [`RetrievalRuntime`] è la proiezione di retrieval del notebook (documenti materializzati,
//! indice dei chunk, vettori) e orchestra la pipeline BM25 → secondo stadio → contesto → LLM.
//! [`RuntimeProjection`] la ricostruisce dopo ogni commit del manifest; [`import_sources`]
//! trasforma file sorgente in un comando governato `REPLACE_DOCUMENTS`.

mod import;
mod ports;
mod projection;
mod runtime;

pub use import::{
    import_sources, PreparedSource, SourceDiscovery, SourceImportOptions, SourceImportSummary,
    SourceLoader, SOURCE_KEY_METADATA,
};
pub use ports::{
    DocumentMaterializer, GenerateRequest, LlmError, LlmGateway, ProjectionStore, SearchEngine,
    VectorStore,
};
pub use projection::RuntimeProjection;
pub use runtime::{ollama_options, ContextSelection, Ranking, RankingRow, RetrievalRuntime};
