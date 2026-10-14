//! F1, O1-KR1: `.qb/qb-rag.json` e `_qb/qb-concepts.json` letti e scritti come dalla versione
//! Kotlin, sui golden generati dalla JVM e sui file reali del repository kubirag.

use std::fs;

use kubi_kernel::domain::concept::AuthorityConceptGraphFile;
use kubirag_acceptance::golden;
use kubirag_domain::concept::canonical;
use kubirag_store_json::{JsonConceptGraphRepository, RagConfigCodec};

#[test]
fn rag_config_round_trips_byte_exact() {
    let divergences = ["default", "custom", "real-kubirag"]
        .into_iter()
        .filter_map(|label| {
            let file = format!("config/{label}-qb-rag.json");
            let text = golden::read(&file);
            let config = RagConfigCodec::decode(&text).unwrap();
            if let Err(error) = config.validate() {
                return Some(format!("{file}: {error}"));
            }
            let written = RagConfigCodec::encode(&config).unwrap();
            (written != text).then(|| format!("{file}\n--- kotlin\n{text}\n--- rust\n{written}"))
        })
        .collect();
    golden::assert_no_divergence("qb-rag.json", divergences);
}

#[test]
fn rag_config_is_written_where_kotlin_reads_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = RagConfigCodec::decode(&golden::read("config/custom-qb-rag.json")).unwrap();
    RagConfigCodec::write(&config, dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("qb-rag.json")).unwrap(),
        golden::read("config/custom-qb-rag.json")
    );
    assert_eq!(RagConfigCodec::read(dir.path()).unwrap(), Some(config));
}

#[test]
fn concept_graph_is_written_in_canonical_form_like_kotlin() {
    let dir = tempfile::tempdir().unwrap();
    let input: AuthorityConceptGraphFile =
        serde_json::from_str(&golden::read("concept/rich-input.json")).unwrap();
    let repository = JsonConceptGraphRepository::new(dir.path());
    repository.write(input, None).unwrap();
    let written = fs::read_to_string(repository.graph_file()).unwrap();
    let expected = golden::read("concept/rich-concepts.json");
    assert!(
        written == expected,
        "--- kotlin\n{expected}\n--- rust\n{written}"
    );
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        1,
        "staging rimosso"
    );
}

#[test]
fn concept_graph_write_advances_revision_and_checks_expected() {
    let dir = tempfile::tempdir().unwrap();
    let repository = JsonConceptGraphRepository::new(dir.path());
    let input: AuthorityConceptGraphFile =
        serde_json::from_str(&golden::read("concept/rich-input.json")).unwrap();
    let first = repository.write(input.clone(), None).unwrap();
    assert_eq!(first.revision(), 4);
    assert!(repository.write(input.clone(), Some(3)).is_err());
    assert_eq!(repository.write(input, Some(4)).unwrap().revision(), 5);
}

#[test]
fn real_kubirag_concept_graph_round_trips() {
    let text = golden::read("concept/real-kubirag-concepts.json");
    let graph = JsonConceptGraphRepository::decode(&text, "real".as_ref()).unwrap();
    let written = JsonConceptGraphRepository::encode(&canonical(graph)).unwrap();
    assert!(written == text, "--- kotlin\n{text}\n--- rust\n{written}");
}
