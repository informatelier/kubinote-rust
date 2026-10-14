use indexmap::IndexMap;
use kubi_kernel::identity::ResourceRef;
use serde::{Deserialize, Serialize};

/// Documento (o chunk) del notebook. I metadati conservano l'ordine di inserimento,
/// come la `LinkedHashMap` di kotlinx, perché l'ordine finisce nei byte del manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RagDocument {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub metadata: IndexMap<String, String>,
    /// Identità stabile della sorgente; `id` può identificare una rappresentazione derivata.
    #[serde(default)]
    pub resource_ref: Option<ResourceRef>,
}

impl RagDocument {
    pub fn new(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            metadata: IndexMap::new(),
            resource_ref: None,
        }
    }
}

/// Metadati editoriali del notebook (autore, copertina, colore…), indipendenti da `RagConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotebookProperties {
    pub author: String,
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub cover_image: String,
    pub background_image: String,
    pub accent_color: String,
}
