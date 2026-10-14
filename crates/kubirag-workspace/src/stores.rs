use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kubirag_app::notebook::{
    ManifestReadResult, ManifestRepository, RagNotebookApplicationService,
};
use kubirag_domain::notebook::RagNotebookManifest;
use kubirag_store_json::{JsonManifestRepository, JsonProjectionStore};

use crate::notebook::detached_service;
use crate::WorkspaceRoots;

/// Il catalogo dei notebook del workspace (`NotePaths` + `RagNotebookGovernanceGateway` per
/// quanto riguarda i notebook non attivi): elenco, creazione vuota, cancellazione.
#[derive(Debug, Clone)]
pub struct Workspace {
    roots: WorkspaceRoots,
}

impl Workspace {
    pub fn new(roots: WorkspaceRoots) -> Self {
        Self { roots }
    }

    pub fn roots(&self) -> &WorkspaceRoots {
        &self.roots
    }

    fn repository(&self) -> JsonManifestRepository {
        JsonManifestRepository::new(&self.roots.authority, self.roots.notebook_runtime_dir())
    }

    /// `NotePaths.authoritativeStoreNames`: le cartelle di `<autorità>/notes` che contengono
    /// il proprio `<nome>-manifest.json`, in ordine alfabetico.
    pub fn store_names(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.roots.authority.join("notes")) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| {
                self.roots
                    .authority
                    .join("notes")
                    .join(name)
                    .join(format!("{name}-manifest.json"))
                    .is_file()
            })
            .collect();
        names.sort();
        names
    }

    pub fn contains(&self, name: &str) -> bool {
        self.store_names().iter().any(|n| n == name)
    }

    /// La proiezione locale degli embedding esiste (`<locale>/notes/<n>/<n>-embeddings.json`).
    pub fn has_embeddings(&self, name: &str) -> bool {
        self.projections().embeddings_file(name).is_file()
    }

    pub fn projections(&self) -> JsonProjectionStore {
        JsonProjectionStore::new(&self.roots.local, &self.roots.authority)
    }

    /// `createEmpty`: un manifest vuoto per un nome valido che non ha né manifest né dati.
    pub fn create_empty(&self, name: &str) -> Result<RagNotebookManifest, String> {
        validate_notebook_name(name)?;
        let repository = self.repository();
        if !matches!(repository.read(name), ManifestReadResult::Missing) {
            return Err(format!("Notebook [{name}] already has a manifest"));
        }
        if repository.store_dir(name).exists() {
            return Err(format!(
                "Notebook [{name}] already has legacy or resource data; migrate it instead of overwriting it"
            ));
        }
        Ok(self.service(name, true)?.snapshot().clone())
    }

    /// Servizio governato di un notebook non attivo (`loadService(name, active = false)`).
    pub fn service(
        &self,
        name: &str,
        initialize_if_missing: bool,
    ) -> Result<RagNotebookApplicationService, String> {
        validate_notebook_name(name)?;
        detached_service(&self.roots, name, initialize_if_missing)
    }

    /// `CtrlStores.deleteStore`: proiezioni locali, binding di pubblicazione, dati
    /// autoritativi e journal del notebook.
    pub fn delete_store(&self, name: &str) -> Result<(), String> {
        validate_notebook_name(name)?;
        let repository = self.repository();
        let id = match repository.read(name) {
            ManifestReadResult::Current(manifest) => Some(manifest.id().as_str().to_owned()),
            _ => None,
        };
        remove_dir(&self.roots.local.join("notes").join(name))?;
        remove_file(&self.publish_bindings_file(name))?;
        remove_dir(&repository.store_dir(name))?;
        if let Some(id) = id {
            remove_file(&self.roots.audit_dir().join(format!("{id}.json")))?;
        }
        Ok(())
    }

    /// `PublicationBindingsStore`: `<locale>/rag/notebooks/publish-bindings/<n>.json`.
    pub fn publish_bindings_file(&self, name: &str) -> PathBuf {
        self.roots
            .notebook_runtime_dir()
            .join("publish-bindings")
            .join(format!("{name}.json"))
    }
}

/// `validateNotebookName`: non vuoto, senza separatori né `..`.
pub fn validate_notebook_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Active Notebook name cannot be blank".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("Invalid active Notebook name [{name}]"));
    }
    Ok(())
}

fn remove_dir(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(format!("{}: {e}", dir.display())),
        _ => Ok(()),
    }
}

fn remove_file(file: &Path) -> Result<(), String> {
    match fs::remove_file(file) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(format!("{}: {e}", file.display())),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let roots = WorkspaceRoots {
            local: dir.path().join(".qb"),
            authority: dir.path().join("_qb"),
        };
        (dir, Workspace::new(roots))
    }

    #[test]
    fn created_notebooks_are_listed_and_deleted_with_their_journal() {
        let (_dir, workspace) = workspace();
        let manifest = workspace.create_empty("beta").unwrap();
        workspace.create_empty("alfa").unwrap();
        assert_eq!(workspace.store_names(), ["alfa", "beta"]);
        let journal = workspace
            .roots()
            .audit_dir()
            .join(format!("{}.json", manifest.id().as_str()));
        fs::create_dir_all(journal.parent().unwrap()).unwrap();
        fs::write(&journal, "{}").unwrap();

        workspace.delete_store("beta").unwrap();
        assert_eq!(workspace.store_names(), ["alfa"]);
        assert!(!journal.exists());
    }

    #[test]
    fn creation_refuses_existing_data_and_unsafe_names() {
        let (_dir, workspace) = workspace();
        workspace.create_empty("alfa").unwrap();
        assert_eq!(
            workspace.create_empty("alfa").unwrap_err(),
            "Notebook [alfa] already has a manifest"
        );
        assert_eq!(
            workspace.create_empty("a/b").unwrap_err(),
            "Invalid active Notebook name [a/b]"
        );
    }
}
