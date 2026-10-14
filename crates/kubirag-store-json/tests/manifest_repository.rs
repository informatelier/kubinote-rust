//! Comportamento del repository del manifest oltre i golden: compare-and-set, identità,
//! portabilità dei file locali fuori dallo store.

use std::fs;

use indexmap::IndexMap;
use kubi_kernel::identity::CanonicalAggregateId;
use kubi_kernel::store::SidecarLock;
use kubirag_app::notebook::{ManifestReadResult, ManifestRepository, ManifestWriteError};
use kubirag_domain::notebook::{
    NotebookResourceRef, RagDocument, RagNotebookManifest, ResourceOwnership, PATH_METADATA,
};
use kubirag_store_json::JsonManifestRepository;

const ID: &str = "4cad446f-a8b1-4c2f-a762-968052b4592c";

fn manifest(
    revision: i64,
    documents: Vec<RagDocument>,
    resources: Vec<NotebookResourceRef>,
) -> RagNotebookManifest {
    let mut draft = RagNotebookManifest::empty(CanonicalAggregateId::parse(ID).unwrap(), "n")
        .unwrap()
        .draft();
    draft.revision = revision;
    draft.documents = documents;
    draft.resources = resources;
    draft.build().unwrap()
}

fn current(repository: &JsonManifestRepository) -> RagNotebookManifest {
    match repository.read("n") {
        ManifestReadResult::Current(manifest) => *manifest,
        other => panic!("{other:?}"),
    }
}

#[test]
fn compare_and_set_rejects_stale_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let repository = JsonManifestRepository::new(dir.path(), dir.path().join("local"));
    repository
        .write(&manifest(0, vec![], vec![]), None)
        .unwrap();
    repository
        .write(&manifest(1, vec![], vec![]), Some(0))
        .unwrap();

    let stale = repository
        .write(&manifest(1, vec![], vec![]), Some(0))
        .unwrap_err();
    assert_eq!(
        stale.to_string(),
        "RAG Notebook revision conflict: expected 0, actual 1"
    );
    assert!(matches!(stale, ManifestWriteError::RevisionConflict(_)));
    let skipping = repository
        .write(&manifest(3, vec![], vec![]), Some(1))
        .unwrap_err();
    assert_eq!(
        skipping.to_string(),
        "Notebook revision must advance by one"
    );
}

#[test]
fn another_identity_cannot_take_over_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let repository = JsonManifestRepository::new(dir.path(), dir.path().join("local"));
    repository
        .write(&manifest(0, vec![], vec![]), None)
        .unwrap();
    let other = RagNotebookManifest::empty(CanonicalAggregateId::random(), "n").unwrap();
    let error = repository.write(&other, None).unwrap_err();
    assert!(matches!(error, ManifestWriteError::IdentityConflict(_)));
    assert!(error.to_string().starts_with(&format!(
        "Aggregate identity conflict: expected {ID}, actual "
    )));
}

#[test]
fn the_write_waits_for_the_sidecar_lock_of_the_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let repository = JsonManifestRepository::new(dir.path(), dir.path().join("local"));
    repository
        .write(&manifest(0, vec![], vec![]), None)
        .unwrap();
    let lock_target = dir.path().join("local/locks").join(ID);
    let held = SidecarLock::acquire(&lock_target).unwrap();
    let writer = std::thread::spawn({
        let root = dir.path().to_path_buf();
        move || {
            JsonManifestRepository::new(&root, root.join("local"))
                .write(&manifest(1, vec![], vec![]), Some(0))
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(
        current(&repository).revision(),
        0,
        "scrittura sotto lock altrui"
    );
    drop(held);
    writer.join().unwrap().unwrap();
    assert_eq!(current(&repository).revision(), 1);
}

#[test]
fn local_files_outside_the_store_are_copied_in_as_managed_resources() {
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside/Città 🎉.md");
    fs::create_dir_all(outside.parent().unwrap()).unwrap();
    fs::write(&outside, "contenuto esterno").unwrap();
    let path = outside.to_string_lossy().into_owned();
    let mut document = RagDocument::new("doc", "x");
    document.metadata = IndexMap::from([(PATH_METADATA.to_owned(), path.clone())]);
    let resource = NotebookResourceRef::new(
        "r1",
        path,
        ResourceOwnership::External,
        None,
        "document-source:doc",
    )
    .unwrap();
    let repository = JsonManifestRepository::new(dir.path().join("_qb"), dir.path().join("local"));
    repository
        .write(&manifest(0, vec![document], vec![resource]), None)
        .unwrap();

    let written = current(&repository);
    let resource = &written.resources()[0];
    assert_eq!(resource.path(), "resources/r1/Citt____.md");
    assert_eq!(resource.ownership(), ResourceOwnership::Managed);
    assert_eq!(
        resource.sha256(),
        Some(kubi_kernel::checksum::sha256_hex(b"contenuto esterno").as_str())
    );
    assert_eq!(
        written.documents()[0]
            .metadata
            .get(PATH_METADATA)
            .map(String::as_str),
        Some("resources/r1/Citt____.md")
    );
    let copied = repository.store_dir("n").join("resources/r1/Citt____.md");
    assert_eq!(fs::read_to_string(copied).unwrap(), "contenuto esterno");
}

#[test]
fn vanished_external_files_are_dropped_and_vanished_managed_ones_block() {
    let dir = tempfile::tempdir().unwrap();
    let repository = JsonManifestRepository::new(dir.path(), dir.path().join("local"));
    let external = NotebookResourceRef::new(
        "r1",
        "gone/file.md",
        ResourceOwnership::External,
        None,
        "document-source:doc",
    )
    .unwrap();
    repository
        .write(
            &manifest(0, vec![RagDocument::new("doc", "x")], vec![external]),
            None,
        )
        .unwrap();
    assert!(current(&repository).resources().is_empty());

    let managed = NotebookResourceRef::new(
        "r2",
        "notes/gone.md",
        ResourceOwnership::Managed,
        None,
        "document-source:doc",
    )
    .unwrap();
    let error = repository
        .write(
            &manifest(1, vec![RagDocument::new("doc", "x")], vec![managed]),
            Some(0),
        )
        .unwrap_err();
    assert_eq!(error.to_string(), "RAG_MANAGED_NOTE_MISSING: notes/gone.md");
}
