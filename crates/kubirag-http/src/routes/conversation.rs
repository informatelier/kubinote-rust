//! **Conversation**: la chat in streaming (`CtrlQuery.postQuery`), le sessioni per utente,
//! gli assistenti (`CtrlRoles`) e la stima della finestra di contesto (`CtrlContext`).

use std::collections::HashMap;
use std::convert::Infallible;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{Body, Bytes};
use axum::extract::{Path as UrlPath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use kubirag_app::retrieval::{LlmGateway, PreparedAnswer};
use kubirag_domain::conversation::{
    parse_at_mention, sanitize_user_id, ContextUsage, RoleEntry, Roles, DEFAULT_SESSION,
};
use kubirag_domain::retrieval::ChatTurn;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use super::synced;
use crate::respond::{self, decode, error, ok};
use crate::state::{AppState, Server};

/// `Content-Type` della risposta in streaming: `application/json` con la codifica di Jetty.
const STREAM_CONTENT_TYPE: &str = "application/json;charset=utf-8";

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `User-ID` normalizzato, `anonymous` se assente.
fn user_id(headers: &HeaderMap) -> String {
    let raw = headers
        .get("User-ID")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("anonymous");
    sanitize_user_id(raw)
}

fn session_or_default(id: Option<&str>) -> String {
    id.filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_SESSION)
        .to_owned()
}

// ── Chat ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryRequest {
    prompt: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default = "enabled")]
    retrieval: bool,
}

fn enabled() -> bool {
    true
}

/// Un passo della risposta: testo emesso così com'è (registrato o no nella sessione) o una
/// risposta dell'LLM preparata sotto il lock del notebook.
enum Step {
    Text { chunk: String, record: bool },
    Answer(PreparedAnswer),
}

/// `CtrlQuery.postQuery` senza generazione: menzioni `@ruolo` e `@party`, contesto scelto
/// dal runtime, una risposta preparata per ogni assistente.
fn prepare(
    s: &mut Server,
    question: &str,
    mention: Option<&str>,
    retrieval: bool,
    history: &[ChatTurn],
) -> Result<Vec<Step>, String> {
    s.sync()?;
    let config = s.effective_config();
    let mut runtime = s.notebook.runtime()?;
    let mut answer = |system: Option<&str>| {
        if retrieval {
            runtime.prepare_answer(&config, question, history, system)
        } else {
            runtime.prepare_pinned_answer(&config, question, history, system)
        }
    };
    let Some(mention) = mention else {
        return Ok(answer(None).into_iter().map(Step::Answer).collect());
    };
    let roles = s.roles.load().unwrap_or_default();
    let mut steps = Vec::new();
    if mention.eq_ignore_ascii_case("party") {
        if roles.is_empty() {
            steps.push(Step::Text {
                chunk: "Nessun ruolo definito. Usa /role-add per crearne.".into(),
                record: false,
            });
        }
        for role in roles.values() {
            steps.push(Step::Text {
                chunk: format!("\n\n### @{}\n\n", role.role_name),
                record: true,
            });
            steps.extend(answer(Some(&role.role_prompt)).map(Step::Answer));
        }
        return Ok(steps);
    }
    match roles
        .values()
        .find(|r| r.role_name.eq_ignore_ascii_case(mention))
    {
        None => steps.push(Step::Text {
            chunk: format!(
                "Ruolo '@{mention}' non trovato. Usa la sezione Assistants per vedere i ruoli disponibili."
            ),
            record: false,
        }),
        Some(role) => {
            steps.push(Step::Text {
                chunk: format!("[@{}] ", role.role_name),
                record: true,
            });
            steps.extend(answer(Some(&role.role_prompt)).map(Step::Answer));
        }
    }
    Ok(steps)
}

fn frame(chunk: &str) -> Bytes {
    let body = json!({ "response": chunk });
    Bytes::from(format!("data: {body}\n\n"))
}

/// `POST /chat/query`: frame `data: {"response": …}` per ogni frammento, la domanda e la
/// risposta completa salvate nella sessione dell'utente.
pub async fn query(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let request: QueryRequest = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let user = user_id(&headers);
    let session = session_or_default(request.session_id.as_deref());
    let (sender, receiver) = mpsc::channel::<Bytes>(64);
    let state_for_task = state.clone();
    tokio::task::spawn_blocking(move || {
        stream_answer(&state_for_task, &user, &session, &request, &sender);
    });
    let stream = ReceiverStream::new(receiver).map(Ok::<_, Infallible>);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(STREAM_CONTENT_TYPE),
    );
    response
}

fn stream_answer(
    state: &AppState,
    user: &str,
    session: &str,
    request: &QueryRequest,
    sender: &mpsc::Sender<Bytes>,
) {
    let send = |chunk: &str| {
        let _ = sender.blocking_send(frame(chunk));
    };
    if request.prompt.trim() == "/index" {
        send("/index is not available in the Rust server yet.");
        return;
    }
    let (mention, actual) = parse_at_mention(&request.prompt);
    let question = if mention.is_some() {
        actual.to_owned()
    } else {
        request.prompt.clone()
    };
    let window =
        state.blocking(|s| usize::try_from(s.config.chat_context_window_size).unwrap_or(0));
    let Ok(history) = state.sessions().with_user(user, false, |sessions| {
        let messages = sessions.messages(session);
        let history: Vec<ChatTurn> = messages[messages.len().saturating_sub(window)..]
            .iter()
            .map(|m| ChatTurn {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();
        sessions.add_message(session, "user", &question, now_millis());
        history
    }) else {
        return;
    };
    let steps = state.blocking(|s| {
        let llm: std::sync::Arc<dyn LlmGateway> = s.llm.clone();
        prepare(s, &question, mention, request.retrieval, &history).map(|steps| (steps, llm))
    });
    let Ok((steps, llm)) = steps else {
        return;
    };
    let mut full = String::new();
    for step in steps {
        match step {
            Step::Text { chunk, record } => {
                if record {
                    full.push_str(&chunk);
                }
                send(&chunk);
            }
            Step::Answer(prepared) => {
                let outcome = prepared.run(llm.as_ref(), &mut |chunk| {
                    full.push_str(chunk);
                    send(chunk);
                });
                if outcome.is_err() {
                    // Kotlin interrompe la risposta senza salvarla nella sessione.
                    return;
                }
            }
        }
    }
    let _ = state.sessions().with_user(user, true, |sessions| {
        sessions.add_message(session, "assistant", &full, now_millis());
    });
}

// ── Sessioni ────────────────────────────────────────────────────────────────

pub async fn sessions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = user_id(&headers);
    state
        .run_sessions(
            move |directory| match directory.with_user(&user, false, |s| s.metadata()) {
                Ok(rows) => {
                    let body: Vec<_> = rows
                        .into_iter()
                        .map(|m| {
                            json!({
                                "id": m.id,
                                "title": m.title,
                                "messageCount": m.message_count,
                                "lastMessage": m.last_message,
                                "updatedAt": m.updated_at,
                            })
                        })
                        .collect();
                    ok(&body)
                }
                Err(e) => respond::internal(&e),
            },
        )
        .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSession {
    session_id: String,
}

pub async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request: CreateSession = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let user = user_id(&headers);
    let limit = if user == "anonymous" { 10 } else { 20 };
    state
        .run_sessions(move |directory| {
            let id = request.session_id;
            let outcome =
                directory.with_user(&user, false, |s| (s.create(&id, limit), s.contains(&id)));
            match outcome {
                Ok((true, _)) => match directory.with_user(&user, true, |_| ()) {
                    Ok(()) => {
                        respond::json(StatusCode::CREATED, &json!({ "id": id, "messageCount": 0 }))
                    }
                    Err(e) => respond::internal(&e),
                },
                Ok((false, true)) => error(
                    StatusCode::CONFLICT,
                    &format!("Session '{id}' already exists"),
                ),
                Ok((false, false)) => {
                    let mut response = error(
                        StatusCode::TOO_MANY_REQUESTS,
                        &format!("Session limit reached ({limit})"),
                    );
                    response
                        .headers_mut()
                        .insert("X-Session-Limit", HeaderValue::from(limit));
                    response
                }
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

pub async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let user = user_id(&headers);
    state
        .run_sessions(move |directory| {
            let found = directory.with_user(&user, false, |s| {
                let messages = s.messages(&id);
                if messages.is_empty() && !s.contains(&id) {
                    return None;
                }
                let rows: Vec<_> = messages
                    .iter()
                    .map(|m| {
                        json!({ "role": m.role, "content": m.content, "timestamp": m.timestamp })
                    })
                    .collect();
                Some(json!({ "id": id, "messages": rows }))
            });
            match found {
                Ok(Some(body)) => ok(&body),
                Ok(None) => respond::not_found(),
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

#[derive(Deserialize)]
struct RenameSession {
    title: String,
}

pub async fn rename_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    body: Bytes,
) -> Response {
    let request: RenameSession = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let user = user_id(&headers);
    state
        .run_sessions(move |directory| {
            persist_if(
                directory,
                &user,
                |s| s.rename(&id, &request.title),
                || ok(&json!({ "id": id, "title": request.title })),
            )
        })
        .await
}

pub async fn delete_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
) -> Response {
    let user = user_id(&headers);
    state
        .run_sessions(move |directory| {
            persist_if(
                directory,
                &user,
                |s| s.delete(&id),
                || ok(&json!({ "id": id })),
            )
        })
        .await
}

/// Esegue un'azione che può fallire con 404; se riesce l'archivio è salvato.
fn persist_if(
    directory: &kubirag_app::conversation::ChatSessionDirectory,
    user: &str,
    action: impl FnOnce(&mut kubirag_domain::conversation::ChatSessions) -> bool,
    success: impl FnOnce() -> Response,
) -> Response {
    match directory.with_user(user, false, action) {
        Ok(true) => match directory.with_user(user, true, |_| ()) {
            Ok(()) => success(),
            Err(e) => respond::internal(&e),
        },
        Ok(false) => respond::not_found(),
        Err(e) => respond::internal(&e),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchSession {
    new_session_id: String,
}

pub async fn branch_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    UrlPath(source): UrlPath<String>,
    body: Bytes,
) -> Response {
    let request: BranchSession = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let user = user_id(&headers);
    let id = request.new_session_id.trim().to_owned();
    if id.is_empty() {
        return error(StatusCode::BAD_REQUEST, "newSessionId is required");
    }
    state
        .run_sessions(move |directory| {
            match directory.with_user(&user, false, |s| s.branch(&source, &id)) {
                Ok(Ok(_)) => match directory.with_user(&user, true, |_| ()) {
                    Ok(()) => respond::json(
                        StatusCode::CREATED,
                        &json!({ "id": id, "sourceId": source }),
                    ),
                    Err(e) => respond::internal(&e),
                },
                Ok(Err(message)) => error(StatusCode::CONFLICT, &message),
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

// ── Finestra di contesto ────────────────────────────────────────────────────

pub async fn context_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let user = user_id(&headers);
    let session = session_or_default(query.get("sessionId").map(String::as_str));
    let history = state
        .run_sessions(move |directory| {
            directory.with_user(&user, false, |s| s.messages(&session).to_vec())
        })
        .await;
    let history = match history {
        Ok(history) => history,
        Err(e) => return respond::internal(&e),
    };
    state
        .run(move |s| {
            synced(s, |s| {
                let pinned = match s.notebook.runtime() {
                    Ok(runtime) => runtime.pinned_documents(),
                    Err(e) => return respond::internal(&e),
                };
                let u = ContextUsage::estimate(&s.config, &pinned, &history);
                ok(&json!({
                    "contextWindow": u.context_window,
                    "chatContextMaxTokens": u.max_budget_tokens,
                    "occupiedTokens": u.occupied_tokens,
                    "systemTokens": u.system_tokens,
                    "pinnedTokens": u.pinned_tokens,
                    "historyTokens": u.history_tokens,
                    "ragAllowanceTokens": u.rag_allowance_tokens,
                    "percentOfWindow": u.percent_of_window,
                    "percentOfBudget": u.percent_of_budget,
                }))
            })
        })
        .await
}

// ── Assistenti ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleRequest {
    role_name: String,
    role_prompt: String,
    #[serde(default)]
    file_patterns: Vec<String>,
    #[serde(default)]
    voice_name: Option<String>,
}

fn load_roles(s: &Server) -> Result<Roles, Response> {
    s.roles.load().map_err(|e| respond::internal(&e))
}

pub async fn roles(State(state): State<AppState>) -> Response {
    state
        .run(|s| {
            let roles = match load_roles(s) {
                Ok(roles) => roles,
                Err(response) => return response,
            };
            let body: Vec<_> = roles
                .values()
                .map(|r| {
                    json!({
                        "roleName": r.role_name,
                        "rolePrompt": r.role_prompt,
                        "filePatterns": r.file_patterns,
                        "voiceName": r.voice_name.as_deref().unwrap_or(""),
                        "active": s.config.current_role.as_deref() == Some(r.role_name.as_str()),
                    })
                })
                .collect();
            ok(&body)
        })
        .await
}

pub async fn create_role(State(state): State<AppState>, body: Bytes) -> Response {
    let request: RoleRequest = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            if request.role_name.trim().is_empty() {
                return error(StatusCode::BAD_REQUEST, "roleName is required");
            }
            let mut roles = match load_roles(s) {
                Ok(roles) => roles,
                Err(response) => return response,
            };
            let name = request.role_name.clone();
            if roles.contains_key(&name) {
                return error(
                    StatusCode::CONFLICT,
                    &format!("Role '{name}' already exists"),
                );
            }
            roles.insert(
                name.clone(),
                RoleEntry::new(
                    name.clone(),
                    request.role_prompt,
                    request.file_patterns,
                    request.voice_name,
                ),
            );
            match s.roles.save(&roles) {
                Ok(()) => respond::json(StatusCode::CREATED, &json!({ "roleName": name })),
                Err(e) => respond::internal(&e),
            }
        })
        .await
}

pub async fn update_role(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
    body: Bytes,
) -> Response {
    let request: RoleRequest = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            let mut roles = match load_roles(s) {
                Ok(roles) => roles,
                Err(response) => return response,
            };
            if !roles.contains_key(&name) {
                return respond::not_found();
            }
            let prompt = request.role_prompt.clone();
            roles.insert(
                name.clone(),
                RoleEntry::new(
                    name.clone(),
                    request.role_prompt,
                    request.file_patterns,
                    request.voice_name,
                ),
            );
            if let Err(e) = s.roles.save(&roles) {
                return respond::internal(&e);
            }
            if s.config.current_role.as_deref() == Some(name.as_str()) {
                s.config.system_prompt = Some(prompt);
                if let Err(e) = s.store_config() {
                    return respond::internal(&e);
                }
            }
            ok(&json!({ "roleName": name }))
        })
        .await
}

pub async fn delete_role(
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Response {
    state
        .run(move |s| {
            let mut roles = match load_roles(s) {
                Ok(roles) => roles,
                Err(response) => return response,
            };
            if roles.shift_remove(&name).is_none() {
                return respond::not_found();
            }
            if let Err(e) = s.roles.save(&roles) {
                return respond::internal(&e);
            }
            if s.config.current_role.as_deref() == Some(name.as_str()) {
                s.config.current_role = None;
                s.config.system_prompt = None;
                if let Err(e) = s.store_config() {
                    return respond::internal(&e);
                }
            }
            ok(&json!({ "roleName": name }))
        })
        .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveRole {
    role_name: String,
}

pub async fn activate_role(State(state): State<AppState>, body: Bytes) -> Response {
    let request: ActiveRole = match decode(&body) {
        Ok(request) => request,
        Err(response) => return response,
    };
    state
        .run(move |s| {
            let roles = match load_roles(s) {
                Ok(roles) => roles,
                Err(response) => return response,
            };
            let Some(role) = roles.get(&request.role_name) else {
                return respond::not_found();
            };
            s.config.current_role = Some(role.role_name.clone());
            s.config.system_prompt = Some(role.role_prompt.clone());
            if let Err(e) = s.store_config() {
                return respond::internal(&e);
            }
            ok(&json!({ "roleName": role.role_name, "rolePrompt": role.role_prompt }))
        })
        .await
}
