//! Bounded context **Notebook Governance**.
//!
//! Aggregate root: [`RagNotebookManifest`] (`_qb/notes/<store>/<store>-manifest.json`).
//! Domain event: [`RagNotebookAuditEvent`], con snapshot prima/dopo, nel journal atomico.

mod audit;
mod document;
mod manifest;
mod resource;

pub use audit::{RagNotebookAuditEvent, RagNotebookAuditSnapshot, RagNotebookCommandType};
pub use document::{NotebookProperties, RagDocument};
pub use manifest::{stable_notebook_id, RagNotebookManifest, RAG_NOTEBOOK_MANIFEST_VERSION};
pub use resource::{NotebookResourceRef, ResourceOwnership};

/// Invariante dell'aggregato violata. I messaggi coincidono con quelli Kotlin, così le
/// diagnostiche restano confrontabili durante la coesistenza delle due versioni.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotebookError {
    #[error("{0}")]
    Invariant(&'static str),
    #[error("Unsupported RAG Notebook manifest version {0}")]
    UnsupportedVersion(String),
    #[error(transparent)]
    Identity(#[from] kubi_kernel::identity::IdentityError),
}

pub(crate) fn ensure(condition: bool, message: &'static str) -> Result<(), NotebookError> {
    if condition {
        Ok(())
    } else {
        Err(NotebookError::Invariant(message))
    }
}

pub(crate) fn is_blank(value: &str) -> bool {
    value.chars().all(char::is_whitespace)
}
