use std::fs;
use std::path::{Path, PathBuf};

use kubi_kernel::domain::concept::{AuthorityConceptGraphFile, ConceptGraphError};
use kubi_kernel::json::JsonDialect;
use kubirag_domain::concept::{canonical, validate, ConceptGraphValidation};

pub const CONCEPT_GRAPH_FILENAME: &str = "qb-concepts.json";

#[derive(Debug, thiserror::Error)]
pub enum ConceptGraphStoreError {
    #[error("Invalid {path}: {message}")]
    Invalid { path: PathBuf, message: String },
    #[error(transparent)]
    Validation(#[from] ConceptGraphValidation),
    #[error("Concept revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: i64, actual: i64 },
    #[error("Concept aggregate identity conflict")]
    IdentityConflict,
    #[error(transparent)]
    Envelope(#[from] ConceptGraphError),
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for ConceptGraphStoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

/// Grafo autoritativo dei concetti `_qb/qb-concepts.json` (`AuthoritativeConceptGraphRepository`):
/// dialetto governance (4 spazi, default scritti) più un newline finale, forma canonica,
/// revisione che avanza di uno a ogni scrittura.
pub struct JsonConceptGraphRepository {
    authority_dir: PathBuf,
}

impl JsonConceptGraphRepository {
    pub fn new(authority_dir: impl Into<PathBuf>) -> Self {
        Self {
            authority_dir: authority_dir.into(),
        }
    }

    pub fn graph_file(&self) -> PathBuf {
        self.authority_dir.join(CONCEPT_GRAPH_FILENAME)
    }

    pub fn encode(graph: &AuthorityConceptGraphFile) -> serde_json::Result<String> {
        Ok(JsonDialect::GOVERNANCE.pretty(&serde_json::to_value(graph)?)? + "\n")
    }

    pub fn decode(
        text: &str,
        path: &Path,
    ) -> Result<AuthorityConceptGraphFile, ConceptGraphStoreError> {
        let graph: AuthorityConceptGraphFile =
            serde_json::from_str(text).map_err(|e| ConceptGraphStoreError::Invalid {
                path: path.to_path_buf(),
                message: e.to_string(),
            })?;
        validate(&graph)?;
        Ok(graph)
    }

    /// Un grafo mancante è vuoto, con identità nuova; uno non valido è un errore.
    pub fn read_manifest(&self) -> Result<AuthorityConceptGraphFile, ConceptGraphStoreError> {
        let file = self.graph_file();
        match fs::read_to_string(&file) {
            Ok(text) => Self::decode(&text, &file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(AuthorityConceptGraphFile::empty())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// `write(manifest, expectedRevision)`: la revisione scritta è quella corrente + 1 (o
    /// quella del grafo se il file non esiste), sempre in forma canonica e validata.
    pub fn write(
        &self,
        graph: AuthorityConceptGraphFile,
        expected_revision: Option<i64>,
    ) -> Result<AuthorityConceptGraphFile, ConceptGraphStoreError> {
        let file = self.graph_file();
        let current = if file.exists() {
            Some(self.read_manifest()?)
        } else {
            None
        };
        let actual = current.as_ref().map_or(0, |c| c.revision());
        if let Some(expected) = expected_revision.filter(|expected| *expected != actual) {
            return Err(ConceptGraphStoreError::RevisionConflict { expected, actual });
        }
        if current.as_ref().is_some_and(|c| c.id() != graph.id()) {
            return Err(ConceptGraphStoreError::IdentityConflict);
        }
        let revision = current.map_or(graph.revision(), |c| c.revision() + 1);
        let canonical = canonical(graph.with_revision(revision)?);
        validate(&canonical)?;
        let text =
            Self::encode(&canonical).map_err(|e| ConceptGraphStoreError::Io(e.to_string()))?;
        kubi_kernel::store::write_verified(
            &file,
            &self.authority_dir,
            ".qb-concepts.json.",
            &text,
            |written| match Self::decode(written, &file) {
                Ok(decoded) if decoded == canonical => Ok(()),
                _ => Err(ConceptGraphStoreError::Io(
                    "Staged concept graph did not round-trip".into(),
                )),
            },
        )?;
        Ok(canonical)
    }
}
