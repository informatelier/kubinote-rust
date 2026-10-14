//! Le convenzioni HTTP di Javalin e di `NotebookGovernanceHttp`: JSON compatto, `If-Match`
//! obbligatorio sulle scritture governate, `ETag`/`X-QB-Revision`, stati dell'`OperationResult`.

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use kubi_kernel::governance::{
    OperationChannel, OperationContext, OperationResult, OperationStatus,
};
use serde::Serialize;
use serde_json::{json, Value};

pub const JSON: &str = "application/json";

/// `ctx.json(...)`: il corpo serializzato compatto.
pub fn json(status: StatusCode, body: &impl Serialize) -> Response {
    match serde_json::to_vec(body) {
        Ok(bytes) => (status, [(header::CONTENT_TYPE, JSON)], bytes).into_response(),
        Err(e) => internal(&e.to_string()),
    }
}

pub fn ok(body: &impl Serialize) -> Response {
    json(StatusCode::OK, body)
}

/// Un `f32` come lo scrive Jackson: la sua rappresentazione decimale più corta, non quella
/// del `f64` in cui `json!` lo allargherebbe (`0.1968244`, non `0.19682440161705017`).
pub use kubi_kernel::json::kotlin_float as float;

/// `{"error": message}` con lo stato dato.
pub fn error(status: StatusCode, message: &str) -> Response {
    json(status, &json!({ "error": message }))
}

pub fn internal(message: &str) -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, message)
}

/// Il corpo che il gestore `error(404)` di `RagServer` mette su ogni 404, qualunque cosa
/// avesse scritto il controller.
pub fn not_found() -> Response {
    json(StatusCode::NOT_FOUND, &json!({ "result": "not found" }))
}

/// Middleware: ogni 404 diventa `{"result":"not found"}`, come in Javalin.
pub async fn rewrite_not_found(response: Response) -> Response {
    if response.status() == StatusCode::NOT_FOUND {
        not_found()
    } else {
        response
    }
}

/// `NotebookGovernanceHttp.expectedRevision`: `If-Match` assente → 428, illeggibile → 400;
/// `W/` e virgolette sono tolti.
#[allow(clippy::result_large_err)]
pub fn expected_revision(headers: &HeaderMap) -> Result<i64, Response> {
    let Some(raw) = headers.get(header::IF_MATCH) else {
        return Err(error(
            StatusCode::PRECONDITION_REQUIRED,
            "If-Match revision required",
        ));
    };
    let raw = raw.to_str().unwrap_or_default().trim();
    let normalized = raw.strip_prefix("W/").unwrap_or(raw).trim();
    let normalized = normalized
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(normalized);
    normalized
        .parse::<i64>()
        .ok()
        .filter(|r| *r >= 0)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Invalid If-Match revision"))
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.trim().is_empty())
}

/// `NotebookGovernanceHttp.context`: attore da `X-QB-Actor` (default `api-web`), canale
/// `API_WEB`, correlazione da `X-Correlation-ID` o casuale.
pub fn operation_context(headers: &HeaderMap, expected: i64) -> OperationContext {
    let actor = header_text(headers, "X-QB-Actor").unwrap_or("api-web");
    let correlation = header_text(headers, "X-Correlation-ID").map(str::to_owned);
    OperationContext::new(actor, OperationChannel::ApiWeb, expected, correlation)
        .expect("a non-blank actor and a non-negative revision make a valid context")
}

/// Il contesto delle ricostruzioni interne (cambio e creazione di notebook).
pub fn system_context(expected: i64) -> OperationContext {
    OperationContext::new("api-web", OperationChannel::ApiWeb, expected, None)
        .expect("a fixed actor and the current revision make a valid context")
}

/// `ETag: "<rev>"` e `X-QB-Revision: <rev>`.
pub fn with_revision(mut response: Response, revision: i64) -> Response {
    let headers = response.headers_mut();
    if let Ok(etag) = HeaderValue::from_str(&format!("\"{revision}\"")) {
        headers.insert(header::ETAG, etag);
    }
    headers.insert("X-QB-Revision", HeaderValue::from(revision));
    response
}

/// L'`OperationResult` come lo serializza Jackson (con la proprietà calcolata `applied`).
pub fn operation_json(result: &OperationResult) -> Value {
    json!({
        "status": result.status(),
        "message": result.message(),
        "correlationId": result.correlation_id(),
        "expectedRevision": result.expected_revision(),
        "previousRevision": result.previous_revision(),
        "currentRevision": result.current_revision(),
        "issues": result.issues(),
        "auditEventId": result.audit_event_id(),
        "applied": result.applied(),
    })
}

/// `NotebookGovernanceHttp.render`: `APPLIED` → `success`, `CONFLICTED` → 409, `REJECTED`
/// → 422, `AUDIT_FAILED` → 500, con la revisione corrente negli header.
pub fn render(result: &OperationResult, success: StatusCode) -> Response {
    let status = match result.status() {
        OperationStatus::Applied => success,
        OperationStatus::Conflicted => StatusCode::CONFLICT,
        OperationStatus::Rejected => StatusCode::UNPROCESSABLE_ENTITY,
        OperationStatus::AuditFailed => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let response = json(status, &operation_json(result));
    match result.current_revision() {
        Some(revision) => with_revision(response, revision),
        None => response,
    }
}

/// Un corpo JSON illeggibile: Jackson lo rifiuterebbe con un 500, qui è un 400 esplicito.
pub fn bad_body(detail: &str) -> Response {
    error(
        StatusCode::BAD_REQUEST,
        &format!("Invalid request body: {detail}"),
    )
}

/// Decodifica un corpo JSON con i default di Jackson + modulo Kotlin.
#[allow(clippy::result_large_err)]
pub fn decode<T: serde::de::DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|e| bad_body(&e.to_string()))
}

/// Un corpo binario con il suo tipo (asset di documenti e notebook).
pub fn bytes(content_type: &str, bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    if let Ok(value) = HeaderValue::from_str(content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_keep_their_shortest_decimal_form() {
        assert_eq!(float(0.196_824_4).to_string(), "0.1968244");
    }

    fn if_match(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_MATCH, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn if_match_accepts_weak_and_quoted_revisions() {
        assert_eq!(expected_revision(&if_match("W/\"7\"")).unwrap(), 7);
        assert_eq!(expected_revision(&if_match(" \"0\" ")).unwrap(), 0);
        assert_eq!(expected_revision(&if_match("3")).unwrap(), 3);
    }

    #[test]
    fn missing_or_invalid_if_match_is_refused() {
        assert_eq!(
            expected_revision(&HeaderMap::new()).unwrap_err().status(),
            StatusCode::PRECONDITION_REQUIRED
        );
        for bad in ["\"abc\"", "\"-1\"", ""] {
            assert_eq!(
                expected_revision(&if_match(bad)).unwrap_err().status(),
                StatusCode::BAD_REQUEST
            );
        }
    }
}
