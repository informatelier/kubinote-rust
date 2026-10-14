//! # kubirag-app
//!
//! Casi d'uso di KubiRAG sopra il dominio puro. Le dipendenze verso il mondo esterno sono
//! **porte** (trait) implementate dagli adattatori (`kubirag-store-json`, poi HTTP, CLI…):
//! questo crate non fa I/O e non dipende dalla feature `fs` del kernel.
//!
//! Un modulo per bounded context: **Notebook Governance** (F1) e **Retrieval** (F2).

pub mod notebook;
pub mod retrieval;
