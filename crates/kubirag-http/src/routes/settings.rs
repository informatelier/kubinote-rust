//! **Settings**: la configurazione condivisa (`CtrlConfig`) e i modelli di Ollama
//! (`CtrlModels`).

use axum::extract::State;
use axum::response::Response;
use kubirag_app::retrieval::ModelCatalog;
use kubirag_domain::config::SettingsUpdate;
use serde::Deserialize;
use serde_json::json;

use crate::respond::{self, decode, ok};
use crate::state::AppState;

pub async fn config(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            let c = &s.config;
            ok(&json!({
                "ollamaBaseUrl": c.ollama_base_url,
                "modelName": c.model_name,
                "embeddingModelName": c.embedding_model_name,
                "ragMaxDocs": c.rag_max_docs,
                "similarityTrashold": c.similarity_trashold,
                "chatContextWindowSize": c.chat_context_window_size,
                "chatContextMaxTokens": c.chat_context_max_tokens,
                "contextWindow": c.context_window,
                "chunkSize": c.chunk_size,
                "chunkOverlap": c.chunk_overlap,
                "titleBoost": crate::respond::float(c.title_boost),
                "queryExpansionEnabled": c.query_expansion_enabled,
                "diversificationEnabled": c.diversification_enabled,
                "reRankingEnabled": c.re_ranking_enabled,
                "reRankingModel": c.re_ranking_model,
                "phraseQueryEnabled": c.phrase_query_enabled,
                "thinkingEnabled": c.thinking_enabled,
                "structuredOutput": c.structured_output,
                "debug": c.debug,
            }))
        })
        .await
}

/// `ConfigUpdateRequest`: tutti i campi obbligatori tranne `reRankingModel` e
/// `thinkingEnabled`, che hanno un default.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigUpdate {
    ollama_base_url: String,
    model_name: String,
    embedding_model_name: String,
    rag_max_docs: i32,
    similarity_trashold: f64,
    chat_context_window_size: i32,
    chat_context_max_tokens: i32,
    context_window: i32,
    chunk_size: i32,
    chunk_overlap: i32,
    title_boost: f32,
    query_expansion_enabled: bool,
    diversification_enabled: bool,
    re_ranking_enabled: bool,
    #[serde(default)]
    re_ranking_model: String,
    phrase_query_enabled: bool,
    structured_output: bool,
    debug: bool,
    #[serde(default = "enabled")]
    thinking_enabled: bool,
}

fn enabled() -> bool {
    true
}

impl From<ConfigUpdate> for SettingsUpdate {
    fn from(u: ConfigUpdate) -> Self {
        Self {
            ollama_base_url: u.ollama_base_url,
            model_name: u.model_name,
            embedding_model_name: u.embedding_model_name,
            rag_max_docs: u.rag_max_docs,
            similarity_trashold: u.similarity_trashold,
            chat_context_window_size: u.chat_context_window_size,
            chat_context_max_tokens: u.chat_context_max_tokens,
            context_window: u.context_window,
            chunk_size: u.chunk_size,
            chunk_overlap: u.chunk_overlap,
            title_boost: u.title_boost,
            query_expansion_enabled: u.query_expansion_enabled,
            diversification_enabled: u.diversification_enabled,
            re_ranking_enabled: u.re_ranking_enabled,
            re_ranking_model: u.re_ranking_model,
            phrase_query_enabled: u.phrase_query_enabled,
            structured_output: u.structured_output,
            debug: u.debug,
            thinking_enabled: u.thinking_enabled,
        }
    }
}

pub async fn update_config(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let update: ConfigUpdate = match decode(&body) {
        Ok(update) => update,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            s.config.apply_settings(update.into());
            s.llm.reconfigure(&s.config.ollama_base_url);
            match s.store_config() {
                Ok(()) => ok(&json!({ "ok": true })),
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

pub async fn models(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            let current = s.config.model_name.clone();
            let body: Vec<_> = s
                .llm
                .models()
                .into_iter()
                .map(|m| {
                    let is_current = m.name == current;
                    json!({ "name": m.name, "maxContext": m.max_context, "current": is_current })
                })
                .collect();
            ok(&body)
        })
        .await
}
