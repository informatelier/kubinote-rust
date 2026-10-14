//! Conversation e Studio: sessioni di chat, assistenti e modelli di documento, negli stessi
//! file della versione Kotlin (`Json { prettyPrint }`: 4 spazi, valori di default omessi).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use indexmap::IndexMap;
use kubi_kernel::json::JsonDialect;
use kubirag_app::conversation::{ChatSessionStore, RoleStore};
use kubirag_app::studio::TemplateLibrary;
use kubirag_domain::conversation::{ChatMessage, ChatSessions, RoleEntry, Roles};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub const ROLES_FILENAME: &str = "qb-roles.json";
pub const TEMPLATES_FILENAME: &str = "qb-templates.json";

fn read_optional(file: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", file.display())),
    }
}

/// `File.writeText`, come Kotlin: questi file non sono governati (nessun lock, nessun CAS).
fn write(file: &Path, tree: &Value) -> Result<(), String> {
    let text = JsonDialect::CONFIG
        .pretty(tree)
        .map_err(|e| e.to_string())?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(file, text).map_err(|e| format!("{}: {e}", file.display()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageRecord {
    content: String,
    timestamp: i64,
    #[serde(default = "user_role")]
    role: String,
}

fn user_role() -> String {
    "user".to_owned()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionsRecord {
    current_session: String,
    sessions: IndexMap<String, Vec<MessageRecord>>,
    #[serde(default)]
    titles: IndexMap<String, String>,
}

/// `ChatSessionManager.storeSessions` / `restoreSessions` su `.qb/qb-chat-web-<utente>.json`.
pub struct JsonChatSessionStore {
    local_root: PathBuf,
}

impl JsonChatSessionStore {
    pub fn new(local_root: impl Into<PathBuf>) -> Self {
        Self {
            local_root: local_root.into(),
        }
    }

    pub fn file(&self, user: &str) -> PathBuf {
        self.local_root.join(format!("qb-chat-web-{user}.json"))
    }

    pub fn encode(sessions: &ChatSessions) -> Value {
        let mut root = Map::new();
        root.insert("currentSession".into(), json!(sessions.current()));
        let encoded: Map<String, Value> = sessions
            .sessions()
            .iter()
            .map(|(id, messages)| {
                let messages = messages
                    .iter()
                    .map(|m| {
                        let mut message = Map::new();
                        message.insert("content".into(), json!(m.content));
                        message.insert("timestamp".into(), json!(m.timestamp));
                        if m.role != "user" {
                            message.insert("role".into(), json!(m.role));
                        }
                        Value::Object(message)
                    })
                    .collect();
                (id.clone(), Value::Array(messages))
            })
            .collect();
        root.insert("sessions".into(), Value::Object(encoded));
        if !sessions.titles().is_empty() {
            root.insert("titles".into(), json!(sessions.titles()));
        }
        Value::Object(root)
    }

    pub fn decode(text: &str) -> Result<ChatSessions, String> {
        let record: SessionsRecord = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let sessions = record
            .sessions
            .into_iter()
            .map(|(id, messages)| {
                let messages = messages
                    .into_iter()
                    .map(|m| ChatMessage {
                        content: m.content,
                        timestamp: m.timestamp,
                        role: m.role,
                    })
                    .collect();
                (id, messages)
            })
            .collect();
        Ok(ChatSessions::restore(
            record.current_session,
            sessions,
            record.titles,
        ))
    }
}

impl ChatSessionStore for JsonChatSessionStore {
    fn load(&self, user: &str) -> Result<Option<ChatSessions>, String> {
        read_optional(&self.file(user))?
            .map(|text| Self::decode(&text))
            .transpose()
    }

    fn save(&self, user: &str, sessions: &ChatSessions) -> Result<(), String> {
        write(&self.file(user), &Self::encode(sessions))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleRecord {
    role_name: String,
    role_prompt: String,
    #[serde(default)]
    file_patterns: Vec<String>,
    #[serde(default)]
    voice_name: Option<String>,
}

/// `CtrlRoles` / `RoleLoader` su `.qb/qb-roles.json`.
pub struct JsonRoleStore {
    file: PathBuf,
}

impl JsonRoleStore {
    pub fn new(local_root: impl AsRef<Path>) -> Self {
        Self {
            file: local_root.as_ref().join(ROLES_FILENAME),
        }
    }

    pub fn encode(roles: &Roles) -> Value {
        let encoded: Map<String, Value> = roles
            .iter()
            .map(|(name, role)| {
                let mut entry = Map::new();
                entry.insert("roleName".into(), json!(role.role_name));
                entry.insert("rolePrompt".into(), json!(role.role_prompt));
                if !role.file_patterns.is_empty() {
                    entry.insert("filePatterns".into(), json!(role.file_patterns));
                }
                if let Some(voice) = &role.voice_name {
                    entry.insert("voiceName".into(), json!(voice));
                }
                (name.clone(), Value::Object(entry))
            })
            .collect();
        Value::Object(encoded)
    }

    pub fn decode(text: &str) -> Result<Roles, String> {
        if text.trim().is_empty() {
            return Ok(Roles::new());
        }
        let records: IndexMap<String, RoleRecord> =
            serde_json::from_str(text).map_err(|e| e.to_string())?;
        Ok(records
            .into_iter()
            .map(|(name, r)| {
                let role = RoleEntry {
                    role_name: r.role_name,
                    role_prompt: r.role_prompt,
                    file_patterns: r.file_patterns,
                    voice_name: r.voice_name,
                };
                (name, role)
            })
            .collect())
    }
}

impl RoleStore for JsonRoleStore {
    fn load(&self) -> Result<Roles, String> {
        read_optional(&self.file)?.map_or_else(|| Ok(Roles::new()), |text| Self::decode(&text))
    }

    fn save(&self, roles: &Roles) -> Result<(), String> {
        write(&self.file, &Self::encode(roles))
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct TemplateRecord {
    name: String,
    path: String,
}

/// `DocTemplateManager`: il registro `.qb/qb-templates.json` più la libreria condivisa
/// `~/.qb/template/` (`*.md`, `*.mm`). Il registro in memoria accumula quanto letto, come
/// l'istanza unica del server Kotlin, e viene riscritto intero a ogni salvataggio.
pub struct FsTemplateLibrary {
    registry: PathBuf,
    library: PathBuf,
    templates: Mutex<IndexMap<String, TemplateRecord>>,
}

impl FsTemplateLibrary {
    pub fn new(local_root: impl AsRef<Path>, home: impl AsRef<Path>) -> Self {
        Self {
            registry: local_root.as_ref().join(TEMPLATES_FILENAME),
            library: home.as_ref().join(".qb").join("template"),
            templates: Mutex::new(IndexMap::new()),
        }
    }

    fn restore(&self, templates: &mut IndexMap<String, TemplateRecord>) -> Result<(), String> {
        if let Some(text) = read_optional(&self.registry)? {
            let stored: IndexMap<String, TemplateRecord> =
                serde_json::from_str(&text).map_err(|e| e.to_string())?;
            *templates = stored;
        }
        let Ok(entries) = fs::read_dir(&self.library) else {
            return Ok(());
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.ends_with(".md") || n.ends_with(".mm"))
            })
            .collect();
        files.sort();
        for file in files {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let path = absolute(&file);
            templates.insert(name.clone(), TemplateRecord { name, path });
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, IndexMap<String, TemplateRecord>>, String> {
        self.templates
            .lock()
            .map_err(|_| "template library lock poisoned".to_owned())
    }
}

fn absolute(path: &Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

impl TemplateLibrary for FsTemplateLibrary {
    fn names(&self) -> Result<Vec<String>, String> {
        let mut templates = self.lock()?;
        self.restore(&mut templates)?;
        let mut names: Vec<String> = templates.values().map(|t| t.name.clone()).collect();
        names.sort_by_key(|name| name.to_lowercase());
        Ok(names)
    }

    fn content(&self, name: &str) -> Result<Option<String>, String> {
        let mut templates = self.lock()?;
        self.restore(&mut templates)?;
        let Some(record) = templates.get(name) else {
            return Ok(None);
        };
        match fs::read_to_string(&record.path) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{}: {e}", record.path)),
        }
    }

    fn save(&self, name: &str, content: &str) -> Result<String, String> {
        let safe = name.trim().replace(['/', '\\'], "_");
        let safe = safe.strip_suffix(".md").unwrap_or(&safe);
        fs::create_dir_all(&self.library)
            .map_err(|e| format!("{}: {e}", self.library.display()))?;
        let file = self.library.join(format!("{safe}.md"));
        fs::write(&file, content).map_err(|e| format!("{}: {e}", file.display()))?;
        let file_name = format!("{safe}.md");
        let mut templates = self.lock()?;
        templates.insert(
            file_name.clone(),
            TemplateRecord {
                name: file_name.clone(),
                path: absolute(&file),
            },
        );
        let tree = serde_json::to_value(&*templates).map_err(|e| e.to_string())?;
        write(&self.registry, &tree)?;
        Ok(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_are_written_like_kotlinx_without_defaults() {
        let mut sessions = ChatSessions::default();
        sessions.add_message("default", "user", "ciao", 1);
        sessions.add_message("default", "assistant", "salve", 2);
        let text = JsonDialect::CONFIG
            .pretty(&JsonChatSessionStore::encode(&sessions))
            .unwrap();
        assert_eq!(
            text,
            "{\n    \"currentSession\": \"default\",\n    \"sessions\": {\n        \"default\": [\n            {\n                \"content\": \"ciao\",\n                \"timestamp\": 1\n            },\n            {\n                \"content\": \"salve\",\n                \"timestamp\": 2,\n                \"role\": \"assistant\"\n            }\n        ]\n    }\n}"
        );
        assert_eq!(JsonChatSessionStore::decode(&text).unwrap(), sessions);
    }

    #[test]
    fn roles_omit_empty_patterns_and_missing_voices() {
        let mut roles = Roles::new();
        roles.insert(
            "poeta".into(),
            RoleEntry::new("poeta", "Rispondi in versi.", vec![], Some("paola".into())),
        );
        roles.insert(
            "revisore".into(),
            RoleEntry::new("revisore", "Sii severo.", vec!["*.md".into()], None),
        );
        let tree = JsonRoleStore::encode(&roles);
        assert_eq!(
            tree,
            json!({
                "poeta": {"roleName": "poeta", "rolePrompt": "Rispondi in versi.", "voiceName": "paola"},
                "revisore": {"roleName": "revisore", "rolePrompt": "Sii severo.", "filePatterns": ["*.md"]}
            })
        );
        assert_eq!(JsonRoleStore::decode(&tree.to_string()).unwrap(), roles);
        assert!(JsonRoleStore::decode("  ").unwrap().is_empty());
    }

    #[test]
    fn templates_are_saved_in_the_home_library_and_registered() {
        let local = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let library = FsTemplateLibrary::new(local.path(), home.path());
        assert_eq!(
            library.save(" verbale/riunione.md ", "# V").unwrap(),
            "verbale_riunione.md"
        );
        fs::write(home.path().join(".qb/template/Alfa.mm"), "x").unwrap();
        assert_eq!(library.names().unwrap(), ["Alfa.mm", "verbale_riunione.md"]);
        let registry = fs::read_to_string(local.path().join(TEMPLATES_FILENAME)).unwrap();
        assert!(registry.contains("\"verbale_riunione.md\": {\n        \"name\""));
    }
}
