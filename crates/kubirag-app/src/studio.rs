//! **Notes & Studio**: per ora solo la libreria dei modelli dell'editor markboard; note,
//! quiz e report arrivano in F5.

/// Libreria dei modelli di documento (`DocTemplateManager`).
pub trait TemplateLibrary: Send + Sync {
    /// Nomi dei modelli, in ordine alfabetico senza distinzione di maiuscole.
    fn names(&self) -> Result<Vec<String>, String>;
    /// Il testo di un modello registrato il cui file esiste ancora (`GET /templates/{name}`).
    fn content(&self, name: &str) -> Result<Option<String>, String>;
    /// Salva un modello e restituisce il nome con cui è registrato.
    fn save(&self, name: &str, content: &str) -> Result<String, String>;
}
