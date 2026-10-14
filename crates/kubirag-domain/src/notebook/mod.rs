//! Bounded context **Notebook Governance**.
//!
//! Aggregate root: [`RagNotebookManifest`] (`_qb/notes/<store>/<store>-manifest.json`).
//! Comandi: [`RagNotebookCommand`], decisi in modo puro sull'aggregato.
//! Domain event: [`RagNotebookAuditEvent`], con snapshot prima/dopo, nel journal atomico.

mod audit;
mod command;
mod document;
mod locator;
mod manifest;
mod resource;

pub use audit::{RagNotebookAuditEvent, RagNotebookAuditSnapshot, RagNotebookCommandType};
pub use command::{
    document_resource, property_resources, NoFingerprints, RagNotebookCommand,
    ResourceFingerprints, PATH_METADATA,
};
pub use document::{NotebookProperties, RagDocument};
pub use locator::{is_bound, is_remote, PortableLocator};
pub use manifest::{
    stable_notebook_id, ManifestDraft, RagNotebookManifest, RAG_NOTEBOOK_MANIFEST_VERSION,
};
pub use resource::{NotebookResourceRef, ResourceOwnership};

/// Invariante dell'aggregato violata. I messaggi coincidono con quelli Kotlin, così le
/// diagnostiche restano confrontabili durante la coesistenza delle due versioni.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotebookError {
    #[error("{0}")]
    Invariant(&'static str),
    /// Precondizione di un comando non soddisfatta (o regola di portabilità violata).
    #[error("{0}")]
    Rejected(String),
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
