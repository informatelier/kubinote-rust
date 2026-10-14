use indexmap::IndexMap;
use kubi_kernel::governance::{ModelIssue, OperationChannel, OperationStatus};
use serde::{Deserialize, Serialize};

/// Comandi espliciti dell'aggregato notebook (linguaggio dell'event storming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RagNotebookCommandType {
    AddDocument,
    AddAndPinDocument,
    AddAndPinDocuments,
    ReplaceDocument,
    ReplaceDocuments,
    RemoveDocument,
    RemoveDocuments,
    Pin,
    Unpin,
    SetPins,
    UpdateProperties,
    RefreshExternal,
    RebuildProjections,
    Reconcile,
    Undo,
}

/// Stato minimo del notebook prima o dopo un comando: hash dei contenuti, pin, risorse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagNotebookAuditSnapshot {
    pub notebook_id: String,
    pub revision: i64,
    /// id documento → SHA-256 del contenuto, nell'ordine dei documenti.
    pub documents: IndexMap<String, String>,
    pub pinned_document_ids: Vec<String>,
    pub resource_ids: Vec<String>,
}

/// Domain event del notebook, persistito nel journal `_qb/governance/rag/notebooks/<id>.json`.
/// Serializza sempre tutti i campi (dialetto `GOVERNANCE`, `encodeDefaults = true`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagNotebookAuditEvent {
    pub id: String,
    pub command: RagNotebookCommandType,
    pub target_id: String,
    pub actor: String,
    pub channel: OperationChannel,
    pub correlation_id: String,
    pub timestamp: String,
    pub expected_revision: i64,
    pub previous_revision: i64,
    pub current_revision: i64,
    pub status: OperationStatus,
    #[serde(default)]
    pub diagnostic: Option<String>,
    #[serde(default)]
    pub issues: Vec<ModelIssue>,
    pub before: RagNotebookAuditSnapshot,
    pub after: RagNotebookAuditSnapshot,
}

impl RagNotebookAuditEvent {
    pub fn event_id(event: &Self) -> &str {
        &event.id
    }
}
