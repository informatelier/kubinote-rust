use std::path::Path;

use kubi_kernel::governance::{ModelIssue, ModelIssueSeverity};
use kubi_kernel::store::sha256_file;
use kubirag_domain::notebook::{is_bound, is_remote, RagNotebookManifest, ResourceOwnership};

/// `scanNotebookManifestIssues`: risorse registrate mancanti o cambiate su disco.
/// Senza `store_dir` i riferimenti relativi non si possono verificare e vengono saltati.
pub fn scan_notebook_issues(
    manifest: &RagNotebookManifest,
    store_dir: Option<&Path>,
) -> Vec<ModelIssue> {
    manifest
        .resources()
        .iter()
        .filter(|r| !is_remote(r.path()) && !is_bound(r.path()))
        .filter_map(|resource| {
            let raw = Path::new(resource.path());
            let file = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                store_dir?.join(resource.path())
            };
            let managed = resource.ownership() == ResourceOwnership::Managed;
            if !file.is_file() {
                let note = resource.path().replace('\\', "/").starts_with("notes/");
                return Some(issue(
                    if managed && note {
                        "RAG_MANAGED_NOTE_MISSING"
                    } else {
                        "RAG_RESOURCE_MISSING"
                    },
                    if managed {
                        ModelIssueSeverity::Error
                    } else {
                        ModelIssueSeverity::Warning
                    },
                    format!("resources/{}/path", resource.id()),
                    format!("Registered resource is missing: {}", resource.path()),
                ));
            }
            let expected = resource.sha256()?;
            let changed = sha256_file(&file).map_or(true, |actual| actual != expected);
            changed.then(|| {
                issue(
                    "RAG_EXTERNAL_RESOURCE_DRIFT",
                    ModelIssueSeverity::Warning,
                    format!("resources/{}/sha256", resource.id()),
                    format!("Registered resource changed: {}", resource.path()),
                )
            })
        })
        .collect()
}

fn issue(code: &str, severity: ModelIssueSeverity, path: String, message: String) -> ModelIssue {
    ModelIssue::new(code, severity, path, message, true).expect("issue fields are not blank")
}
