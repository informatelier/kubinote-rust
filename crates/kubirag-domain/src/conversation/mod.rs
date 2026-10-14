//! **Conversation**: le sessioni di chat di un utente, gli assistenti (ruoli) e la stima
//! dell'occupazione della finestra di contesto.
//!
//! Il modello è quello di `ChatSessionManager`, `RoleEntry`/`RoleLoader` e
//! `ContextUsageEstimator` della versione Kotlin, senza I/O: la persistenza
//! (`qb-chat-web-<utente>.json`, `qb-roles.json`) è una porta di `kubirag-app`.

mod roles;
mod sessions;
mod usage;

pub use roles::{parse_at_mention, RoleEntry, Roles};
pub use sessions::{
    sanitize_user_id, ChatMessage, ChatSessions, SessionMetadata, DEFAULT_SESSION, MAX_MESSAGES,
};
pub use usage::{estimate_tokens, ContextUsage};
