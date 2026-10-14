use std::time::Duration;

use kubirag_domain::notebook::{RagDocument, RagNotebookManifest};
use kubirag_domain::retrieval::{
    CacheStats, Embedding, EmbeddingProjectionDescriptor, SearchOptions, SearchResult,
};
use serde_json::{Map, Value};

/// Primo stadio: indice lessicale dei chunk (`SearchEngine`).
pub trait SearchEngine: Send {
    fn configure(&mut self, options: SearchOptions);
    /// Ricostruisce l'indice da zero; azzera le statistiche della cache.
    fn index_all(&mut self, chunks: &[RagDocument]);
    /// Al più `max_results` documenti padre, diversificati, in ordine di punteggio.
    fn search(&mut self, query: &str, max_results: usize) -> Vec<SearchResult>;
    /// La query come la vede il motore dopo l'espansione (`/stat`).
    fn expanded_query(&self, query: &str) -> String;
    fn cache_stats(&self) -> CacheStats;
}

/// Vettori dei documenti (`VectorStore`): uno per documento, confrontati col coseno.
pub trait VectorStore: Send {
    fn add(&mut self, embedding: Embedding);
    fn remove(&mut self, id: &str) -> Option<Embedding>;
    fn get(&self, id: &str) -> Option<&Embedding>;
    /// Coseno ≥ `threshold`, decrescente, primi `limit`. Errore se le dimensioni non coincidono.
    fn find_similar(
        &self,
        query: &[f32],
        threshold: f64,
        limit: usize,
    ) -> Result<Vec<Embedding>, String>;
    fn embeddings(&self) -> Vec<Embedding>;
    fn replace_all(&mut self, embeddings: Vec<Embedding>);
}

/// Errore del modello linguistico. `Timeout` è distinto perché il rerank lo riporta come esito.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlmError {
    #[error("LLM request timed out")]
    Timeout,
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Protocol(String),
}

/// Una richiesta di generazione in streaming (`/api/generate`).
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateRequest {
    pub model: String,
    pub prompt: String,
    pub system: Option<String>,
    pub options: Map<String, Value>,
    pub format: Option<String>,
    /// `None` lascia il default di Ollama, `Some(false)` disattiva il thinking.
    pub think: Option<bool>,
    /// Tempo massimo dell'intera risposta (il rerank usa 5 s).
    pub timeout: Option<Duration>,
    /// Interruttore sul contenuto: lo stream si ferma oltre questi caratteri.
    pub max_chars: usize,
}

/// Porta verso il modello linguistico (Ollama).
pub trait LlmGateway: Send + Sync {
    /// Chiama `on_chunk` per ogni frammento non vuoto e restituisce il testo completo.
    fn generate(
        &self,
        request: &GenerateRequest,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError>;

    fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>, LlmError>;
}

/// Viste derivate di un notebook su disco: documenti e vettori locali, profilo portabile.
pub trait ProjectionStore: Send + Sync {
    fn save_documents(&self, store: &str, documents: &[RagDocument]) -> Result<(), String>;
    fn load_embeddings(&self, store: &str) -> Result<Vec<Embedding>, String>;
    fn save_embeddings(&self, store: &str, embeddings: &[Embedding]) -> Result<(), String>;
    fn read_profile(&self, store: &str) -> Option<EmbeddingProjectionDescriptor>;
    fn write_profile(
        &self,
        store: &str,
        descriptor: &EmbeddingProjectionDescriptor,
    ) -> Result<(), String>;
}

/// Documenti a runtime dal manifest: le note gestite sono rilette dai file del notebook.
pub trait DocumentMaterializer: Send + Sync {
    fn materialize(&self, manifest: &RagNotebookManifest) -> Result<Vec<RagDocument>, String>;
}
