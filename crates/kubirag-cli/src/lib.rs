//! # kubirag-cli
//!
//! Interfaccia a riga di comando di KubiRAG in Rust (F2): la stessa sintassi e gli stessi
//! testi del `CommandTerminal` Kotlin, sopra gli stessi file del workspace (`.qb/`, `_qb/`).
//!
//! | Modulo | Ruolo |
//! |---|---|
//! | [`bootstrap`] | composition root: radici del workspace, configurazione, adattatori |
//! | [`terminal`] | dispatch, REPL, `-c`, `--script` |
//! | [`commands`] | `/load`, `/list`, `/pin`, `/unpin`, `/pinned`, `/stat`, `/embed`, `/lucene` |
//! | [`tokenizer`], [`format`] | parsing della riga e formati numerici della JVM |

pub mod bootstrap;
pub mod commands;
pub mod format;
pub mod terminal;
pub mod tokenizer;
