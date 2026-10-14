use kubirag_domain::conversation::{ChatSessions, Roles};

/// Persistenza delle sessioni di un utente (`qb-chat-web-<utente>.json`).
pub trait ChatSessionStore: Send + Sync {
    /// `None` se l'utente non ha ancora un archivio; un file illeggibile è un errore.
    fn load(&self, user: &str) -> Result<Option<ChatSessions>, String>;
    fn save(&self, user: &str, sessions: &ChatSessions) -> Result<(), String>;
}

/// Persistenza degli assistenti (`qb-roles.json`).
pub trait RoleStore: Send + Sync {
    fn load(&self) -> Result<Roles, String>;
    fn save(&self, roles: &Roles) -> Result<(), String>;
}
