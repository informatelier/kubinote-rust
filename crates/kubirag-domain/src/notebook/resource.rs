use serde::{Deserialize, Serialize};

use super::{ensure, is_blank, NotebookError};

/// Chi governa i byte della risorsa: il notebook (`MANAGED`, copiata nello store) o terzi.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceOwnership {
    Managed,
    #[default]
    External,
}

/// Risorsa registrata nel notebook. `kind = "document-source:<docId>"` lega la risorsa a un
/// documento: il writer ne ricava `metadata.path` del documento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", try_from = "NotebookResourceRecord")]
pub struct NotebookResourceRef {
    id: String,
    path: String,
    ownership: ResourceOwnership,
    sha256: Option<String>,
    kind: String,
}

impl NotebookResourceRef {
    pub const DEFAULT_KIND: &'static str = "document-source";

    pub fn new(
        id: impl Into<String>,
        path: impl Into<String>,
        ownership: ResourceOwnership,
        sha256: Option<String>,
        kind: impl Into<String>,
    ) -> Result<Self, NotebookError> {
        let resource = Self {
            id: id.into(),
            path: path.into(),
            ownership,
            sha256,
            kind: kind.into(),
        };
        ensure(
            !is_blank(&resource.id),
            "Notebook resource id cannot be blank",
        )?;
        ensure(
            !is_blank(&resource.path),
            "Notebook resource path cannot be blank",
        )?;
        ensure(
            !is_blank(&resource.kind),
            "Notebook resource kind cannot be blank",
        )?;
        Ok(resource)
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn ownership(&self) -> ResourceOwnership {
        self.ownership
    }

    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Documento sorgente collegato, se la risorsa è `document-source:<docId>`.
    pub fn source_document_id(&self) -> Option<&str> {
        self.kind.strip_prefix("document-source:")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotebookResourceRecord {
    /// In Kotlin il default è un UUID casuale: un id assente non è mai riproducibile.
    #[serde(default = "random_id")]
    id: String,
    path: String,
    #[serde(default)]
    ownership: ResourceOwnership,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default = "default_kind")]
    kind: String,
}

fn random_id() -> String {
    kubi_kernel::identity::CanonicalAggregateId::random().to_string()
}

fn default_kind() -> String {
    NotebookResourceRef::DEFAULT_KIND.to_owned()
}

impl TryFrom<NotebookResourceRecord> for NotebookResourceRef {
    type Error = NotebookError;

    fn try_from(r: NotebookResourceRecord) -> Result<Self, Self::Error> {
        Self::new(r.id, r.path, r.ownership, r.sha256, r.kind)
    }
}
