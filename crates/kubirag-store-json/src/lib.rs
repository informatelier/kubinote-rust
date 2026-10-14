//! # kubirag-store-json
//!
//! Adattatori di persistenza compatibili con i file della versione Kotlin (strangler fig:
//! le due versioni leggono e scrivono gli stessi notebook). Implementano le porte di
//! `kubirag-app` con il filesystem:
//!
//! | Bounded context | File | Adattatore |
//! |---|---|---|
//! | Notebook Governance | `_qb/notes/<store>/<store>-manifest.json` | [`JsonManifestRepository`] |
//! | Notebook Governance | journal dei domain event | [`JsonNotebookAuditJournal`] |
//! | Notebook Governance | file sorgente e asset | [`FsResourceFingerprints`] |
//! | Concept Knowledge | `_qb/qb-concepts.json` | [`JsonConceptGraphRepository`] |
//! | Configurazione | `.qb/qb-rag.json` | [`RagConfigCodec`] |

mod audit_journal;
mod concept_graph;
mod config_codec;
mod fingerprints;
mod issues;
mod manifest_codec;
mod manifest_repository;
mod paths;

pub use audit_journal::JsonNotebookAuditJournal;
pub use concept_graph::{
    ConceptGraphStoreError, JsonConceptGraphRepository, CONCEPT_GRAPH_FILENAME,
};
pub use config_codec::{RagConfigCodec, RAG_CONFIG_FILENAME};
pub use fingerprints::FsResourceFingerprints;
pub use issues::scan_notebook_issues;
pub use kubirag_app::notebook::ManifestReadResult;
pub use manifest_codec::ManifestCodec;
pub use manifest_repository::JsonManifestRepository;
