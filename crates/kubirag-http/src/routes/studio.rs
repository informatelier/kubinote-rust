//! **Notes & Studio**: la libreria dei modelli dell'editor markboard (`CtrlTemplates`).

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;

use crate::respond::{self, decode, error, ok};
use crate::state::AppState;

pub async fn templates(State(state): State<AppState>) -> Response {
    state
        .run(|s| match s.templates.names() {
            Ok(names) => {
                let body: Vec<_> = names
                    .into_iter()
                    .map(|name| json!({ "name": name }))
                    .collect();
                ok(&body)
            }
            Err(e) => respond::internal(&e),
        })
        .await
}

pub async fn template(State(state): State<AppState>, UrlPath(name): UrlPath<String>) -> Response {
    state
        .run(move |s| match s.templates.content(&name) {
            Ok(Some(content)) => ok(&json!({ "name": name, "content": content })),
            Ok(None) => respond::not_found(),
            Err(e) => respond::internal(&e),
        })
        .await
}

#[derive(Deserialize)]
struct CreateTemplate {
    name: String,
    content: String,
}

pub async fn create_template(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let request: CreateTemplate = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    if request.name.trim().is_empty() {
        return error(StatusCode::BAD_REQUEST, "name is required");
    }
    state
        .run(
            move |s| match s.templates.save(&request.name, &request.content) {
                Ok(name) => respond::json(StatusCode::CREATED, &json!({ "name": name })),
                Err(e) => respond::internal(&e),
            },
        )
        .await
}
