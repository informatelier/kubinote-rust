use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use kubi_kernel::governance::{AggregateIdentityConflict, ModelIssue, RevisionConflictError};
use kubi_kernel::store::{sha256_file, SidecarLock};
use kubirag_app::notebook::{ManifestReadResult, ManifestRepository, ManifestWriteError};
use kubirag_domain::notebook::{
    NotebookResourceRef, PortableLocator, RagNotebookManifest, ResourceOwnership, PATH_METADATA,
};

use crate::paths::{canonical, relative_invariant, safe_file_name};
use crate::{scan_notebook_issues, ManifestCodec};

/// Repository del manifest autoritativo (`RagNotebookManifestRepository`).
///
/// - `qb_root`: radice autoritativa e collaborativa (`_qb/`), contiene `notes/<store>/`;
/// - `local_root`: radice locale del processo (`.qb/`), con lock e staging.
///
/// Il compare-and-set prende il lock sidecar `<local>/locks/<id>.lock`, lo stesso della JVM:
/// un processo Kotlin e uno Rust sullo stesso notebook non scrivono mai insieme.
pub struct JsonManifestRepository {
    qb_root: PathBuf,
    local_root: PathBuf,
}

impl JsonManifestRepository {
    pub fn new(qb_root: impl Into<PathBuf>, local_root: impl Into<PathBuf>) -> Self {
        Self {
            qb_root: qb_root.into(),
            local_root: local_root.into(),
        }
    }

    pub fn store_dir(&self, name: &str) -> PathBuf {
        self.qb_root.join("notes").join(name)
    }

    pub fn manifest_file(&self, name: &str) -> PathBuf {
        self.store_dir(name).join(format!("{name}-manifest.json"))
    }

    fn read_text(&self, name: &str) -> ManifestReadResult {
        match fs::read_to_string(self.manifest_file(name)) {
            Ok(text) => ManifestCodec::classify(Some(&text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ManifestReadResult::Missing
            }
            Err(error) => ManifestReadResult::Invalid(error.to_string()),
        }
    }

    fn write_locked(
        &self,
        manifest: &RagNotebookManifest,
        expected: i64,
    ) -> Result<(), ManifestWriteError> {
        if expected < 0 {
            return Err(rejected("Expected revision cannot be negative"));
        }
        let lock_target = self.local_root.join("locks").join(manifest.id().as_str());
        let _lock = SidecarLock::acquire(&lock_target).map_err(failure)?;
        let current =
            match self.read_text(manifest.name()) {
                ManifestReadResult::Current(current) => Some(current),
                ManifestReadResult::Missing => None,
                ManifestReadResult::LegacyMissingIdentity { .. } => return Err(rejected(
                    "Legacy Notebook manifest must be migrated before compare-and-set persistence",
                )),
                ManifestReadResult::UnsupportedVersion(version) => {
                    return Err(rejected(format!(
                        "Unsupported Notebook manifest version {version}"
                    )))
                }
                ManifestReadResult::Invalid(diagnostic) => {
                    return Err(rejected(format!("Invalid Notebook manifest: {diagnostic}")))
                }
            };
        let actual = current.as_ref().map_or(0, |c| c.revision());
        if actual != expected {
            return Err(RevisionConflictError {
                label: "RAG Notebook",
                expected,
                actual: Some(actual),
            }
            .into());
        }
        if let Some(current) = current.filter(|c| c.id() != manifest.id()) {
            return Err(identity_conflict(&current, manifest));
        }
        if manifest.revision() != expected + 1 {
            return Err(rejected("Notebook revision must advance by one"));
        }
        self.write_unlocked(manifest)
    }

    /// Normalizza per la portabilità, poi scrive con staging, rilettura e rename atomico.
    fn write_unlocked(&self, manifest: &RagNotebookManifest) -> Result<(), ManifestWriteError> {
        let store = self.store_dir(manifest.name());
        let mut portable = Vec::new();
        for resource in manifest.resources() {
            portable.extend(portable_resource(&store, resource)?);
        }
        let paths_by_document: IndexMap<&str, &str> = portable
            .iter()
            .filter_map(|r| r.source_document_id().map(|id| (id, r.path())))
            .collect();
        let mut draft = manifest.draft();
        for document in &mut draft.documents {
            match paths_by_document.get(document.id.as_str()) {
                Some(path) => {
                    document
                        .metadata
                        .insert(PATH_METADATA.to_owned(), (*path).to_owned());
                }
                None => {
                    document.metadata.shift_remove(PATH_METADATA);
                }
            }
        }
        draft.resources = portable;
        let validated = draft.build().map_err(|e| rejected(e.to_string()))?;
        let text = ManifestCodec::encode(&validated).map_err(failure)?;
        write_verified(
            &self.manifest_file(manifest.name()),
            &self.local_root.join("staging"),
            "qb-rag-manifest",
            &text,
            |written| match ManifestCodec::classify(Some(written)) {
                ManifestReadResult::Current(decoded) if *decoded == validated => Ok(()),
                _ => Err(rejected("Staged Notebook manifest did not round-trip")),
            },
        )
    }
}

/// `write_verified` del kernel vuole un errore costruibile da `io::Error`.
struct WriteFailure(ManifestWriteError);

impl From<std::io::Error> for WriteFailure {
    fn from(error: std::io::Error) -> Self {
        Self(failure(error))
    }
}

fn write_verified(
    target: &Path,
    staging: &Path,
    prefix: &str,
    text: &str,
    verify: impl FnOnce(&str) -> Result<(), ManifestWriteError>,
) -> Result<(), ManifestWriteError> {
    kubi_kernel::store::write_verified(target, staging, prefix, text, |written| {
        verify(written).map_err(WriteFailure)
    })
    .map_err(|WriteFailure(error)| error)
}

impl ManifestRepository for JsonManifestRepository {
    fn read(&self, name: &str) -> ManifestReadResult {
        self.read_text(name)
    }

    fn write(
        &self,
        manifest: &RagNotebookManifest,
        expected_revision: Option<i64>,
    ) -> Result<(), ManifestWriteError> {
        match expected_revision {
            Some(expected) => self.write_locked(manifest, expected),
            None => {
                if let ManifestReadResult::Current(current) = self.read_text(manifest.name()) {
                    if current.id() != manifest.id() {
                        return Err(identity_conflict(&current, manifest));
                    }
                }
                self.write_unlocked(manifest)
            }
        }
    }

    fn issues(&self, manifest: &RagNotebookManifest) -> Vec<ModelIssue> {
        scan_notebook_issues(manifest, Some(&self.store_dir(manifest.name())))
    }
}

/// `portableResource`: remoti e `bound://` restano esterni; un file locale diventa una
/// risorsa gestita relativa allo store (copiata in `resources/<id>/` se sta fuori), con il suo
/// SHA-256. Un file esterno sparito viene tolto; uno gestito sparito blocca la scrittura.
fn portable_resource(
    store_dir: &Path,
    resource: &NotebookResourceRef,
) -> Result<Option<NotebookResourceRef>, ManifestWriteError> {
    let rebuild = |path: String, ownership: ResourceOwnership, sha256: Option<String>| {
        NotebookResourceRef::new(resource.id(), path, ownership, sha256, resource.kind())
            .map_err(|e| rejected(e.to_string()))
    };
    let path =
        match PortableLocator::classify(resource.path()).map_err(|e| rejected(e.to_string()))? {
            PortableLocator::Remote(path) | PortableLocator::Bound(path) => {
                return rebuild(
                    path,
                    ResourceOwnership::External,
                    resource.sha256().map(str::to_owned),
                )
                .map(Some)
            }
            PortableLocator::Local(path) => path,
        };
    let store = canonical(store_dir);
    let raw = Path::new(&path);
    let source = if raw.is_absolute() {
        canonical(raw)
    } else {
        canonical(&store.join(&path))
    };
    if !source.is_file() {
        if resource.ownership() == ResourceOwnership::Managed {
            let code = if path.replace('\\', "/").starts_with("notes/") {
                "RAG_MANAGED_NOTE_MISSING"
            } else {
                "RAG_MANAGED_RESOURCE_MISSING"
            };
            return Err(rejected(format!("{code}: {path}")));
        }
        return Ok(None);
    }
    let relative = relative_invariant(&source, &store).unwrap_or_else(|| {
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("resources/{}/{}", resource.id(), safe_file_name(&name))
    });
    if relative
        .split('/')
        .any(|segment| segment == ".." || segment.trim().is_empty())
    {
        return Err(rejected("Invalid managed resource path"));
    }
    let target = store.join(&relative);
    if source != canonical(&target) {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(failure)?;
        }
        fs::copy(&source, &target).map_err(failure)?;
    }
    let sha256 = sha256_file(&target).map_err(failure)?;
    rebuild(relative, ResourceOwnership::Managed, Some(sha256)).map(Some)
}

fn identity_conflict(
    current: &RagNotebookManifest,
    manifest: &RagNotebookManifest,
) -> ManifestWriteError {
    AggregateIdentityConflict {
        expected_id: current.id().as_str().to_owned(),
        actual_id: manifest.id().as_str().to_owned(),
    }
    .into()
}

fn rejected(message: impl Into<String>) -> ManifestWriteError {
    ManifestWriteError::Rejected(message.into())
}

fn failure(error: impl std::fmt::Display) -> ManifestWriteError {
    ManifestWriteError::Failure(error.to_string())
}
