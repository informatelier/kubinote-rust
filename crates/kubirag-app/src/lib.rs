//! # kubirag-app
//!
//! Casi d'uso di KubiRAG sopra il dominio puro. Le dipendenze verso il mondo esterno sono
//! **porte** (trait) implementate dagli adattatori (`kubirag-store-json`, poi HTTP, CLI…):
//! questo crate non fa I/O e non dipende dalla feature `fs` del kernel.
//!
//! Un modulo per bounded context: **Notebook Governance** (F1), **Retrieval** (F2) e
//! **Conversation** e la parte di **Notes & Studio** che serve al server HTTP (F3).

pub mod conversation;
pub mod notebook;
pub mod retrieval;
pub mod studio;
