use std::collections::HashSet;
use std::hash::Hash;

use kubi_kernel::governance::Revisioned;
use kubi_kernel::identity::{name_uuid_from_bytes, CanonicalAggregateId};
use serde::{Deserialize, Serialize};

use super::{
    ensure, is_blank, NotebookError, NotebookProperties, NotebookResourceRef, RagDocument,
};

pub const RAG_NOTEBOOK_MANIFEST_VERSION: &str = "1";

/// Aggregate root del notebook: unica autorità su documenti, pin, proprietà e risorse.
///
/// Le invarianti sono garantite dal costruttore e dalla deserializzazione: un valore di questo
/// tipo è sempre valido (id canonico, documenti e risorse con id unici, nessun pin orfano).
/// `revision` serve alla concorrenza ottimistica: ogni commit la fa avanzare di uno.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ManifestRecord", into = "ManifestRecord")]
pub struct RagNotebookManifest {
    id: CanonicalAggregateId,
    name: String,
    revision: i64,
    documents: Vec<RagDocument>,
    pinned_document_ids: Vec<String>,
    properties: NotebookProperties,
    resources: Vec<NotebookResourceRef>,
}

impl RagNotebookManifest {
    pub const AGGREGATE_TYPE: &'static str = "rag-notebook";
    pub const PRODUCER: &'static str = "kubirag";

    pub fn new(
        id: CanonicalAggregateId,
        name: impl Into<String>,
        revision: i64,
        documents: Vec<RagDocument>,
        pinned_document_ids: Vec<String>,
        properties: NotebookProperties,
        resources: Vec<NotebookResourceRef>,
    ) -> Result<Self, NotebookError> {
        let manifest = Self {
            id,
            name: name.into(),
            revision,
            documents,
            pinned_document_ids,
            properties,
            resources,
        };
        manifest.check_invariants()?;
        Ok(manifest)
    }

    /// Notebook vuoto appena creato, revisione 0.
    pub fn empty(id: CanonicalAggregateId, name: impl Into<String>) -> Result<Self, NotebookError> {
        Self::new(
            id,
            name,
            0,
            Vec::new(),
            Vec::new(),
            NotebookProperties::default(),
            Vec::new(),
        )
    }

    fn check_invariants(&self) -> Result<(), NotebookError> {
        ensure(!is_blank(&self.name), "Notebook name cannot be blank")?;
        ensure(self.revision >= 0, "Notebook revision cannot be negative")?;
        ensure(
            self.documents.iter().all(|d| !is_blank(&d.id)),
            "Notebook contains a blank document id",
        )?;
        ensure(
            all_distinct(self.documents.iter().map(|d| &d.id)),
            "Notebook contains duplicate document ids",
        )?;
        ensure(
            all_distinct(self.pinned_document_ids.iter()),
            "Notebook contains duplicate pins",
        )?;
        ensure(
            self.pinned_document_ids
                .iter()
                .all(|pin| self.documents.iter().any(|d| &d.id == pin)),
            "Notebook contains orphan pins",
        )?;
        ensure(
            all_distinct(self.resources.iter().map(|r| r.id())),
            "Notebook contains duplicate resource ids",
        )
    }

    pub fn id(&self) -> &CanonicalAggregateId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn documents(&self) -> &[RagDocument] {
        &self.documents
    }

    pub fn pinned_document_ids(&self) -> &[String] {
        &self.pinned_document_ids
    }

    pub fn properties(&self) -> &NotebookProperties {
        &self.properties
    }

    pub fn resources(&self) -> &[NotebookResourceRef] {
        &self.resources
    }
}

impl Revisioned for RagNotebookManifest {
    fn revision(&self) -> i64 {
        self.revision
    }
}

/// Identità deterministica di compatibilità, riservata ai manifest legacy senza id.
pub fn stable_notebook_id(name: &str) -> CanonicalAggregateId {
    let seed = format!("qb-rag-notebook\u{1f}{name}");
    CanonicalAggregateId::parse(name_uuid_from_bytes(seed.as_bytes()).to_string())
        .expect("UUID::to_string is canonical lower-case")
}

fn all_distinct<T: Eq + Hash>(values: impl Iterator<Item = T>) -> bool {
    let mut seen = HashSet::new();
    values.into_iter().all(|value| seen.insert(value))
}

/// Forma persistita del manifest, nell'ordine dei campi della data class Kotlin.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRecord {
    #[serde(default = "current_version")]
    version: String,
    id: CanonicalAggregateId,
    name: String,
    #[serde(default)]
    revision: i64,
    #[serde(default)]
    documents: Vec<RagDocument>,
    #[serde(default)]
    pinned_document_ids: Vec<String>,
    #[serde(default)]
    properties: NotebookProperties,
    #[serde(default)]
    resources: Vec<NotebookResourceRef>,
}

fn current_version() -> String {
    RAG_NOTEBOOK_MANIFEST_VERSION.to_owned()
}

impl TryFrom<ManifestRecord> for RagNotebookManifest {
    type Error = NotebookError;

    fn try_from(r: ManifestRecord) -> Result<Self, Self::Error> {
        if r.version != RAG_NOTEBOOK_MANIFEST_VERSION {
            return Err(NotebookError::UnsupportedVersion(r.version));
        }
        Self::new(
            r.id,
            r.name,
            r.revision,
            r.documents,
            r.pinned_document_ids,
            r.properties,
            r.resources,
        )
    }
}

impl From<RagNotebookManifest> for ManifestRecord {
    fn from(m: RagNotebookManifest) -> Self {
        Self {
            version: current_version(),
            id: m.id,
            name: m.name,
            revision: m.revision,
            documents: m.documents,
            pinned_document_ids: m.pinned_document_ids,
            properties: m.properties,
            resources: m.resources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> CanonicalAggregateId {
        CanonicalAggregateId::parse("4cad446f-a8b1-4c2f-a762-968052b4592c").unwrap()
    }

    #[test]
    fn orphan_pin_is_rejected() {
        let result = RagNotebookManifest::new(
            id(),
            "n",
            0,
            vec![],
            vec!["missing".into()],
            NotebookProperties::default(),
            vec![],
        );
        assert_eq!(
            result,
            Err(NotebookError::Invariant("Notebook contains orphan pins"))
        );
    }

    #[test]
    fn pinned_documents_must_exist() {
        let manifest = RagNotebookManifest::new(
            id(),
            "n",
            1,
            vec![RagDocument::new("a", "testo")],
            vec!["a".into()],
            NotebookProperties::default(),
            vec![],
        )
        .unwrap();
        assert_eq!(manifest.pinned_document_ids(), ["a"]);
        assert_eq!(manifest.revision(), 1);
    }
}
