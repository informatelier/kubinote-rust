use kubi_kernel::governance::ModelIssue;
use kubi_kernel::identity::CanonicalAggregateId;
use kubirag_domain::notebook::RagNotebookManifest;

use super::{ManifestReadResult, ManifestRepository, ManifestWriteError};

/// Il notebook non si può aprire. I messaggi sono quelli di `RagNotebookManifestManager.load`.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("RAG_LEGACY_NOTEBOOK_LAYOUT_UNSUPPORTED: recreate or import notebook [{0}]")]
    LegacyLayout(String),
    #[error("RAG_LEGACY_NOTEBOOK_LAYOUT_UNSUPPORTED: authoritative manifest is missing for [{0}]")]
    Missing(String),
    #[error("Unsupported RAG Notebook manifest version {0}")]
    UnsupportedVersion(String),
    #[error("Invalid RAG Notebook manifest: {0}")]
    Invalid(String),
    #[error(transparent)]
    Write(#[from] ManifestWriteError),
}

/// Tiene lo stato corrente dell'aggregato e lo sostituisce solo dopo un commit riuscito.
pub struct RagNotebookManifestManager {
    repository: Box<dyn ManifestRepository>,
    current: RagNotebookManifest,
}

impl RagNotebookManifestManager {
    /// `load(name, initializeIfMissing)`: un notebook mancante viene creato vuoto, con identità
    /// nuova, solo se richiesto.
    pub fn load(
        repository: Box<dyn ManifestRepository>,
        name: &str,
        initialize_if_missing: bool,
    ) -> Result<Self, LoadError> {
        let current = match repository.read(name) {
            ManifestReadResult::Current(manifest) => *manifest,
            ManifestReadResult::LegacyMissingIdentity { .. } => {
                return Err(LoadError::LegacyLayout(name.to_owned()))
            }
            ManifestReadResult::UnsupportedVersion(version) => {
                return Err(LoadError::UnsupportedVersion(version))
            }
            ManifestReadResult::Invalid(diagnostic) => return Err(LoadError::Invalid(diagnostic)),
            ManifestReadResult::Missing if initialize_if_missing => {
                let created = RagNotebookManifest::empty(CanonicalAggregateId::random(), name)
                    .map_err(|e| ManifestWriteError::Rejected(e.to_string()))?;
                repository.write(&created, None)?;
                created
            }
            ManifestReadResult::Missing => return Err(LoadError::Missing(name.to_owned())),
        };
        Ok(Self {
            repository,
            current,
        })
    }

    pub fn snapshot(&self) -> &RagNotebookManifest {
        &self.current
    }

    pub fn name(&self) -> &str {
        self.current.name()
    }

    pub fn issues(&self) -> Vec<ModelIssue> {
        self.repository.issues(&self.current)
    }

    /// Scrive con compare-and-set e rilegge: lo stato corrente diventa quello normalizzato dal
    /// writer, non quello proposto.
    pub fn commit(
        &mut self,
        next: &RagNotebookManifest,
        expected_revision: i64,
    ) -> Result<&RagNotebookManifest, ManifestWriteError> {
        self.repository.write(next, Some(expected_revision))?;
        match self.repository.read(next.name()) {
            ManifestReadResult::Current(manifest) => {
                self.current = *manifest;
                Ok(&self.current)
            }
            other => Err(ManifestWriteError::Failure(format!(
                "Committed Notebook manifest cannot be read back: {other:?}"
            ))),
        }
    }
}
