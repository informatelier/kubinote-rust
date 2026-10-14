//! # kubirag-domain
//!
//! Aggregati e value object di KubiRAG, puri: nessun I/O, nessun runtime asincrono.
//! Un modulo per bounded context; in F0 c'è solo **Notebook Governance**, l'aggregato
//! autoritativo da cui tutte le altre viste (retrieval, wiki, libro) sono proiezioni.

pub mod notebook;
