//! # kubirag-domain
//!
//! Aggregati e value object di KubiRAG, puri: nessun I/O, nessun runtime asincrono.
//! Un modulo per bounded context:
//! - [`notebook`] **Notebook Governance**: l'aggregato autoritativo da cui tutte le altre viste
//!   (retrieval, wiki, libro) sono proiezioni; comandi, invarianti e domain event.
//! - [`concept`] **Concept Knowledge**: forma canonica e validazione del grafo dei concetti.
//! - [`config`]: la configurazione dell'applicazione (`qb-rag.json`) e i suoi vincoli.

pub mod concept;
pub mod config;
pub mod notebook;
