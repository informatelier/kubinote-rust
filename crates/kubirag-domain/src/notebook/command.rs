//! Comandi dell'aggregato notebook (il linguaggio dell'event storming): ogni comando produce,
//! dallo stato corrente, lo stato successivo con revisione `expected + 1`, oppure un rifiuto
//! con il messaggio che la versione Kotlin scrive nell'audit.
//!
//! La decisione è pura. L'unica informazione esterna, l'impronta SHA-256 dei file sorgente,
//! arriva dalla porta [`ResourceFingerprints`], implementata dall'adattatore di persistenza.

use indexmap::IndexSet;
use kubi_kernel::identity::name_uuid_from_bytes;

use super::{
    is_bound, NotebookError, NotebookProperties, NotebookResourceRef, RagDocument,
    RagNotebookCommandType, RagNotebookManifest, ResourceOwnership,
};

/// Chiave dei metadati che contiene il percorso sorgente (`DocumentMetaData.path`).
pub const PATH_METADATA: &str = "path";
const DOCUMENT_SOURCE: &str = "document-source:";
const NOTEBOOK_PROPERTY: &str = "notebook-property:";

/// Porta verso i file: impronta dei contenuti a cui puntano le risorse registrate.
pub trait ResourceFingerprints {
    /// SHA-256 del file sorgente di un documento, `None` se non è un file leggibile.
    /// Kotlin: `MANAGED` relativo → rispetto allo store; altrimenti il percorso così com'è.
    fn document_source(&self, path: &str, ownership: ResourceOwnership) -> Option<String>;
    /// SHA-256 di un asset del notebook (copertina, sfondo), relativo allo store.
    fn notebook_asset(&self, path: &str) -> Option<String>;
}

/// Nessun file raggiungibile: utile ai test del dominio e alle risorse solo remote.
pub struct NoFingerprints;

impl ResourceFingerprints for NoFingerprints {
    fn document_source(&self, _: &str, _: ResourceOwnership) -> Option<String> {
        None
    }

    fn notebook_asset(&self, _: &str) -> Option<String> {
        None
    }
}

/// Comando su un notebook. Gli insiemi di id conservano l'ordine di inserimento, come i
/// `Set` Kotlin (`LinkedHashSet`), perché l'ordine finisce nei pin del manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RagNotebookCommand {
    AddDocument {
        document: RagDocument,
        ownership: ResourceOwnership,
    },
    AddAndPinDocument {
        document: RagDocument,
        ownership: ResourceOwnership,
    },
    AddDocumentsAndSetPins {
        documents: Vec<RagDocument>,
        pin_ids: IndexSet<String>,
        ownership: ResourceOwnership,
    },
    ReplaceDocument {
        document: RagDocument,
        ownership: ResourceOwnership,
    },
    ReplaceDocuments {
        remove_ids: IndexSet<String>,
        documents: Vec<RagDocument>,
        ownership: ResourceOwnership,
        preserve_replacement_pins: bool,
    },
    RemoveDocument {
        id: String,
    },
    RemoveDocuments {
        ids: IndexSet<String>,
    },
    Pin {
        id: String,
    },
    Unpin {
        id: String,
    },
    SetPins {
        ids: IndexSet<String>,
        pinned: bool,
    },
    UpdateProperties {
        properties: NotebookProperties,
    },
    /// Nuovo contenuto di una sorgente esterna: è un `REPLACE_DOCUMENT` esplicito.
    RefreshExternal {
        document: RagDocument,
    },
    /// Riconciliazione con un candidato della stessa identità e dello stesso nome.
    Reconcile {
        candidate: Box<RagNotebookManifest>,
    },
}

impl RagNotebookCommand {
    pub fn command_type(&self) -> RagNotebookCommandType {
        use RagNotebookCommandType as T;
        match self {
            Self::AddDocument { .. } => T::AddDocument,
            Self::AddAndPinDocument { .. } => T::AddAndPinDocument,
            Self::AddDocumentsAndSetPins { .. } => T::AddAndPinDocuments,
            Self::ReplaceDocument { .. } | Self::RefreshExternal { .. } => T::ReplaceDocument,
            Self::ReplaceDocuments { .. } => T::ReplaceDocuments,
            Self::RemoveDocument { .. } => T::RemoveDocument,
            Self::RemoveDocuments { .. } => T::RemoveDocuments,
            Self::Pin { .. } => T::Pin,
            Self::Unpin { .. } => T::Unpin,
            Self::SetPins { .. } => T::SetPins,
            Self::UpdateProperties { .. } => T::UpdateProperties,
            Self::Reconcile { .. } => T::Reconcile,
        }
    }

    /// `targetId` dell'evento di audit.
    pub fn target_id(&self, current: &RagNotebookManifest) -> String {
        match self {
            Self::AddDocument { document, .. }
            | Self::AddAndPinDocument { document, .. }
            | Self::ReplaceDocument { document, .. }
            | Self::RefreshExternal { document } => document.id.clone(),
            Self::AddDocumentsAndSetPins { .. }
            | Self::ReplaceDocuments { .. }
            | Self::RemoveDocuments { .. } => "documents".to_owned(),
            Self::RemoveDocument { id } | Self::Pin { id } | Self::Unpin { id } => id.clone(),
            Self::SetPins { .. } => "pins".to_owned(),
            Self::UpdateProperties { .. } => current.name().to_owned(),
            Self::Reconcile { candidate } => candidate.id().as_str().to_owned(),
        }
    }

    /// Decide lo stato successivo. La revisione del risultato è `current.revision + 1`: il
    /// controllo della revisione attesa spetta al servizio applicativo, prima di chiamare qui.
    pub fn decide(
        &self,
        current: &RagNotebookManifest,
        fingerprints: &dyn ResourceFingerprints,
    ) -> Result<RagNotebookManifest, NotebookError> {
        let mut next = current.draft();
        match self {
            Self::AddDocument {
                document,
                ownership,
            } => {
                require_absent(current, &document.id)?;
                next.documents.push(document.clone());
                next.resources
                    .extend(document_resource(document, *ownership, fingerprints));
            }
            Self::AddAndPinDocument {
                document,
                ownership,
            } => {
                require_absent(current, &document.id)?;
                next.documents.push(document.clone());
                next.pinned_document_ids.push(document.id.clone());
                next.resources
                    .extend(document_resource(document, *ownership, fingerprints));
            }
            Self::AddDocumentsAndSetPins {
                documents,
                pin_ids,
                ownership,
            } => {
                require(all_distinct(documents), "Documents contain duplicate ids")?;
                require(
                    documents.iter().all(|d| !current.has_document(&d.id)),
                    "One or more document ids already exist",
                )?;
                next.documents.extend(documents.iter().cloned());
                require(
                    pin_ids
                        .iter()
                        .all(|id| next.documents.iter().any(|d| &d.id == id)),
                    "One or more pin ids do not reference a document",
                )?;
                next.pinned_document_ids
                    .extend(pin_ids.iter().filter(|id| !current.is_pinned(id)).cloned());
                next.resources.extend(
                    documents
                        .iter()
                        .filter_map(|d| document_resource(d, *ownership, fingerprints)),
                );
            }
            Self::ReplaceDocument {
                document,
                ownership,
            } => replace_document(
                current,
                &mut next.documents,
                &mut next.resources,
                document,
                *ownership,
                fingerprints,
            )?,
            Self::RefreshExternal { document } => replace_document(
                current,
                &mut next.documents,
                &mut next.resources,
                document,
                ResourceOwnership::External,
                fingerprints,
            )?,
            Self::ReplaceDocuments {
                remove_ids,
                documents,
                ownership,
                preserve_replacement_pins,
            } => {
                require(
                    remove_ids.iter().all(|id| current.has_document(id)),
                    "One or more documents selected for replacement do not exist",
                )?;
                require(
                    all_distinct(documents),
                    "Replacement contains duplicate document ids",
                )?;
                next.documents.retain(|d| !remove_ids.contains(&d.id));
                require(
                    documents
                        .iter()
                        .all(|r| !next.documents.iter().any(|d| d.id == r.id)),
                    "Replacement document id already exists",
                )?;
                next.documents.extend(documents.iter().cloned());
                next.pinned_document_ids.retain(|id| {
                    !remove_ids.contains(id)
                        || (*preserve_replacement_pins && documents.iter().any(|d| &d.id == id))
                });
                next.resources
                    .retain(|r| !remove_ids.iter().any(|id| is_source_of(r, id)));
                next.resources.extend(
                    documents
                        .iter()
                        .filter_map(|d| document_resource(d, *ownership, fingerprints)),
                );
            }
            Self::RemoveDocument { id } => {
                require_present(current, id)?;
                next.documents.retain(|d| &d.id != id);
                next.pinned_document_ids.retain(|pin| pin != id);
                next.resources.retain(|r| !is_source_of(r, id));
            }
            Self::RemoveDocuments { ids } => {
                require(
                    ids.iter().all(|id| current.has_document(id)),
                    "One or more documents selected for removal do not exist",
                )?;
                next.documents.retain(|d| !ids.contains(&d.id));
                next.pinned_document_ids.retain(|pin| !ids.contains(pin));
                next.resources
                    .retain(|r| !ids.iter().any(|id| is_source_of(r, id)));
            }
            Self::Pin { id } => {
                require_present(current, id)?;
                if current.is_pinned(id) {
                    return Err(rejected(format!("Document [{id}] is already pinned")));
                }
                next.pinned_document_ids.push(id.clone());
            }
            Self::Unpin { id } => {
                if !current.is_pinned(id) {
                    return Err(rejected(format!("Document [{id}] is not pinned")));
                }
                next.pinned_document_ids.retain(|pin| pin != id);
            }
            Self::SetPins { ids, pinned } => {
                require(
                    ids.iter().all(|id| current.has_document(id)),
                    "One or more documents selected for pinning do not exist",
                )?;
                if *pinned {
                    next.pinned_document_ids
                        .extend(ids.iter().filter(|id| !current.is_pinned(id)).cloned());
                } else {
                    next.pinned_document_ids.retain(|pin| !ids.contains(pin));
                }
            }
            Self::UpdateProperties { properties } => {
                next.properties = properties.clone();
                next.resources
                    .retain(|r| !r.kind().starts_with(NOTEBOOK_PROPERTY));
                next.resources
                    .extend(property_resources(current.name(), properties, fingerprints));
            }
            Self::Reconcile { candidate } => {
                if candidate.id() != current.id() {
                    return Err(rejected("RAG Notebook identity cannot change"));
                }
                if candidate.name() != current.name() {
                    return Err(rejected(
                        "RAG Notebook reconciliation cannot move authority",
                    ));
                }
                next = candidate.draft();
            }
        }
        // Prima `copy(...)` con la revisione corrente (invarianti), poi `copy(revision + 1)`.
        next.revision = current.revision();
        let evolved = next.build()?;
        let mut advanced = evolved.draft();
        advanced.revision = current.revision() + 1;
        advanced.build()
    }
}

fn replace_document(
    current: &RagNotebookManifest,
    documents: &mut [RagDocument],
    resources: &mut Vec<NotebookResourceRef>,
    document: &RagDocument,
    ownership: ResourceOwnership,
    fingerprints: &dyn ResourceFingerprints,
) -> Result<(), NotebookError> {
    if !current.has_document(&document.id) {
        return Err(rejected(format!("Document [{}] not found", document.id)));
    }
    for slot in documents.iter_mut().filter(|d| d.id == document.id) {
        *slot = document.clone();
    }
    resources.retain(|r| !is_source_of(r, &document.id));
    resources.extend(document_resource(document, ownership, fingerprints));
    Ok(())
}

/// Risorsa `document-source:<docId>` per un documento che dichiara `metadata.path`.
/// L'id è deterministico: `nameUUIDFromBytes("resource\u001f<docId>\u001f<path>")`.
pub fn document_resource(
    document: &RagDocument,
    ownership: ResourceOwnership,
    fingerprints: &dyn ResourceFingerprints,
) -> Option<NotebookResourceRef> {
    let path = document
        .metadata
        .get(PATH_METADATA)
        .filter(|path| !path.trim().is_empty())?;
    let sha256 = if is_bound(path) {
        None
    } else {
        fingerprints.document_source(path, ownership)
    };
    let id = name_uuid_from_bytes(format!("resource\u{1f}{}\u{1f}{path}", document.id).as_bytes());
    let kind = format!("{DOCUMENT_SOURCE}{}", document.id);
    Some(
        NotebookResourceRef::new(id.to_string(), path.clone(), ownership, sha256, kind)
            .expect("id, path and kind are not blank"),
    )
}

/// Risorse `notebook-property:cover|background` per le immagini delle proprietà.
pub fn property_resources(
    notebook_name: &str,
    properties: &NotebookProperties,
    fingerprints: &dyn ResourceFingerprints,
) -> Vec<NotebookResourceRef> {
    [
        ("cover", &properties.cover_image),
        ("background", &properties.background_image),
    ]
    .into_iter()
    .filter(|(_, reference)| !reference.trim().is_empty())
    .map(|(property, reference)| {
        // Qui il confronto è sensibile alle maiuscole, come in Kotlin.
        let external = reference.starts_with("http://") || reference.starts_with("https://");
        let seed =
            format!("notebook-property\u{1f}{notebook_name}\u{1f}{property}\u{1f}{reference}");
        NotebookResourceRef::new(
            name_uuid_from_bytes(seed.as_bytes()).to_string(),
            reference.clone(),
            if external {
                ResourceOwnership::External
            } else {
                ResourceOwnership::Managed
            },
            (!external)
                .then(|| fingerprints.notebook_asset(reference))
                .flatten(),
            format!("{NOTEBOOK_PROPERTY}{property}"),
        )
        .expect("id, path and kind are not blank")
    })
    .collect()
}

fn is_source_of(resource: &NotebookResourceRef, document_id: &str) -> bool {
    resource.source_document_id() == Some(document_id)
}

fn all_distinct(documents: &[RagDocument]) -> bool {
    let ids: IndexSet<&str> = documents.iter().map(|d| d.id.as_str()).collect();
    ids.len() == documents.len()
}

fn require_absent(current: &RagNotebookManifest, id: &str) -> Result<(), NotebookError> {
    if current.has_document(id) {
        Err(rejected(format!("Document [{id}] already exists")))
    } else {
        Ok(())
    }
}

fn require_present(current: &RagNotebookManifest, id: &str) -> Result<(), NotebookError> {
    if current.has_document(id) {
        Ok(())
    } else {
        Err(rejected(format!("Document [{id}] not found")))
    }
}

fn require(condition: bool, message: &str) -> Result<(), NotebookError> {
    if condition {
        Ok(())
    } else {
        Err(rejected(message))
    }
}

fn rejected(message: impl Into<String>) -> NotebookError {
    NotebookError::Rejected(message.into())
}
