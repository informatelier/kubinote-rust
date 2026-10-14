//! **Notebook Governance**: stato e diagnosi del manifest (`CtrlRagGovernance`), proprietà
//! editoriali (`CtrlNotebook`) e documenti pinned (`CtrlPinned`).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use indexmap::IndexSet;
use kubi_kernel::governance::{ModelIssue, ModelIssueSeverity};
use kubirag_app::notebook::RagNotebookApplicationService;
use kubirag_domain::notebook::{
    NotebookProperties, RagNotebookCommand, RagNotebookCommandType, RagNotebookManifest,
    RAG_NOTEBOOK_MANIFEST_VERSION,
};
use serde::Deserialize;
use serde_json::json;

use super::synced;
use crate::respond::{
    self, error, expected_revision, ok, operation_context, render, with_revision,
};
use crate::state::{AppState, Server};

/// `RagNotebookGovernanceGateway.issues`: problemi del servizio più quelli dell'ultimo
/// evento del journal, senza doppioni per codice e percorso.
fn visible_issues(service: &RagNotebookApplicationService) -> Vec<ModelIssue> {
    let last = service
        .audit_events()
        .ok()
        .and_then(|events| events.last().map(|e| e.issues.clone()))
        .unwrap_or_default();
    let mut seen = IndexSet::new();
    service
        .issues()
        .iter()
        .chain(last.iter())
        .filter(|issue| seen.insert((issue.code().to_owned(), issue.path().to_owned())))
        .cloned()
        .collect()
}

pub async fn status(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| {
                let issues = visible_issues(s.notebook.service());
                let m = s.notebook.manifest();
                let body = json!({
                    "id": m.id().as_str(),
                    "name": m.name(),
                    "version": RAG_NOTEBOOK_MANIFEST_VERSION,
                    "revision": m.revision(),
                    "documents": m.documents().len(),
                    "pins": m.pinned_document_ids().len(),
                    "resources": m.resources().len(),
                    "issueCount": issues.len(),
                });
                with_revision(ok(&body), m.revision())
            })
        })
        .await
}

pub async fn issues(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| {
                let issues = visible_issues(s.notebook.service());
                with_revision(ok(&issues), s.notebook.manifest().revision())
            })
        })
        .await
}

pub async fn audit(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| match s.notebook.service().audit_events() {
                Ok(events) => with_revision(ok(&events), s.notebook.manifest().revision()),
                Err(e) => respond::internal(&e),
            })
        })
        .await
}

pub async fn health(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| {
                let issues = visible_issues(s.notebook.service());
                let degraded = issues
                    .iter()
                    .any(|i| i.severity() == ModelIssueSeverity::Error);
                let revision = s.notebook.manifest().revision();
                let body = json!({
                    "status": if degraded { "degraded" } else { "healthy" },
                    "revision": revision,
                    "issues": issues,
                });
                with_revision(ok(&body), revision)
            })
        })
        .await
}

pub async fn rebuild(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let expected = match expected_revision(&headers) {
        Ok(r) => r,
        Err(response) => return response,
    };
    let context = operation_context(&headers, expected);
    state
        .run(move |s| {
            synced(s, |s| {
                render(
                    &s.notebook.service().rebuild_projections(&context),
                    StatusCode::OK,
                )
            })
        })
        .await
}

pub async fn undo(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let expected = match expected_revision(&headers) {
        Ok(r) => r,
        Err(response) => return response,
    };
    let context = operation_context(&headers, expected);
    state
        .run(move |s| {
            synced(s, |s| {
                render(
                    &s.notebook.service().undo_unavailable(&context),
                    StatusCode::OK,
                )
            })
        })
        .await
}

// ── Pinned ──────────────────────────────────────────────────────────────────

pub async fn pinned(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| {
                let m = s.notebook.manifest();
                let docs: Vec<_> = m
                    .documents()
                    .iter()
                    .filter(|d| m.pinned_document_ids().contains(&d.id))
                    .map(|d| {
                        json!({
                            "id": d.id,
                            "filename": d.metadata.get("filename").unwrap_or(&d.id),
                            "path": d.metadata.get("path").map_or("", String::as_str),
                        })
                    })
                    .collect();
                with_revision(ok(&docs), m.revision())
            })
        })
        .await
}

async fn governed(
    state: AppState,
    headers: HeaderMap,
    command: RagNotebookCommand,
    success: StatusCode,
) -> Response {
    let expected = match expected_revision(&headers) {
        Ok(r) => r,
        Err(response) => return response,
    };
    let context = operation_context(&headers, expected);
    state
        .run(move |s| {
            synced(s, |s| {
                render(&s.notebook.service().execute(&command, &context), success)
            })
        })
        .await
}

pub async fn pin(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    governed(
        state,
        headers,
        RagNotebookCommand::Pin { id },
        StatusCode::OK,
    )
    .await
}

pub async fn unpin(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    governed(
        state,
        headers,
        RagNotebookCommand::Unpin { id },
        StatusCode::OK,
    )
    .await
}

pub async fn remove_document(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    governed(
        state,
        headers,
        RagNotebookCommand::RemoveDocument { id },
        StatusCode::OK,
    )
    .await
}

// ── Proprietà del notebook ──────────────────────────────────────────────────

const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];

/// `CtrlNotebook.resolveStore`: `?store=` o il notebook attivo, mai fuori da `notes/`.
#[allow(clippy::result_large_err)]
fn resolve_store(s: &Server, query: &HashMap<String, String>) -> Result<String, Response> {
    let requested = query.get("store").map_or("", |v| v.trim());
    let store = if requested.is_empty() {
        s.current_store
            .clone()
            .unwrap_or_else(|| kubirag_workspace::DEFAULT_STORE.to_owned())
    } else {
        requested.to_owned()
    };
    if store.contains('/') || store.contains('\\') || store.contains("..") {
        return Err(error(StatusCode::BAD_REQUEST, "Invalid store name"));
    }
    Ok(store)
}

/// `withNotebookService(name)`: il servizio attivo se è il notebook caricato, altrimenti
/// uno senza proiezioni (creato vuoto se manca, come `loadService(active = false)`).
fn with_notebook_service(
    s: &mut Server,
    store: &str,
    action: impl FnOnce(&mut RagNotebookApplicationService) -> Response,
) -> Response {
    if store == s.active_name() {
        return synced(s, |s| action(s.notebook.service()));
    }
    match s.workspace.service(store, true) {
        Ok(mut service) => action(&mut service),
        Err(e) => respond::internal(&e),
    }
}

fn properties_json(store: &str, manifest: &RagNotebookManifest) -> serde_json::Value {
    let p = manifest.properties();
    json!({
        "storeName": store,
        "defaultTitle": store,
        "author": p.author,
        "title": p.title,
        "subtitle": p.subtitle,
        "description": p.description,
        "coverImage": p.cover_image,
        "backgroundImage": p.background_image,
        "accentColor": p.accent_color,
    })
}

pub async fn notebook(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    state
        .run(move |s| {
            let store = match resolve_store(s, &query) {
                Ok(store) => store,
                Err(response) => return response,
            };
            with_notebook_service(s, &store, |service| {
                let manifest = service.snapshot();
                with_revision(ok(&properties_json(&store, manifest)), manifest.revision())
            })
        })
        .await
}

/// `NotebookProperties` di Jackson: ogni campo assente è vuoto.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct PropertiesRequest {
    author: String,
    title: String,
    subtitle: String,
    description: String,
    cover_image: String,
    background_image: String,
    accent_color: String,
}

pub async fn save_notebook(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    state
        .run(move |s| {
            let store = match resolve_store(s, &query) {
                Ok(store) => store,
                Err(response) => return response,
            };
            let expected = match expected_revision(&headers) {
                Ok(r) => r,
                Err(response) => return response,
            };
            let Ok(request) = serde_json::from_slice::<PropertiesRequest>(&body) else {
                return error(StatusCode::BAD_REQUEST, "Invalid notebook properties");
            };
            let assets = s
                .roots()
                .authority
                .join("notes")
                .join(&store)
                .join("assets");
            let context = operation_context(&headers, expected);
            let cwd = s.cwd.clone();
            with_notebook_service(s, &store, |service| {
                if let Some(stale) =
                    service.preflight(RagNotebookCommandType::UpdateProperties, &store, &context)
                {
                    return render(&stale, StatusCode::OK);
                }
                let properties = NotebookProperties {
                    author: request.author,
                    title: request.title,
                    subtitle: request.subtitle,
                    description: request.description,
                    cover_image: import_image(&cwd, &assets, &request.cover_image),
                    background_image: import_image(&cwd, &assets, &request.background_image),
                    accent_color: request.accent_color,
                };
                render(
                    &service.execute(
                        &RagNotebookCommand::UpdateProperties { properties },
                        &context,
                    ),
                    StatusCode::OK,
                )
            })
        })
        .await
}

/// `CtrlNotebook.importImageRef`: URL e riferimenti `assets/` restano; un file immagine
/// locale è copiato negli asset del notebook; tutto il resto diventa vuoto.
fn import_image(cwd: &Path, assets: &Path, raw: &str) -> String {
    let reference = raw.trim();
    let lower = reference.to_lowercase();
    if reference.is_empty() {
        return String::new();
    }
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || reference.starts_with("assets/")
    {
        return reference.to_owned();
    }
    let source = cwd.join(reference);
    let extension = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !source.is_file() || !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return String::new();
    }
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if fs::create_dir_all(assets).is_err() || fs::copy(&source, assets.join(&safe)).is_err() {
        return String::new();
    }
    format!("assets/{safe}")
}

/// `GET /notebook/asset/{file}`: un'immagine importata, solo dentro `assets/`.
pub async fn notebook_asset(
    State(state): State<AppState>,
    UrlPath(file): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    state
        .run(move |s| {
            let store = match resolve_store(s, &query) {
                Ok(store) => store,
                Err(response) => return response,
            };
            let base = s.roots().authority.join("notes").join(store).join("assets");
            serve_inside(&base, &file)
        })
        .await
}

/// Un file sotto `base` (canonico, senza uscirne), con il tipo dedotto dal nome.
pub fn serve_inside(base: &Path, file: &str) -> Response {
    let (Ok(base), Ok(asset)) = (fs::canonicalize(base), fs::canonicalize(base.join(file))) else {
        return respond::not_found();
    };
    if !asset.starts_with(&base) || !asset.is_file() {
        return respond::not_found();
    }
    match fs::read(&asset) {
        Ok(bytes) => respond::bytes(content_type(&asset), bytes),
        Err(e) => respond::internal(&e.to_string()),
    }
}

/// `URLConnection.guessContentTypeFromName` per i tipi che il frontend usa.
fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "html" | "htm" => "text/html",
        "txt" | "md" => "text/plain",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
