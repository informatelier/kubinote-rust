//! Le viste derivate del Retrieval sono byte per byte quelle della JVM.

use std::path::PathBuf;

use kubirag_app::retrieval::ProjectionStore;
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::{Embedding, EmbeddingProjectionDescriptor};
use kubirag_store_json::JsonProjectionStore;

fn golden(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/golden/retrieval/projections")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn documents_round_trip_byte_for_byte() {
    let text = golden("kubirag-documents.json");
    let documents: Vec<RagDocument> = serde_json::from_str(&text).unwrap();
    assert_eq!(JsonProjectionStore::encode_documents(&documents), text);
}

#[test]
fn embeddings_round_trip_byte_for_byte() {
    let text = golden("kubirag-embeddings.json");
    let embeddings: Vec<Embedding> = serde_json::from_str(&text).unwrap();
    assert_eq!(embeddings[0].vector[3], f32::MAX);
    assert_eq!(JsonProjectionStore::encode_embeddings(&embeddings), text);
}

#[test]
fn profiles_round_trip_byte_for_byte() {
    for name in ["embedding-profile.json", "embedding-profile-building.json"] {
        let text = golden(name);
        let descriptor: EmbeddingProjectionDescriptor = serde_json::from_str(&text).unwrap();
        assert_eq!(
            JsonProjectionStore::encode_profile(&descriptor),
            text,
            "{name}"
        );
    }
}

#[test]
fn the_profile_lives_in_the_authority_and_vectors_stay_local() {
    let root = tempfile::tempdir().unwrap();
    let store = JsonProjectionStore::new(root.path().join(".qb"), root.path().join("_qb"));
    let profile = EmbeddingProjectionDescriptor::building("m", 768, 128, 1);
    store.write_profile("nb", &profile).unwrap();
    assert_eq!(store.read_profile("nb"), Some(profile));
    assert!(root
        .path()
        .join("_qb/notes/nb/embedding-profile.json")
        .is_file());
    assert!(!root
        .path()
        .join("_qb/notes/nb/embedding-profile.json.tmp")
        .exists());
    assert_eq!(store.load_embeddings("nb").unwrap(), vec![]);
    store.save_embeddings("nb", &[]).unwrap();
    assert!(root
        .path()
        .join(".qb/notes/nb/nb-embeddings.json")
        .is_file());
}
