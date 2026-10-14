//! **Conversation**: archivio delle sessioni di chat per utente e catalogo degli assistenti,
//! dietro porte di persistenza.

mod directory;
mod ports;

pub use directory::ChatSessionDirectory;
pub use ports::{ChatSessionStore, RoleStore};
