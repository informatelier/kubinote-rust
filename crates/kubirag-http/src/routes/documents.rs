//! **Notebook Governance**: le sorgenti del notebook (`CtrlDocuments`). L'upload salva il
//! file nella directory di lavoro (`./<targetDir>/<nome>`) e lo registra con un
//! `ADD_DOCUMENT` governato; il writer del manifest ne fa una risorsa gestita.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use axum::extract::{Multipart, Path as UrlPath, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use indexmap::IndexSet;
use kubi_kernel::governance::OperationStatus;
use kubirag_domain::notebook::{
    RagDocument, RagNotebookCommand, RagNotebookCommandType, ResourceOwnership,
};
use kubirag_domain::retrieval::{CHUNK_INDEX, PARENT_ID};
use kubirag_store_json::{canonical, load_document_from_file};
use serde_json::json;

use super::governance::serve_inside;
use super::synced;
use crate::respond::{
    self, error, expected_revision, ok, operation_context, render, with_revision,
};
use crate::state::AppState;

const ALLOWED_EXTENSIONS: [&str; 3] = ["md", "txt", "pdf"];

fn meta<'a>(document: &'a RagDocument, key: &str) -> Option<&'a str> {
    document.metadata.get(key).map(String::as_str)
}

pub async fn list(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            synced(s, |s| {
                let docs: Vec<_> = s
                    .notebook
                    .manifest()
                    .documents()
                    .iter()
                    .map(|d| {
                        let path = meta(d, "path");
                        json!({
                            "id": d.id,
                            "path": path.unwrap_or(""),
                            "filename": meta(d, "filename")
                                .or_else(|| path.map(|p| p.rsplit('/').next().unwrap_or(p)))
                                .unwrap_or(&d.id),
                            "enriched": d.metadata.contains_key("enrichedAt"),
                            "parentId": meta(d, PARENT_ID).unwrap_or(""),
                            "chunkIndex": meta(d, CHUNK_INDEX).unwrap_or(""),
                            "pageRange": meta(d, "page_range").unwrap_or(""),
                            "groupName": meta(d, "group_name").unwrap_or(""),
                        })
                    })
                    .collect();
                ok(&docs)
            })
        })
        .await
}

pub async fn detail(State(state): State<AppState>, UrlPath(id): UrlPath<String>) -> Response {
    state
        .run(move |s| {
            synced(s, |s| {
                let Some(d) = s
                    .notebook
                    .manifest()
                    .documents()
                    .iter()
                    .find(|d| d.id == id)
                else {
                    return respond::not_found();
                };
                ok(&json!({
                    "id": d.id,
                    "filename": meta(d, "filename").unwrap_or(""),
                    "path": meta(d, "path").unwrap_or(""),
                    "content": d.content,
                    "enrichedAt": meta(d, "enrichedAt").unwrap_or(""),
                    "enrichedByModel": meta(d, "enrichedByModel").unwrap_or(""),
                    "aiSummary": meta(d, "aiSummary").unwrap_or(""),
                    "aiEntities": meta(d, "aiEntities").unwrap_or(""),
                    "aiRelations": meta(d, "aiRelations").unwrap_or(""),
                }))
            })
        })
        .await
}

/// `GET /documents/{id}/asset?file=`: un file accanto alla sorgente (immagini del markdown).
pub async fn asset(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    state
        .run(move |s| {
            synced(s, |s| {
                let Some(d) = s
                    .notebook
                    .manifest()
                    .documents()
                    .iter()
                    .find(|d| d.id == id)
                else {
                    return respond::not_found();
                };
                let Some(path) = meta(d, "path").filter(|p| !p.trim().is_empty()) else {
                    return respond::not_found();
                };
                let Some(file) = query
                    .get("file")
                    .map(|f| f.trim())
                    .filter(|f| !f.is_empty())
                else {
                    return error(StatusCode::BAD_REQUEST, "file query parameter required");
                };
                let source = s.cwd.join(path);
                match source.parent() {
                    Some(dir) if dir.exists() => serve_inside(dir, file),
                    _ => respond::not_found(),
                }
            })
        })
        .await
}

/// `DELETE /documents/groups/{parentId}`: tutti i chunk di un PDF importato.
pub async fn remove_group(
    State(state): State<AppState>,
    UrlPath(parent): UrlPath<String>,
    headers: HeaderMap,
) -> Response {
    let expected = match expected_revision(&headers) {
        Ok(r) => r,
        Err(response) => return response,
    };
    let context = operation_context(&headers, expected);
    state
        .run(move |s| {
            synced(s, |s| {
                let ids: IndexSet<String> = s
                    .notebook
                    .manifest()
                    .documents()
                    .iter()
                    .filter(|d| meta(d, PARENT_ID) == Some(parent.as_str()))
                    .map(|d| d.id.clone())
                    .collect();
                if ids.is_empty() {
                    return respond::not_found();
                }
                render(
                    &s.notebook
                        .service()
                        .execute(&RagNotebookCommand::RemoveDocuments { ids }, &context),
                    StatusCode::OK,
                )
            })
        })
        .await
}

struct Upload {
    filename: String,
    content: Vec<u8>,
    target_dir: String,
}

async fn read_upload(mut multipart: Multipart) -> Result<Option<Upload>, String> {
    let (mut file, mut target_dir) = (None, String::new());
    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        match field.name() {
            Some("file") => {
                let name = field.file_name().unwrap_or_default().to_owned();
                let bytes = field.bytes().await.map_err(|e| e.to_string())?;
                file = Some((name, bytes.to_vec()));
            }
            Some("targetDir") => target_dir = field.text().await.map_err(|e| e.to_string())?,
            _ => {}
        }
    }
    Ok(file.map(|(filename, content)| Upload {
        filename,
        content,
        target_dir: target_dir.trim().to_owned(),
    }))
}

/// La directory scelta senza `../` né separatori iniziali, come `CtrlDocuments`.
fn safe_dir(raw: &str) -> String {
    raw.replace("../", "")
        .replace("..\\", "")
        .trim_start_matches(['/', '\\'])
        .to_owned()
}

/// `POST /documents/upload`: `md` e `txt` diventano un documento; un `pdf` è salvato e la
/// risposta `202` chiede la configurazione dei chunk, come in Kotlin.
pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    let expected = match expected_revision(&headers) {
        Ok(r) => r,
        Err(response) => return response,
    };
    let upload = match read_upload(multipart).await {
        Ok(Some(upload)) => upload,
        Ok(None) => return error(StatusCode::BAD_REQUEST, "No file received"),
        Err(e) => return error(StatusCode::BAD_REQUEST, &e),
    };
    let extension = upload
        .filename
        .rsplit_once('.')
        .map_or_else(String::new, |(_, ext)| ext.to_lowercase());
    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return error(
            StatusCode::BAD_REQUEST,
            &format!("Unsupported format: .{extension} (accepted: md, txt, pdf)"),
        );
    }
    let context = operation_context(&headers, expected);
    state
        .run(move |s| {
            let dir = safe_dir(&upload.target_dir);
            let shown = if dir.is_empty() {
                format!("./{}", upload.filename)
            } else {
                format!("./{dir}/{}", upload.filename)
            };
            let target_dir = if dir.is_empty() {
                s.cwd.clone()
            } else {
                s.cwd.join(&dir)
            };
            if fs::create_dir_all(&target_dir).is_err() {
                return respond::internal(&format!("Save error: cannot create {shown}"));
            }
            let target = target_dir.join(&upload.filename);
            let existed = target.exists();
            if existed && fs::read(&target).ok().as_deref() != Some(upload.content.as_slice()) {
                return error(
                    StatusCode::CONFLICT,
                    &format!("A different file already exists at {shown}; it was not overwritten"),
                );
            }
            let cwd = s.cwd.clone();
            synced(s, |s| {
                let indexed = s
                    .notebook
                    .manifest()
                    .documents()
                    .iter()
                    .find(|d| is_indexed_source(&cwd, meta(d, "path"), &target))
                    .cloned();
                let command = if indexed.is_some() {
                    RagNotebookCommandType::ReplaceDocument
                } else {
                    RagNotebookCommandType::AddDocument
                };
                let target_id = indexed
                    .as_ref()
                    .map_or(upload.filename.as_str(), |d| d.id.as_str());
                if let Some(stale) = s.notebook.service().preflight(command, target_id, &context) {
                    return render(&stale, StatusCode::OK);
                }
                if !existed {
                    if let Err(e) = fs::write(&target, &upload.content) {
                        return respond::internal(&format!("Save error: {e}"));
                    }
                }
                if extension == "pdf" {
                    return pdf_requires_config(s, &upload.filename, &shown, existed);
                }
                let loaded = match load_document_from_file(&absolute(&target)) {
                    Ok(document) => document,
                    Err(e) => return respond::internal(&e),
                };
                let result = match indexed {
                    None => s.notebook.service().execute(
                        &RagNotebookCommand::AddDocument {
                            document: loaded,
                            ownership: ResourceOwnership::External,
                        },
                        &context,
                    ),
                    Some(previous) => {
                        let document = RagDocument {
                            id: previous.id.clone(),
                            resource_ref: previous.resource_ref.clone().or(loaded.resource_ref),
                            ..loaded
                        };
                        s.notebook.service().execute(
                            &RagNotebookCommand::ReplaceDocument {
                                document,
                                ownership: ResourceOwnership::External,
                            },
                            &context,
                        )
                    }
                };
                if !existed
                    && matches!(
                        result.status(),
                        OperationStatus::Conflicted | OperationStatus::Rejected
                    )
                {
                    let _ = fs::remove_file(&target);
                }
                let mut response = render(&result, StatusCode::CREATED);
                let headers = response.headers_mut();
                headers.insert(
                    "X-QB-Source-Reused",
                    HeaderValue::from_static(bool_text(existed)),
                );
                headers.insert(
                    "X-QB-Source-Reindexed",
                    HeaderValue::from_static(bool_text(
                        command == RagNotebookCommandType::ReplaceDocument,
                    )),
                );
                response
            })
        })
        .await
}

fn bool_text(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// `isIndexedSource`: il percorso del documento, risolto dalla directory di lavoro, è il file
/// caricato. Dopo il primo commit il percorso è quello della risorsa gestita, quindi un nuovo
/// upload dello stesso file non lo ritrova (stesso comportamento di Kotlin).
fn is_indexed_source(cwd: &Path, path: Option<&str>, target: &Path) -> bool {
    match path.filter(|p| !p.trim().is_empty()) {
        Some(path) => canonical(&cwd.join(path)) == canonical(target),
        None => false,
    }
}

/// La risposta `202` di un PDF: il frontend chiede le pagine per chunk e chiama
/// `/documents/pdf-chunked` (rimandato a F5).
fn pdf_requires_config(
    s: &mut crate::state::Server,
    filename: &str,
    shown: &str,
    existed: bool,
) -> Response {
    let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    let manifest = s.notebook.manifest();
    let existing = manifest
        .documents()
        .iter()
        .filter(|d| {
            meta(d, PARENT_ID).is_some()
                && (meta(d, "filename").is_some_and(|f| f.starts_with(filename))
                    || meta(d, "group_name") == Some(stem))
        })
        .count();
    let body = json!({
        "requiresConfig": true,
        "existingChunks": existing,
        "defaultPagesPerChunk": s.config.pdf_default_pages_per_chunk,
        "suggestedGroupName": stem,
        "savedPath": shown,
        "reusedExisting": existed,
    });
    with_revision(
        respond::json(StatusCode::ACCEPTED, &body),
        manifest.revision(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_directory_cannot_climb_or_be_absolute() {
        assert_eq!(safe_dir("../../etc"), "etc");
        assert_eq!(safe_dir("/docs"), "docs");
        assert_eq!(safe_dir("..\\x"), "x");
    }
}
