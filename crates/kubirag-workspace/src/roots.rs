use std::path::{Path, PathBuf};

use kubi_kernel::workspace::QbWorkspaceRootResolver;
use kubirag_domain::config::RagConfig;
use kubirag_store_json::RagConfigCodec;

/// Le due radici del workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoots {
    /// `.qb/`: configurazione e proiezioni locali.
    pub local: PathBuf,
    /// `_qb/` (o quella di `qb-project.json`): manifest, journal, note.
    pub authority: PathBuf,
}

impl WorkspaceRoots {
    /// `KubiConfig.qbLocalPath()` e `qbAuthorityDir()`: `QB_ROOT_PATH` sceglie la radice
    /// locale (default `.qb/` nella directory corrente), `QB_AUTHORITY_PATH` quella di
    /// autorità; senza override l'autorità viene da `qb-project.json` del progetto.
    pub fn resolve(cwd: &Path, env: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let local = env("QB_ROOT_PATH").map_or_else(|| cwd.join(".qb"), |p| cwd.join(p));
        let authority = match env("QB_AUTHORITY_PATH") {
            Some(path) => cwd.join(path),
            None => {
                let project = QbWorkspaceRootResolver::discover_project_root(cwd);
                let roots = QbWorkspaceRootResolver::resolve(&project, false);
                if !roots.usable() {
                    return Err(roots
                        .issues
                        .iter()
                        .map(|i| format!("{}: {}", i.code(), i.message()))
                        .collect::<Vec<_>>()
                        .join("; "));
                }
                roots.authority_root
            }
        };
        Ok(Self { local, authority })
    }

    /// `<locale>/rag/notebooks`: lock e staging del manifest e del journal (`localDirectory`).
    pub fn notebook_runtime_dir(&self) -> PathBuf {
        self.local.join("rag").join("notebooks")
    }

    /// `<autorità>/governance/rag/notebooks`: un journal per notebook, per id
    /// (`semanticAuditDirectory`).
    pub fn audit_dir(&self) -> PathBuf {
        self.authority
            .join("governance")
            .join("rag")
            .join("notebooks")
    }
}

/// `qb-rag.json` della radice locale, o la configurazione di default.
pub fn read_config(roots: &WorkspaceRoots) -> Result<RagConfig, String> {
    Ok(RagConfigCodec::read(&roots.local)?.unwrap_or_default())
}
