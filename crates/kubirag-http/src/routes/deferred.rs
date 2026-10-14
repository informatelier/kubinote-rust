//! Le route di `RagServer` dei bounded context non ancora portati. Rispondono `501` con un
//! corpo esplicito (`code: NOT_IMPLEMENTED` e la fase del porting), così la SPA mostra un
//! errore leggibile invece di un 404 e il test di copertura sa che cosa manca e quando arriva.

use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::json;

use crate::respond;

/// Una route rimandata: verbo, path (sintassi axum), fase e bounded context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredRoute {
    pub method: &'static str,
    pub path: &'static str,
    pub phase: &'static str,
    pub context: &'static str,
}

const fn route(
    method: &'static str,
    path: &'static str,
    phase: &'static str,
    context: &'static str,
) -> DeferredRoute {
    DeferredRoute {
        method,
        path,
        phase,
        context,
    }
}

const KNOWLEDGE: &str = "Knowledge";
const STUDIO: &str = "Notes & Studio";
const PUBLISHING: &str = "Publishing";
const GOVERNANCE: &str = "Notebook Governance";
const RETRIEVAL: &str = "Retrieval & Q&A";
const ANALYSIS: &str = "Analysis";

/// Tutte le route di `RagServer` (kubirag 09aba65) che il server Rust non implementa ancora.
pub const DEFERRED_ROUTES: &[DeferredRoute] = &[
    // F4 Conoscenza: concetti, evidenze, identità e permalink, bound.
    route("GET", "/bounds", "F4", KNOWLEDGE),
    route("GET", "/bounds/{name}", "F4", KNOWLEDGE),
    route("GET", "/bounds/{name}/resources", "F4", KNOWLEDGE),
    route("GET", "/bounds/{name}/updates", "F4", KNOWLEDGE),
    route("GET", "/bounds/{name}/concepts", "F4", KNOWLEDGE),
    route("POST", "/bounds/{name}/sources", "F4", KNOWLEDGE),
    route("GET", "/evidence", "F4", KNOWLEDGE),
    route("GET", "/evidence/status", "F4", KNOWLEDGE),
    route("POST", "/evidence/discover", "F4", KNOWLEDGE),
    route("POST", "/evidence/web", "F4", KNOWLEDGE),
    route("POST", "/evidence/export", "F4", KNOWLEDGE),
    route("POST", "/evidence/{id}/reject", "F4", KNOWLEDGE),
    route("POST", "/evidence/{id}/accept", "F4", KNOWLEDGE),
    route("GET", "/concepts", "F4", KNOWLEDGE),
    route("GET", "/concepts/graph", "F4", KNOWLEDGE),
    route("GET", "/concepts/tags", "F4", KNOWLEDGE),
    route("GET", "/concepts/predicates", "F4", KNOWLEDGE),
    route("GET", "/concepts/targets", "F4", KNOWLEDGE),
    route("GET", "/concepts/suggestions", "F4", KNOWLEDGE),
    route("POST", "/concepts/suggestions/{id}/accept", "F4", KNOWLEDGE),
    route("POST", "/concepts/suggestions/{id}/reject", "F4", KNOWLEDGE),
    route("POST", "/concepts/{concept}/relations", "F4", KNOWLEDGE),
    route("DELETE", "/concepts/{concept}/relations", "F4", KNOWLEDGE),
    route("GET", "/concepts/{concept}", "F4", KNOWLEDGE),
    route("GET", "/concepts/{concept}/evidence", "F4", KNOWLEDGE),
    route("DELETE", "/concepts/{concept}", "F4", KNOWLEDGE),
    route("POST", "/concepts/note", "F4", KNOWLEDGE),
    route("POST", "/concepts/link", "F4", KNOWLEDGE),
    route("POST", "/concepts/unlink", "F4", KNOWLEDGE),
    route("POST", "/concepts/insert", "F4", KNOWLEDGE),
    route("POST", "/concepts/alias", "F4", KNOWLEDGE),
    route("POST", "/concepts/unalias", "F4", KNOWLEDGE),
    route("POST", "/concepts/rename", "F4", KNOWLEDGE),
    route("POST", "/concepts/export", "F4", KNOWLEDGE),
    route("POST", "/concepts/okf/export", "F4", KNOWLEDGE),
    route("POST", "/concepts/okf/import", "F4", KNOWLEDGE),
    route("GET", "/resources/identity/status", "F4", KNOWLEDGE),
    route("POST", "/resources/identity/migrate", "F4", KNOWLEDGE),
    route("GET", "/resources/search", "F4", KNOWLEDGE),
    route("GET", "/resources/resolve", "F4", KNOWLEDGE),
    route("POST", "/resources/permalinks", "F4", KNOWLEDGE),
    route("GET", "/resources/permalinks/preview", "F4", KNOWLEDGE),
    route("POST", "/resources/permalinks/canonical", "F4", KNOWLEDGE),
    route("POST", "/resources/permalinks/repair", "F4", KNOWLEDGE),
    route("POST", "/resources/unavailable", "F4", KNOWLEDGE),
    route("POST", "/resources/merge", "F4", KNOWLEDGE),
    route("POST", "/resources/tombstone", "F4", KNOWLEDGE),
    route("GET", "/pages/{id}", "F4", KNOWLEDGE),
    route("GET", "/terms/{id}", "F4", KNOWLEDGE),
    // F5 Note e pubblicazione: note, artefatti, studio, wiki, slide, libro.
    route("GET", "/notes", "F5", STUDIO),
    route("POST", "/notes", "F5", STUDIO),
    route("GET", "/notes/tags", "F5", STUDIO),
    route("GET", "/notes/folders", "F5", STUDIO),
    route("POST", "/notes/folder", "F5", STUDIO),
    route("POST", "/notes/folders", "F5", STUDIO),
    route("GET", "/notes/watch/status", "F5", STUDIO),
    route("POST", "/notes/watch/start", "F5", STUDIO),
    route("POST", "/notes/watch/stop", "F5", STUDIO),
    route("POST", "/notes/watch/scan", "F5", STUDIO),
    route("POST", "/notes/watch/refresh", "F5", STUDIO),
    route("POST", "/notes/publication/publish-all", "F5", PUBLISHING),
    route("GET", "/notes/relations/{name}", "F5", STUDIO),
    route("GET", "/notes/{name}/relations", "F5", STUDIO),
    route("PUT", "/notes/{name}/relations", "F5", STUDIO),
    route("GET", "/notes/{name}/asset", "F5", STUDIO),
    route("GET", "/notes/{name}/publication", "F5", PUBLISHING),
    route(
        "POST",
        "/notes/{name}/publication/publish",
        "F5",
        PUBLISHING,
    ),
    route("GET", "/notes/{name}", "F5", STUDIO),
    route("PUT", "/notes/{name}", "F5", STUDIO),
    route("POST", "/notes/{name}/rename", "F5", STUDIO),
    route("DELETE", "/notes/{name}", "F5", STUDIO),
    route("POST", "/notes/{name}/pin", "F5", STUDIO),
    route("GET", "/artifacts", "F5", STUDIO),
    route("POST", "/artifacts", "F5", STUDIO),
    route("GET", "/artifacts/content", "F5", STUDIO),
    route("PUT", "/artifacts/content", "F5", STUDIO),
    route("DELETE", "/artifacts/content", "F5", STUDIO),
    route("POST", "/artifacts/rename", "F5", STUDIO),
    route("POST", "/artifacts/migration", "F5", STUDIO),
    route("GET", "/canvas-api/resources", "F5", STUDIO),
    route("POST", "/canvas-api/concepts", "F5", STUDIO),
    route("POST", "/canvas-api/import", "F5", STUDIO),
    route("POST", "/quiz/generate", "F5", STUDIO),
    route("GET", "/report/stats", "F5", STUDIO),
    route("POST", "/report/generate", "F5", STUDIO),
    route("POST", "/enrich/start", "F5", STUDIO),
    route("GET", "/enrich/status", "F5", STUDIO),
    route("POST", "/enrich/stop", "F5", STUDIO),
    route("POST", "/slides/generate", "F5", PUBLISHING),
    route("GET", "/slides/{filename}", "F5", PUBLISHING),
    route("POST", "/wiki/generate", "F5", PUBLISHING),
    route("GET", "/wiki/status", "F5", PUBLISHING),
    route("GET", "/wiki/page-ids/status", "F5", PUBLISHING),
    route("POST", "/wiki/page-ids/prepare", "F5", PUBLISHING),
    route("GET", "/wiki/README.md", "F5", PUBLISHING),
    route("GET", "/wiki/_sidebar.md", "F5", PUBLISHING),
    route("GET", "/wiki/marked-5-1-1.min.js", "F5", PUBLISHING),
    route("GET", "/wiki/sources/{*path}", "F5", PUBLISHING),
    route("GET", "/wiki/{*path}", "F5", PUBLISHING),
    route("GET", "/wiki", "F5", PUBLISHING),
    route("GET", "/wiki/", "F5", PUBLISHING),
    route("GET", "/book/notes", "F5", PUBLISHING),
    route("POST", "/book/generate", "F5", PUBLISHING),
    route("GET", "/book/{filename}", "F5", PUBLISHING),
    route("POST", "/documents/pdf-chunked", "F5", GOVERNANCE),
    route("POST", "/documents/scrape", "F5", GOVERNANCE),
    route("POST", "/stores/import-folder", "F5", GOVERNANCE),
    route("POST", "/stores/archive/export", "F5", GOVERNANCE),
    route("POST", "/stores/archive/import", "F5", GOVERNANCE),
    route("GET", "/chat/query", "F5", RETRIEVAL),
    // F6 Resto: mindmap LSA.
    route("GET", "/mindmap", "F6", ANALYSIS),
    route("GET", "/mindmap/terms", "F6", ANALYSIS),
    route("GET", "/mindmap/manual", "F6", ANALYSIS),
    route("PUT", "/mindmap/manual", "F6", ANALYSIS),
    route("DELETE", "/mindmap/manual", "F6", ANALYSIS),
];

impl DeferredRoute {
    pub fn http_method(&self) -> Method {
        Method::from_bytes(self.method.as_bytes()).expect("deferred routes use standard verbs")
    }

    pub fn response(&self) -> Response {
        respond::json(
            StatusCode::NOT_IMPLEMENTED,
            &json!({
                "error": format!(
                    "{} {} is not available in the Rust server yet ({}: {})",
                    self.method, self.path, self.phase, self.context
                ),
                "code": "NOT_IMPLEMENTED",
                "phase": self.phase,
            }),
        )
    }
}
