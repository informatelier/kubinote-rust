//! Le route, raggruppate per bounded context come i controller di `RagServer`.

mod conversation;
pub mod deferred;
mod documents;
mod governance;
mod retrieval;
mod settings;
mod stores;
mod studio;

use axum::extract::DefaultBodyLimit;
use axum::response::Response;
use axum::routing::{delete, get, on, post, put, MethodFilter};
use axum::Router;

use crate::assets::STATIC_ASSETS;
use crate::respond;
use crate::state::{AppState, Server};
use deferred::DEFERRED_ROUTES;

/// Massimo corpo accettato (upload di sorgenti): lo stesso ordine di grandezza di Javalin.
const MAX_BODY_BYTES: usize = 512 * 1024 * 1024;

/// Allinea il notebook attivo al manifest su disco, poi esegue l'azione.
fn synced(s: &mut Server, action: impl FnOnce(&mut Server) -> Response) -> Response {
    match s.sync() {
        Ok(()) => action(s),
        Err(e) => respond::internal(&e),
    }
}

fn method_filter(route: &deferred::DeferredRoute) -> MethodFilter {
    MethodFilter::try_from(route.http_method()).expect("deferred routes use standard verbs")
}

/// Il router completo: asset statici, API implementate, route rimandate.
pub fn router(state: AppState) -> Router {
    let mut router = Router::new();
    for asset in STATIC_ASSETS {
        router = router.route(asset.route, get(move || async move { asset.response() }));
    }
    router = router
        // Notebook Governance
        .route("/governance", get(governance::status))
        .route("/governance/issues", get(governance::issues))
        .route("/governance/audit", get(governance::audit))
        .route("/governance/health", get(governance::health))
        .route("/governance/rebuild", post(governance::rebuild))
        .route("/governance/undo", post(governance::undo))
        .route(
            "/notebook",
            get(governance::notebook).post(governance::save_notebook),
        )
        .route("/notebook/asset/{file}", get(governance::notebook_asset))
        .route("/pinned", get(governance::pinned))
        .route(
            "/pinned/{id}",
            post(governance::pin).delete(governance::unpin),
        )
        .route("/documents", get(documents::list))
        .route("/documents/upload", post(documents::upload))
        .route(
            "/documents/{id}",
            get(documents::detail).delete(governance::remove_document),
        )
        .route("/documents/{id}/asset", get(documents::asset))
        .route(
            "/documents/groups/{parent}",
            delete(documents::remove_group),
        )
        .route("/stores", get(stores::list).post(stores::create))
        .route("/stores/active", post(stores::switch))
        .route("/stores/{name}", delete(stores::delete))
        .route("/stores/{name}/save", post(stores::save))
        // Retrieval & Q&A
        .route("/search", get(retrieval::search))
        .route("/embedding-projection", get(retrieval::status))
        .route("/embedding-projection/rebuild", post(retrieval::rebuild))
        .route("/embedding-projection/{name}", get(retrieval::status_for))
        // Conversation
        .route("/chat/query", post(conversation::query))
        .route(
            "/chat/sessions",
            get(conversation::sessions).post(conversation::create_session),
        )
        .route(
            "/chat/sessions/{id}",
            get(conversation::session)
                .put(conversation::rename_session)
                .delete(conversation::delete_session),
        )
        .route(
            "/chat/sessions/{id}/branch",
            post(conversation::branch_session),
        )
        .route("/context/usage", get(conversation::context_usage))
        .route(
            "/roles",
            get(conversation::roles).post(conversation::create_role),
        )
        .route("/roles/active", post(conversation::activate_role))
        .route(
            "/roles/{name}",
            put(conversation::update_role).delete(conversation::delete_role),
        )
        // Settings
        .route(
            "/config",
            get(settings::config).put(settings::update_config),
        )
        .route("/models", get(settings::models))
        // Notes & Studio
        .route(
            "/templates",
            get(studio::templates).post(studio::create_template),
        )
        .route("/templates/{name}", get(studio::template));
    for route in DEFERRED_ROUTES {
        router = router.route(
            route.path,
            on(
                method_filter(route),
                move || async move { route.response() },
            ),
        );
    }
    router
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}
