//! # kubirag-retrieval
//!
//! Adattatori del bounded context **Retrieval** di `kubirag-app`:
//! - [`LuceneCompatibleSearchEngine`]: BM25 con gli stessi analyzer, la stessa espansione della
//!   query e la stessa aritmetica `float` del `LuceneSearchEngine` Kotlin (Lucene 10.4), così
//!   i punteggi coincidono e la migrazione non cambia il ranking (O2-KR1);
//! - [`InMemoryVectorStore`]: coseno a scansione completa, come `InMemoryVectorStore`.

pub mod analysis;
mod engine;
mod expansion;
mod index;
mod vector;

pub use engine::LuceneCompatibleSearchEngine;
pub use expansion::expand_query;
pub use vector::InMemoryVectorStore;
