use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use kubirag_domain::conversation::{sanitize_user_id, ChatSessions};

use super::ChatSessionStore;

/// `ChatSessionManagerFactory`: un archivio di sessioni per utente, letto dal disco al primo
/// accesso e poi tenuto in memoria. Gli id utente sono normalizzati come in Kotlin.
pub struct ChatSessionDirectory {
    store: Arc<dyn ChatSessionStore>,
    users: Mutex<HashMap<String, ChatSessions>>,
}

impl ChatSessionDirectory {
    pub fn new(store: Arc<dyn ChatSessionStore>) -> Self {
        Self {
            store,
            users: Mutex::new(HashMap::new()),
        }
    }

    /// Esegue `action` sull'archivio dell'utente. Con `persist` l'archivio è salvato dopo
    /// l'azione (`storeSessions`), anche se l'azione non ha cambiato nulla.
    pub fn with_user<T>(
        &self,
        user: &str,
        persist: bool,
        action: impl FnOnce(&mut ChatSessions) -> T,
    ) -> Result<T, String> {
        let user = sanitize_user_id(user);
        let mut users = self.lock()?;
        if !users.contains_key(&user) {
            // `restoreSessions`: un file illeggibile vale come archivio nuovo.
            let restored = self.store.load(&user).ok().flatten().unwrap_or_default();
            users.insert(user.clone(), restored);
        }
        let sessions = users
            .get_mut(&user)
            .expect("user archive was just inserted");
        let result = action(sessions);
        if persist {
            self.store.save(&user, sessions)?;
        }
        Ok(result)
    }

    fn lock(&self) -> Result<MutexGuard<'_, HashMap<String, ChatSessions>>, String> {
        self.users
            .lock()
            .map_err(|_| "chat session directory lock poisoned".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct MemoryStore {
        saved: StdMutex<Vec<(String, ChatSessions)>>,
    }

    impl ChatSessionStore for MemoryStore {
        fn load(&self, _: &str) -> Result<Option<ChatSessions>, String> {
            Err("file illeggibile".into())
        }

        fn save(&self, user: &str, sessions: &ChatSessions) -> Result<(), String> {
            self.saved
                .lock()
                .unwrap()
                .push((user.to_owned(), sessions.clone()));
            Ok(())
        }
    }

    #[test]
    fn users_are_sanitized_and_unreadable_archives_start_fresh() {
        let store = Arc::new(MemoryStore::default());
        let directory = ChatSessionDirectory::new(store.clone());
        let created = directory
            .with_user("a.b", true, |s| s.create("x", 10))
            .unwrap();
        assert!(created);
        let count = directory
            .with_user("a_b", false, |s| s.sessions().len())
            .unwrap();
        assert_eq!(
            count, 2,
            "default + x, stesso archivio dopo la normalizzazione"
        );
        assert_eq!(store.saved.lock().unwrap()[0].0, "a_b");
    }
}
