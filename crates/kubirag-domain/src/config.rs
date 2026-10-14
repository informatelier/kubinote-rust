//! Configurazione di KubiRAG (`.qb/qb-rag.json`): modelli Ollama, parametri di retrieval e di
//! generazione, backend. È globale all'applicazione, non per notebook.
//!
//! Il file è scritto con `Json { prettyPrint }` (4 spazi, valori di default omessi): per
//! questo il tipo sa dire quali campi hanno il valore di default Kotlin.

use kubi_kernel::json::kotlin_float;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const DEFAULT_BACKEND: &str = "default";
const BACKENDS: [&str; 2] = [DEFAULT_BACKEND, "arcadedb"];

/// Campi nell'ordine delle proprietà della classe Kotlin `RagConfig`, che è l'ordine del file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RagConfig {
    pub ollama_base_url: String,
    pub embedding_model_name: String,
    pub model_name: String,
    /// Modello per l'arricchimento in background; vuoto = `model_name`.
    pub loop_model: String,
    /// Il refuso è nel formato del file.
    pub similarity_trashold: f64,
    pub rag_max_docs: i32,
    pub debug: bool,
    pub system_prompt: Option<String>,
    pub current_role: Option<String>,
    /// Autorità HTTP(S) stabile usata solo per i permalink FAIR promossi esplicitamente.
    pub public_base_uri: String,
    pub chat_context_window_size: i32,
    pub chat_context_max_tokens: i32,
    /// `num_ctx` di Ollama, in token.
    pub context_window: i32,
    pub hybrid_search_enabled: bool,
    pub hybrid_search_lucene_weight: f32,
    pub hybrid_search_vector_weight: f32,
    pub chunk_size: i32,
    pub chunk_overlap: i32,
    pub title_boost: f32,
    pub query_expansion_enabled: bool,
    pub diversification_enabled: bool,
    pub re_ranking_enabled: bool,
    /// Modello del rerank LLM; vuoto = `model_name`.
    pub re_ranking_model: String,
    pub phrase_query_enabled: bool,
    pub thinking_enabled: bool,
    pub stream_max_chars: i32,
    pub repeat_penalty: f64,
    pub structured_output: bool,
    pub role_file_pattern: String,
    pub role_exclude_pattern: String,
    pub role_reduce_prompt: String,
    pub save_analysis_result: bool,
    pub max_analysis_tokens: i32,
    pub pdf_default_pages_per_chunk: i32,
    pub search_backend: String,
    pub vector_backend: String,
    pub document_backend: String,
    /// Build che ha scritto il file (`unknown` per i file legacy).
    pub version: String,
    /// Proprietà privata di `RagConfig` con backing field: kotlinx la serializza (mai, in
    /// pratica, perché vale sempre il default).
    pub config_filename: String,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            ollama_base_url: "http://localhost:11434/api".into(),
            embedding_model_name: "qwen3:4b".into(),
            model_name: "qwen3:4b".into(),
            loop_model: String::new(),
            similarity_trashold: 0.3,
            rag_max_docs: 3,
            debug: false,
            system_prompt: None,
            current_role: None,
            public_base_uri: String::new(),
            chat_context_window_size: 20,
            chat_context_max_tokens: 3000,
            context_window: 4096,
            hybrid_search_enabled: true,
            hybrid_search_lucene_weight: 0.7,
            hybrid_search_vector_weight: 0.3,
            chunk_size: 768,
            chunk_overlap: 128,
            title_boost: 2.0,
            query_expansion_enabled: true,
            diversification_enabled: true,
            re_ranking_enabled: true,
            re_ranking_model: String::new(),
            phrase_query_enabled: true,
            thinking_enabled: true,
            stream_max_chars: 64_000,
            repeat_penalty: 1.2,
            structured_output: false,
            role_file_pattern: String::new(),
            role_exclude_pattern: String::new(),
            role_reduce_prompt: String::new(),
            save_analysis_result: true,
            max_analysis_tokens: 500,
            pdf_default_pages_per_chunk: 20,
            search_backend: DEFAULT_BACKEND.into(),
            vector_backend: DEFAULT_BACKEND.into(),
            document_backend: DEFAULT_BACKEND.into(),
            version: "unknown".into(),
            config_filename: "qb-rag.json".into(),
        }
    }
}

impl RagConfig {
    pub fn effective_loop_model(&self) -> &str {
        non_blank_or(&self.loop_model, &self.model_name)
    }

    pub fn effective_re_ranking_model(&self) -> &str {
        non_blank_or(&self.re_ranking_model, &self.model_name)
    }

    /// Tutti i campi, in ordine, con i `Float` scritti come `Float.toString()`.
    pub fn to_fields(&self) -> Map<String, Value> {
        let mut fields = Map::new();
        let mut put = |name: &str, value: Value| {
            fields.insert(name.to_owned(), value);
        };
        put("ollamaBaseUrl", self.ollama_base_url.clone().into());
        put(
            "embeddingModelName",
            self.embedding_model_name.clone().into(),
        );
        put("modelName", self.model_name.clone().into());
        put("loopModel", self.loop_model.clone().into());
        put("similarityTrashold", self.similarity_trashold.into());
        put("ragMaxDocs", self.rag_max_docs.into());
        put("debug", self.debug.into());
        put("systemPrompt", self.system_prompt.clone().into());
        put("currentRole", self.current_role.clone().into());
        put("publicBaseUri", self.public_base_uri.clone().into());
        put(
            "chatContextWindowSize",
            self.chat_context_window_size.into(),
        );
        put("chatContextMaxTokens", self.chat_context_max_tokens.into());
        put("contextWindow", self.context_window.into());
        put("hybridSearchEnabled", self.hybrid_search_enabled.into());
        put(
            "hybridSearchLuceneWeight",
            kotlin_float(self.hybrid_search_lucene_weight),
        );
        put(
            "hybridSearchVectorWeight",
            kotlin_float(self.hybrid_search_vector_weight),
        );
        put("chunkSize", self.chunk_size.into());
        put("chunkOverlap", self.chunk_overlap.into());
        put("titleBoost", kotlin_float(self.title_boost));
        put("queryExpansionEnabled", self.query_expansion_enabled.into());
        put(
            "diversificationEnabled",
            self.diversification_enabled.into(),
        );
        put("reRankingEnabled", self.re_ranking_enabled.into());
        put("reRankingModel", self.re_ranking_model.clone().into());
        put("phraseQueryEnabled", self.phrase_query_enabled.into());
        put("thinkingEnabled", self.thinking_enabled.into());
        put("streamMaxChars", self.stream_max_chars.into());
        put("repeatPenalty", self.repeat_penalty.into());
        put("structuredOutput", self.structured_output.into());
        put("roleFilePattern", self.role_file_pattern.clone().into());
        put(
            "roleExcludePattern",
            self.role_exclude_pattern.clone().into(),
        );
        put("roleReducePrompt", self.role_reduce_prompt.clone().into());
        put("saveAnalysisResult", self.save_analysis_result.into());
        put("maxAnalysisTokens", self.max_analysis_tokens.into());
        put(
            "pdfDefaultPagesPerChunk",
            self.pdf_default_pages_per_chunk.into(),
        );
        put("searchBackend", self.search_backend.clone().into());
        put("vectorBackend", self.vector_backend.clone().into());
        put("documentBackend", self.document_backend.clone().into());
        put("version", self.version.clone().into());
        put("configFilename", self.config_filename.clone().into());
        fields
    }

    /// `validateConfig()`: i vincoli controllati al ripristino, con i messaggi Kotlin.
    pub fn validate(&self) -> Result<(), String> {
        let checks: [(bool, String); 14] = [
            (
                (1..=50).contains(&self.rag_max_docs),
                format!(
                    "ragMaxDocs must be between 1 and 50, got: {}",
                    self.rag_max_docs
                ),
            ),
            (
                (0.0..=1.0).contains(&self.hybrid_search_lucene_weight),
                format!(
                    "hybridSearchLuceneWeight must be between 0.0 and 1.0, got: {}",
                    self.hybrid_search_lucene_weight
                ),
            ),
            (
                (0.0..=1.0).contains(&self.hybrid_search_vector_weight),
                format!(
                    "hybridSearchVectorWeight must be between 0.0 and 1.0, got: {}",
                    self.hybrid_search_vector_weight
                ),
            ),
            (
                (self.hybrid_search_lucene_weight + self.hybrid_search_vector_weight - 1.0).abs()
                    < 0.001,
                format!(
                    "hybrid search weights must sum to 1.0, got: {}",
                    self.hybrid_search_lucene_weight + self.hybrid_search_vector_weight
                ),
            ),
            (
                (100..=2000).contains(&self.chunk_size),
                format!(
                    "chunkSize must be between 100 and 2000, got: {}",
                    self.chunk_size
                ),
            ),
            (
                self.chunk_overlap >= 0,
                format!(
                    "chunkOverlap must be non-negative, got: {}",
                    self.chunk_overlap
                ),
            ),
            (
                self.chunk_overlap < self.chunk_size,
                format!(
                    "chunkOverlap ({}) must be less than chunkSize ({})",
                    self.chunk_overlap, self.chunk_size
                ),
            ),
            (
                self.title_boost >= 0.0,
                format!("titleBoost must be non-negative, got: {}", self.title_boost),
            ),
            (
                (1_000..=10_000_000).contains(&self.stream_max_chars),
                format!(
                    "streamMaxChars must be between 1_000 and 10_000_000, got: {}",
                    self.stream_max_chars
                ),
            ),
            (
                (0.5..=2.0).contains(&self.repeat_penalty),
                format!(
                    "repeatPenalty must be between 0.5 and 2.0, got: {}",
                    self.repeat_penalty
                ),
            ),
            (
                (256..=1_000_000).contains(&self.context_window),
                format!(
                    "contextWindow must be between 256 and 1_000_000, got: {}",
                    self.context_window
                ),
            ),
            backend_check("searchBackend", &self.search_backend),
            backend_check("vectorBackend", &self.vector_backend),
            backend_check("documentBackend", &self.document_backend),
        ];
        checks
            .into_iter()
            .find(|(valid, _)| !valid)
            .map_or(Ok(()), |(_, message)| Err(message))
    }
}

fn backend_check(name: &str, value: &str) -> (bool, String) {
    (
        BACKENDS.contains(&value),
        format!("{name} must be one of [default, arcadedb], got: {value}"),
    )
}

fn non_blank_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

impl Serialize for RagConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_fields().serialize(serializer)
    }
}

impl kubi_kernel::json::KotlinDefaults for RagConfig {
    fn is_kotlin_default(field: &str, value: &Value) -> bool {
        RagConfig::default().to_fields().get(field) == Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(RagConfig::default().validate(), Ok(()));
    }

    #[test]
    fn weights_must_sum_to_one() {
        let config = RagConfig {
            hybrid_search_vector_weight: 0.5,
            ..RagConfig::default()
        };
        assert!(config
            .validate()
            .unwrap_err()
            .starts_with("hybrid search weights"));
    }
}
