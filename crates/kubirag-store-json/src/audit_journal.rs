use std::path::{Path, PathBuf};

use kubi_kernel::governance::AtomicAuditJournal;
use kubirag_app::notebook::NotebookAuditLog;
use kubirag_domain::notebook::RagNotebookAuditEvent;

/// Journal dei domain event del notebook (`RagNotebookAuditJournal`): lock sidecar su
/// `<local>/locks/rag-audit`, staging in `<local>/staging`, envelope `{version, events}`.
pub struct JsonNotebookAuditJournal {
    journal: AtomicAuditJournal<RagNotebookAuditEvent>,
    file: PathBuf,
}

impl JsonNotebookAuditJournal {
    /// `local_dir` è la radice locale (non collaborativa) per lock e staging; Kotlin usa per
    /// default la cartella del journal.
    pub fn new(file: impl Into<PathBuf>, local_dir: &Path) -> Self {
        let file = file.into();
        let journal =
            AtomicAuditJournal::new(&file, "RAG Notebook", RagNotebookAuditEvent::event_id)
                .with_lock_target(local_dir.join("locks/rag-audit"))
                .with_staging_dir(local_dir.join("staging"));
        Self { journal, file }
    }

    /// Journal predefinito di un notebook: `<store>/<name>-audit.json`.
    pub fn for_store(store_dir: &Path, name: &str) -> Self {
        Self::new(store_dir.join(format!("{name}-audit.json")), store_dir)
    }

    pub fn file(&self) -> &Path {
        &self.file
    }
}

impl NotebookAuditLog for JsonNotebookAuditJournal {
    fn append(&self, event: RagNotebookAuditEvent) -> Result<(), String> {
        self.journal.append(event).map_err(|e| e.to_string())
    }

    fn events(&self) -> Result<Vec<RagNotebookAuditEvent>, String> {
        self.journal.events().map_err(|e| e.to_string())
    }
}
