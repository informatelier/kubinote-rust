//! Notebook Governance: caricamento dell'aggregato, transizioni governate dai comandi,
//! concorrenza ottimistica, domain event nel journal e ricostruzione delle proiezioni.

mod manager;
mod ports;
mod service;

pub use manager::{LoadError, RagNotebookManifestManager};
pub use ports::{
    Clock, IdGenerator, ManifestReadResult, ManifestRepository, ManifestWriteError, NoProjection,
    NotebookAuditLog, NotebookProjection, RandomIds, SystemClock,
};
pub use service::RagNotebookApplicationService;
