//! # kubirag-workspace
//!
//! Composition root condiviso da `kubirag-cli` e `kubirag-http`: risolve le radici del
//! workspace come `KubiConfig`, legge la configurazione e collega gli adattatori (JSON,
//! motore BM25, vettori in memoria, LLM) ai servizi applicativi, con gli stessi percorsi della
//! versione Kotlin (`RagWorkspace`):
//!
//! | Cosa | Percorso |
//! |---|---|
//! | manifest | `<autorità>/notes/<store>/<store>-manifest.json` |
//! | lock del manifest | `<locale>/rag/notebooks/locks/<id>.lock` |
//! | journal dei domain event | `<autorità>/governance/rag/notebooks/<id>.json` |
//! | proiezioni locali | `<locale>/notes/<store>/` |

mod notebook;
mod roots;
mod stores;

pub use notebook::Notebook;
pub use roots::{read_config, WorkspaceRoots};
pub use stores::{validate_notebook_name, Workspace};

/// Notebook aperto quando non se ne sceglie uno (`NotePaths.DEFAULT_STORE`).
pub const DEFAULT_STORE: &str = "kubirag";
