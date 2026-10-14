use indexmap::IndexMap;

use crate::retrieval::text::take_utf16;

/// Sessione che esiste sempre in un archivio nuovo e verso cui si torna dopo una cancellazione.
pub const DEFAULT_SESSION: &str = "default";
/// Messaggi conservati per sessione: oltre, si scarta il più vecchio.
pub const MAX_MESSAGES: usize = 200;
/// Caratteri dell'ultimo messaggio mostrati nell'elenco delle sessioni.
const LAST_MESSAGE_CHARS: usize = 100;

/// `ChatSessionManagerFactory.sanitizeUserId`: tutto ciò che non è `[a-zA-Z0-9_-]` diventa `_`.
pub fn sanitize_user_id(user_id: &str) -> String {
    user_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Un messaggio della conversazione; `role` è `user` o `assistant`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub content: String,
    /// Millisecondi epoch (`System.currentTimeMillis()`).
    pub timestamp: i64,
    pub role: String,
}

/// Una riga dell'elenco delle sessioni (`getSessionMetadata`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub id: String,
    pub title: String,
    pub message_count: usize,
    pub last_message: String,
    pub updated_at: i64,
}

/// Le sessioni di chat di un utente (`ChatSessionManager`): aggregato con la sessione
/// corrente, i titoli e il limite di messaggi per sessione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSessions {
    current: String,
    sessions: IndexMap<String, Vec<ChatMessage>>,
    titles: IndexMap<String, String>,
}

impl Default for ChatSessions {
    /// Un archivio nuovo ha solo la sessione `default`.
    fn default() -> Self {
        Self {
            current: DEFAULT_SESSION.to_owned(),
            sessions: IndexMap::from([(DEFAULT_SESSION.to_owned(), Vec::new())]),
            titles: IndexMap::new(),
        }
    }
}

impl ChatSessions {
    /// Ricostruisce l'archivio da quanto persistito (`restoreSessions`), senza aggiungere
    /// la sessione `default` se il file non la contiene.
    pub fn restore(
        current: String,
        sessions: IndexMap<String, Vec<ChatMessage>>,
        titles: IndexMap<String, String>,
    ) -> Self {
        Self {
            current,
            sessions,
            titles,
        }
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    pub fn sessions(&self) -> &IndexMap<String, Vec<ChatMessage>> {
        &self.sessions
    }

    pub fn titles(&self) -> &IndexMap<String, String> {
        &self.titles
    }

    pub fn contains(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    /// I messaggi di una sessione, vuoti se non esiste.
    pub fn messages(&self, id: &str) -> &[ChatMessage] {
        self.sessions.get(id).map_or(&[], Vec::as_slice)
    }

    /// Crea una sessione vuota e la rende corrente. `false` se esiste già o se il limite
    /// di sessioni è raggiunto.
    pub fn create(&mut self, id: &str, limit: usize) -> bool {
        if self.sessions.contains_key(id) || self.sessions.len() >= limit {
            return false;
        }
        self.sessions.insert(id.to_owned(), Vec::new());
        self.current = id.to_owned();
        true
    }

    /// Copia i messaggi di `source` (anche inesistente: nessun messaggio) in una sessione nuova.
    pub fn branch(&mut self, source: &str, id: &str) -> Result<usize, String> {
        if self.sessions.contains_key(id) {
            return Err(format!("Session '{id}' already exists."));
        }
        let messages = self.messages(source).to_vec();
        let count = messages.len();
        self.sessions.insert(id.to_owned(), messages);
        Ok(count)
    }

    /// Aggiunge un messaggio, creando la sessione se manca e scartando il più vecchio oltre
    /// [`MAX_MESSAGES`].
    pub fn add_message(&mut self, id: &str, role: &str, content: &str, timestamp: i64) {
        let messages = self.sessions.entry(id.to_owned()).or_default();
        if messages.len() >= MAX_MESSAGES {
            messages.remove(0);
        }
        messages.push(ChatMessage {
            content: content.to_owned(),
            timestamp,
            role: role.to_owned(),
        });
    }

    pub fn rename(&mut self, id: &str, title: &str) -> bool {
        if !self.sessions.contains_key(id) {
            return false;
        }
        self.titles.insert(id.to_owned(), title.to_owned());
        true
    }

    /// Cancella una sessione; se era la corrente si torna a `default`, ricreandola se serve.
    pub fn delete(&mut self, id: &str) -> bool {
        if self.sessions.shift_remove(id).is_none() {
            return false;
        }
        self.titles.shift_remove(id);
        if self.current == id {
            self.current = DEFAULT_SESSION.to_owned();
            self.sessions.entry(DEFAULT_SESSION.to_owned()).or_default();
        }
        true
    }

    pub fn metadata(&self) -> Vec<SessionMetadata> {
        self.sessions
            .iter()
            .map(|(id, messages)| {
                let last = messages.last();
                SessionMetadata {
                    id: id.clone(),
                    title: self.titles.get(id).unwrap_or(id).clone(),
                    message_count: messages.len(),
                    last_message: last
                        .map(|m| take_utf16(&m.content, LAST_MESSAGE_CHARS).to_owned())
                        .unwrap_or_default(),
                    updated_at: last.map_or(0, |m| m.timestamp),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_ids_keep_only_safe_characters() {
        assert_eq!(
            sanitize_user_id("mario.rossi@example"),
            "mario_rossi_example"
        );
        assert_eq!(sanitize_user_id("a-b_C9"), "a-b_C9");
        assert_eq!(sanitize_user_id("città"), "citt_");
    }

    #[test]
    fn creating_a_session_makes_it_current_within_the_limit() {
        let mut sessions = ChatSessions::default();
        assert!(sessions.create("studio", 2));
        assert_eq!(sessions.current(), "studio");
        assert!(!sessions.create("studio", 10), "esiste già");
        assert!(!sessions.create("terza", 2), "limite raggiunto");
    }

    #[test]
    fn the_oldest_message_is_dropped_past_the_limit() {
        let mut sessions = ChatSessions::default();
        for i in 0..=MAX_MESSAGES {
            sessions.add_message("s", "user", &i.to_string(), i as i64);
        }
        let messages = sessions.messages("s");
        assert_eq!(messages.len(), MAX_MESSAGES);
        assert_eq!(messages[0].content, "1");
    }

    #[test]
    fn deleting_the_current_session_goes_back_to_default() {
        let mut sessions = ChatSessions::restore("x".into(), IndexMap::new(), IndexMap::new());
        sessions.add_message("x", "user", "ciao", 1);
        assert!(sessions.rename("x", "Titolo"));
        assert!(sessions.delete("x"));
        assert_eq!(sessions.current(), DEFAULT_SESSION);
        assert!(sessions.contains(DEFAULT_SESSION));
        assert!(sessions.titles().is_empty());
        assert!(!sessions.delete("x"));
    }

    #[test]
    fn a_branch_copies_the_messages_and_refuses_existing_targets() {
        let mut sessions = ChatSessions::default();
        sessions.add_message(DEFAULT_SESSION, "user", "uno", 1);
        assert_eq!(sessions.branch(DEFAULT_SESSION, "ramo"), Ok(1));
        assert_eq!(sessions.messages("ramo").len(), 1);
        assert_eq!(
            sessions.branch(DEFAULT_SESSION, "ramo"),
            Err("Session 'ramo' already exists.".into())
        );
        assert_eq!(sessions.branch("nessuna", "vuoto"), Ok(0));
    }

    #[test]
    fn metadata_truncates_the_last_message_to_100_utf16_units() {
        let mut sessions = ChatSessions::default();
        sessions.add_message(DEFAULT_SESSION, "assistant", &"é".repeat(150), 42);
        let row = &sessions.metadata()[0];
        assert_eq!(row.last_message.chars().count(), 100);
        assert_eq!(row.updated_at, 42);
        assert_eq!(row.title, DEFAULT_SESSION);
    }
}
