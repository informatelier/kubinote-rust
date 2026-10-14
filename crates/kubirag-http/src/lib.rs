//! # kubirag-http
//!
//! Adattatore HTTP (driving) di KubiRAG su axum, al posto del server Javalin di `RagServer`
//! per la SPA esistente, senza toccarne il JavaScript (F3, O3).
//!
//! | Modulo | Ruolo |
//! |---|---|
//! | [`assets`] | il frontend incorporato nel binario, sulle stesse route e con gli stessi tipi |
//! | `routes` | handler per bounded context; le route non ancora portate rispondono `501` |
//! | `respond` | convenzioni di Javalin e di `NotebookGovernanceHttp` (`If-Match`, `ETag`, 404) |
//! | [`state`] | composition root del server: notebook attivo, configurazione, archivi |
//!
//! Le risposte replicano quelle Kotlin byte per byte dove la SPA le legge; lo scenario di
//! `contracts/golden/http` lo verifica in `kubirag-acceptance`.

// Gli handler restituiscono `Result<_, Response>` per uscire presto con la risposta d'errore:
// è il ramo freddo e viene subito convertito, quindi la dimensione di `Response` non pesa.
#![allow(clippy::result_large_err)]

pub mod assets;
mod respond;
mod routes;
pub mod state;

use std::net::SocketAddr;

use axum::middleware::map_response;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

pub use routes::deferred::{DeferredRoute, DEFERRED_ROUTES};
pub use state::{AppState, ServerOptions};

/// Il router con i middleware di `RagServer`: CORS aperto (`anyHost`) e il corpo
/// `{"result":"not found"}` su ogni 404.
pub fn app(state: AppState) -> Router {
    routes::router(state)
        .layer(map_response(respond::rewrite_not_found))
        .layer(CorsLayer::permissive())
}

/// Serve l'applicazione finché `shutdown` non si completa.
pub async fn serve(
    listener: TcpListener,
    state: AppState,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}
