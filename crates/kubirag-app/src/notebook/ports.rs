use kubi_kernel::governance::{AggregateIdentityConflict, ModelIssue, RevisionConflictError};
use kubi_kernel::identity::CanonicalAggregateId;
use kubirag_domain::notebook::{RagNotebookAuditEvent, RagNotebookManifest};

/// Esito della lettura del manifest, come `RagNotebookManifestReadResult` in Kotlin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestReadResult {
    Current(Box<RagNotebookManifest>),
    /// Manifest senza `id`: va migrato prima di qualsiasi scrittura governata.
    LegacyMissingIdentity {
        name: String,
    },
    UnsupportedVersion(String),
    Invalid(String),
    Missing,
}

/// Scrittura del manifest rifiutata. I messaggi sono quelli Kotlin: finiscono nell'audit.
#[derive(Debug, thiserror::Error)]
pub enum ManifestWriteError {
    #[error(transparent)]
    RevisionConflict(#[from] RevisionConflictError),
    #[error(transparent)]
    IdentityConflict(#[from] AggregateIdentityConflict),
    /// Regola violata (portabilità, risorsa gestita mancante, revisione che non avanza…).
    #[error("{0}")]
    Rejected(String),
    /// Guasto tecnico dell'adattatore (I/O, lock, serializzazione).
    #[error("{0}")]
    Failure(String),
}

/// Porta di persistenza del manifest autoritativo `_qb/notes/<store>/<store>-manifest.json`.
pub trait ManifestRepository: Send {
    fn read(&self, name: &str) -> ManifestReadResult;

    /// Con `expected_revision` è un compare-and-set sotto lock: la revisione su disco deve
    /// valere `expected` e quella nuova `expected + 1`. Senza, scrive solo se l'identità
    /// coincide (creazione). Il writer normalizza le risorse per la portabilità.
    fn write(
        &self,
        manifest: &RagNotebookManifest,
        expected_revision: Option<i64>,
    ) -> Result<(), ManifestWriteError>;

    /// Diagnostiche sulle risorse registrate (file mancanti, contenuti cambiati).
    fn issues(&self, manifest: &RagNotebookManifest) -> Vec<ModelIssue>;
}

/// Porta del journal dei domain event del notebook.
pub trait NotebookAuditLog: Send {
    fn append(&self, event: RagNotebookAuditEvent) -> Result<(), String>;
    fn events(&self) -> Result<Vec<RagNotebookAuditEvent>, String>;
}

/// Viste derivate (documenti a runtime, indice, embedding) ricostruite dopo ogni commit.
pub trait NotebookProjection: Send {
    fn rebuild(
        &self,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
    ) -> Result<(), String>;
}

/// `RagNotebookProjection.None`.
pub struct NoProjection;

impl NotebookProjection for NoProjection {
    fn rebuild(&self, _: &RagNotebookManifest, _: &RagNotebookManifest) -> Result<(), String> {
        Ok(())
    }
}

/// Orologio degli eventi (`Instant.now().toString()`), sostituibile nei test.
pub trait Clock: Send {
    fn now(&self) -> String;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> String {
        kubi_kernel::time::now()
    }
}

/// Generatore degli id degli eventi e dei nuovi notebook (`UUID.randomUUID()`).
pub trait IdGenerator: Send {
    fn new_id(&self) -> String;
}

pub struct RandomIds;

impl IdGenerator for RandomIds {
    fn new_id(&self) -> String {
        CanonicalAggregateId::random().as_str().to_owned()
    }
}
