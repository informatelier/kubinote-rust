//! **Notebook Governance**: il catalogo dei notebook (`CtrlStores`): elenco, creazione,
//! cambio del notebook attivo, salvataggio delle proiezioni, cancellazione.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use serde::Deserialize;
use serde_json::json;

use crate::respond::{self, decode, error, ok, render, system_context};
use crate::state::{AppState, Server};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreRequest {
    store_name: String,
}

pub async fn list(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            let stores: Vec<_> = s
                .workspace
                .store_names()
                .into_iter()
                .map(|name| {
                    let active = s.current_store.as_deref() == Some(name.as_str());
                    let has_embeddings = s.workspace.has_embeddings(&name);
                    json!({ "storeName": name, "active": active, "hasEmbeddings": has_embeddings })
                })
                .collect();
            ok(&stores)
        })
        .await
}

/// La ricostruzione che segue creazione, cambio e salvataggio: rende il notebook attivo
/// allineato al suo manifest.
fn rebuild_active(s: &mut Server, success: StatusCode) -> Response {
    let revision = s.notebook.manifest().revision();
    render(
        &s.notebook
            .service()
            .rebuild_projections(&system_context(revision)),
        success,
    )
}

pub async fn create(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let request: StoreRequest = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            let name = request.store_name.trim().to_owned();
            if name.is_empty() {
                return error(StatusCode::BAD_REQUEST, "storeName is required");
            }
            if s.workspace.contains(&name) {
                return error(
                    StatusCode::CONFLICT,
                    &format!("Store '{name}' already exists"),
                );
            }
            if let Err(detail) = s.workspace.create_empty(&name) {
                return error(
                    StatusCode::BAD_REQUEST,
                    &format!("Could not create Notebook '{name}': {detail}"),
                );
            }
            if let Err(detail) = s.activate(&name) {
                return respond::internal(&format!(
                    "Notebook '{name}' was created, but its runtime initialization failed: {detail}"
                ));
            }
            rebuild_active(s, StatusCode::CREATED)
        })
        .await
}

pub async fn switch(State(state): State<AppState>, body: axum::body::Bytes) -> Response {
    let request: StoreRequest = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            let name = request.store_name.trim().to_owned();
            if !s.workspace.contains(&name) {
                return respond::not_found();
            }
            if let Err(detail) = s.activate(&name) {
                return respond::internal(&detail);
            }
            rebuild_active(s, StatusCode::OK)
        })
        .await
}

pub async fn save(State(state): State<AppState>, UrlPath(name): UrlPath<String>) -> Response {
    state
        .run(move |s| {
            if !s.workspace.contains(&name) && s.current_store.as_deref() != Some(name.as_str()) {
                return respond::not_found();
            }
            if name != s.active_name() {
                return error(
                    StatusCode::CONFLICT,
                    "Switch to the Notebook before saving it",
                );
            }
            match s.sync() {
                Ok(()) => rebuild_active(s, StatusCode::OK),
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

/// `CtrlNotebookArchive.isAllowedClient`: solo da loopback e, se c'è `Origin`, dalla stessa
/// origine del server.
fn local_same_origin(peer: SocketAddr, headers: &HeaderMap) -> bool {
    if !peer.ip().is_loopback() {
        return false;
    }
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|o| o.to_str().ok())
        .filter(|o| !o.trim().is_empty())
    else {
        return true;
    };
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();
    origin.eq_ignore_ascii_case(&format!("http://{host}"))
}

pub async fn delete(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    UrlPath(name): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    if !local_same_origin(peer, &headers) {
        return error(
            StatusCode::FORBIDDEN,
            "Notebook filesystem operations are available only to the local same-origin dashboard",
        );
    }
    state
        .run(move |s| {
            if !s.workspace.contains(&name) {
                return respond::not_found();
            }
            if query.get("confirm") != Some(&name) {
                return error(
                    StatusCode::BAD_REQUEST,
                    "confirm query parameter must equal the notebook name",
                );
            }
            s.embedding_overrides.remove(&name);
            if let Err(e) = s.workspace.delete_store(&name) {
                return respond::internal(&e);
            }
            if name == s.active_name() {
                // `ragSystem.delete` riporta il runtime sul notebook predefinito.
                if let Err(e) = s.activate(kubirag_workspace::DEFAULT_STORE) {
                    return respond::internal(&e);
                }
            }
            if s.current_store.as_deref() == Some(name.as_str()) {
                s.current_store = None;
            }
            ok(&json!({ "storeName": name }))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(origin: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost:7070"));
        if let Some(origin) = origin {
            headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }
        headers
    }

    #[test]
    fn only_local_same_origin_clients_may_delete() {
        let local: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let remote: SocketAddr = "192.168.1.10:5000".parse().unwrap();
        assert!(local_same_origin(local, &headers(None)));
        assert!(local_same_origin(
            local,
            &headers(Some("http://LOCALHOST:7070"))
        ));
        assert!(!local_same_origin(
            local,
            &headers(Some("http://evil.example"))
        ));
        assert!(!local_same_origin(remote, &headers(None)));
    }
}
