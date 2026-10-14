//! F1, O1-KR2: nessuna sequenza di comandi viola le invarianti dell'aggregato notebook.
//!
//! 10.000 sequenze casuali di comandi (con id che si ripetono apposta, così pin doppi,
//! documenti mancanti e sostituzioni in conflitto capitano spesso). Dopo ogni comando:
//! - un rifiuto lascia lo stato com'era (l'aggregato è immutabile, lo stato non avanza);
//! - un successo avanza la revisione di uno e rispetta tutte le invarianti;
//! - lo stato sopravvive al round-trip JSON;
//! - ogni risorsa `document-source:<id>` punta a un documento esistente.

use indexmap::IndexSet;
use kubi_kernel::identity::CanonicalAggregateId;
use kubirag_domain::notebook::{
    NoFingerprints, NotebookProperties, RagDocument, RagNotebookCommand, RagNotebookManifest,
    ResourceOwnership, PATH_METADATA,
};
use proptest::prelude::*;

const IDS: [&str; 5] = ["a", "b", "c", "d", "e"];

fn doc_id() -> impl Strategy<Value = String> {
    prop::sample::select(&IDS[..]).prop_map(str::to_owned)
}

fn ids() -> impl Strategy<Value = IndexSet<String>> {
    prop::collection::vec(doc_id(), 0..4).prop_map(|ids| ids.into_iter().collect())
}

fn ownership() -> impl Strategy<Value = ResourceOwnership> {
    prop_oneof![
        Just(ResourceOwnership::Managed),
        Just(ResourceOwnership::External)
    ]
}

fn document() -> impl Strategy<Value = RagDocument> {
    let path = prop_oneof![
        Just(None),
        Just(Some("notes/x.md".to_owned())),
        Just(Some("https://informatelier.it/page".to_owned())),
        Just(Some("bound://glossary/terms.md".to_owned())),
        Just(Some("   ".to_owned())),
    ];
    (doc_id(), "[a-zà🎉 ]{0,8}", path).prop_map(|(id, content, path)| {
        let mut document = RagDocument::new(id, content);
        if let Some(path) = path {
            document.metadata.insert(PATH_METADATA.to_owned(), path);
        }
        document
    })
}

fn properties() -> impl Strategy<Value = NotebookProperties> {
    (
        prop_oneof![
            Just(""),
            Just("assets/cover.png"),
            Just("https://x.y/c.png")
        ],
        prop_oneof![Just(""), Just("assets/bg.jpg")],
    )
        .prop_map(|(cover, background)| NotebookProperties {
            author: "Nicola".into(),
            cover_image: cover.into(),
            background_image: background.into(),
            ..NotebookProperties::default()
        })
}

fn command() -> impl Strategy<Value = RagNotebookCommand> {
    use RagNotebookCommand as C;
    prop_oneof![
        (document(), ownership()).prop_map(|(document, ownership)| C::AddDocument {
            document,
            ownership
        }),
        (document(), ownership()).prop_map(|(document, ownership)| C::AddAndPinDocument {
            document,
            ownership
        }),
        (prop::collection::vec(document(), 0..3), ids(), ownership()).prop_map(
            |(documents, pin_ids, ownership)| C::AddDocumentsAndSetPins {
                documents,
                pin_ids,
                ownership
            }
        ),
        (document(), ownership()).prop_map(|(document, ownership)| C::ReplaceDocument {
            document,
            ownership
        }),
        (
            ids(),
            prop::collection::vec(document(), 0..3),
            ownership(),
            any::<bool>()
        )
            .prop_map(
                |(remove_ids, documents, ownership, preserve_replacement_pins)| {
                    C::ReplaceDocuments {
                        remove_ids,
                        documents,
                        ownership,
                        preserve_replacement_pins,
                    }
                }
            ),
        doc_id().prop_map(|id| C::RemoveDocument { id }),
        ids().prop_map(|ids| C::RemoveDocuments { ids }),
        doc_id().prop_map(|id| C::Pin { id }),
        doc_id().prop_map(|id| C::Unpin { id }),
        (ids(), any::<bool>()).prop_map(|(ids, pinned)| C::SetPins { ids, pinned }),
        properties().prop_map(|properties| C::UpdateProperties { properties }),
        document().prop_map(|document| C::RefreshExternal { document }),
    ]
}

fn assert_invariants(manifest: &RagNotebookManifest) -> Result<(), TestCaseError> {
    let ids: Vec<&str> = manifest.documents().iter().map(|d| d.id.as_str()).collect();
    let distinct: IndexSet<&str> = ids.iter().copied().collect();
    prop_assert_eq!(distinct.len(), ids.len(), "documenti duplicati");
    let pins: IndexSet<&String> = manifest.pinned_document_ids().iter().collect();
    prop_assert_eq!(
        pins.len(),
        manifest.pinned_document_ids().len(),
        "pin duplicati"
    );
    for pin in manifest.pinned_document_ids() {
        prop_assert!(manifest.has_document(pin), "pin orfano {}", pin);
    }
    let resources: IndexSet<&str> = manifest.resources().iter().map(|r| r.id()).collect();
    prop_assert_eq!(
        resources.len(),
        manifest.resources().len(),
        "risorse duplicate"
    );
    for resource in manifest.resources() {
        if let Some(source) = resource.source_document_id() {
            prop_assert!(manifest.has_document(source), "risorsa orfana {}", source);
        }
    }
    let encoded = serde_json::to_string(manifest).unwrap();
    let decoded: RagNotebookManifest = serde_json::from_str(&encoded).unwrap();
    prop_assert_eq!(&decoded, manifest, "round-trip JSON");
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn no_command_sequence_breaks_the_aggregate(commands in prop::collection::vec(command(), 1..12)) {
        let id = CanonicalAggregateId::parse("4cad446f-a8b1-4c2f-a762-968052b4592c").unwrap();
        let mut state = RagNotebookManifest::empty(id, "proptest").unwrap();
        for command in &commands {
            match command.decide(&state, &NoFingerprints) {
                Ok(next) => {
                    prop_assert_eq!(next.revision(), state.revision() + 1);
                    prop_assert_eq!(next.id(), state.id());
                    prop_assert_eq!(next.name(), state.name());
                    assert_invariants(&next)?;
                    state = next;
                }
                Err(_) => assert_invariants(&state)?,
            }
        }
    }
}
