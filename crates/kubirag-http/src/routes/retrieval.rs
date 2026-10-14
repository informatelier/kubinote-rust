//! **Retrieval & Q&A**: ricerca senza generazione (`GET /search`) e proiezione degli
//! embedding (`CtrlEmbeddingProjection`).

use std::collections::HashMap;

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use kubirag_app::retrieval::ProjectionStore;
use kubirag_domain::retrieval::{EmbeddingProjectionStatus, EmbeddingSettings};
use serde::Deserialize;
use serde_json::{json, Value};

use super::synced;
use crate::respond::{self, error, ok};
use crate::state::{AppState, Server};

/// Risultati della barra di ricerca (`searchDocuments`, `maxResults = 20`).
const SEARCH_RESULTS: usize = 20;

pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let q = query.get("q").map_or("", |q| q.trim()).to_owned();
    if q.is_empty() {
        return ok(&json!([]));
    }
    state
        .run(move |s| {
            synced(s, |s| {
                let config = s.effective_config();
                let hits = match s.notebook.runtime() {
                    Ok(mut runtime) => runtime.search_documents(&config, &q, SEARCH_RESULTS),
                    Err(e) => return respond::internal(&e),
                };
                let body: Vec<_> = hits
                    .into_iter()
                    .map(|h| {
                        json!({
                            "rank": h.rank,
                            "documentId": h.document_id,
                            "score": respond::float(h.score),
                            "filename": h.filename,
                            "path": h.path,
                        })
                    })
                    .collect();
                ok(&body)
            })
        })
        .await
}

/// `EmbeddingProjectionStatus` come lo serializza Jackson.
pub fn status_json(status: &EmbeddingProjectionStatus) -> Value {
    json!({
        "state": status.state,
        "compatible": status.compatible,
        "configuredModel": status.configured_model,
        "descriptor": status.descriptor,
        "reasons": status.reasons,
    })
}

fn active_status(s: &Server) -> Result<EmbeddingProjectionStatus, String> {
    let config = s.effective_config();
    Ok(s.notebook.runtime()?.embedding_status(&config))
}

pub async fn status(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| match active_status(s) {
                Ok(status) => ok(&status_json(&status)),
                Err(e) => respond::internal(&e),
            })
        })
        .await
}

/// `embeddingProjectionStatus(name, ragConfig)`: qualunque notebook, con la configurazione
/// senza il modello del profilo.
pub async fn status_for(State(state): State<AppState>, UrlPath(name): UrlPath<String>) -> Response {
    state
        .run(move |s| {
            let descriptor = s.workspace.projections().read_profile(&name);
            let status = EmbeddingProjectionStatus::evaluate(
                descriptor,
                EmbeddingSettings {
                    model: &s.config.embedding_model_name,
                    chunk_size: s.config.chunk_size,
                    chunk_overlap: s.config.chunk_overlap,
                },
            );
            ok(&status_json(&status))
        })
        .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RebuildRequest {
    force: bool,
    use_notebook_profile: bool,
}

impl Default for RebuildRequest {
    fn default() -> Self {
        Self {
            force: true,
            use_notebook_profile: false,
        }
    }
}

/// `POST /embedding-projection/rebuild`: ricalcola gli embedding con il modello della
/// configurazione o, con `useNotebookProfile`, con quello del profilo del notebook.
pub async fn rebuild(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let request: RebuildRequest = match respond::decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            synced(s, |s| match rebuild_embeddings(s, &request) {
                Ok(body) => ok(&body),
                Err(e) => error(StatusCode::UNPROCESSABLE_ENTITY, &e),
            })
        })
        .await
}

fn rebuild_embeddings(s: &mut Server, request: &RebuildRequest) -> Result<Value, String> {
    let store = s.active_name();
    let model = if request.use_notebook_profile {
        let descriptor = s
            .workspace
            .projections()
            .read_profile(&store)
            .ok_or("Il notebook non contiene un profilo embedding")?;
        s.embedding_overrides
            .insert(store, descriptor.model.clone());
        descriptor.model
    } else {
        s.embedding_overrides.remove(&store);
        s.config.embedding_model_name.clone()
    };
    let config = s.effective_config();
    let (completed, total) = s
        .notebook
        .runtime()?
        .backfill_embeddings(&config, request.force)?;
    Ok(json!({
        "model": model,
        "completed": completed,
        "total": total,
        "projection": status_json(&active_status(s)?),
    }))
}
