//! F1, O1-KR1: lo stesso scenario di comandi, eseguito dal servizio applicativo Kotlin e da
//! quello Rust, produce lo stesso manifest (byte per byte), lo stesso journal di domain event
//! (byte per byte, con id evento e timestamp normalizzati) e gli stessi `OperationResult`.

use std::cell::Cell;
use std::fs;
use std::path::Path;

use indexmap::IndexSet;
use kubi_kernel::governance::{OperationChannel, OperationContext};
use kubi_kernel::identity::CanonicalAggregateId;
use kubirag_acceptance::golden;
use kubirag_app::notebook::{
    Clock, IdGenerator, ManifestRepository, NoProjection, RagNotebookApplicationService,
    RagNotebookManifestManager,
};
use kubirag_domain::notebook::{
    NotebookProperties, RagDocument, RagNotebookCommand, RagNotebookCommandType,
    RagNotebookManifest, ResourceOwnership,
};
use kubirag_store_json::{
    FsResourceFingerprints, JsonManifestRepository, JsonNotebookAuditJournal,
};
use serde_json::{json, Value};

/// Timestamp fisso del generatore (`SCENARIO_TIMESTAMP`).
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> String {
        "2026-10-14T00:00:00Z".to_owned()
    }
}

/// `event-1`, `event-2`, … come la normalizzazione del generatore.
#[derive(Default)]
struct SequentialIds(Cell<usize>);

impl IdGenerator for SequentialIds {
    fn new_id(&self) -> String {
        self.0.set(self.0.get() + 1);
        format!("event-{}", self.0.get())
    }
}

fn string(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} in {value}"))
        .to_owned()
}

fn strings(value: &Value, key: &str) -> IndexSet<String> {
    value[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

fn document(step: &Value) -> RagDocument {
    serde_json::from_value(step["document"].clone()).unwrap()
}

fn ownership(step: &Value) -> ResourceOwnership {
    serde_json::from_value(step.get("ownership").cloned().unwrap_or(json!("EXTERNAL"))).unwrap()
}

fn command(step: &Value) -> Option<RagNotebookCommand> {
    use RagNotebookCommand as C;
    Some(match string(step, "command").as_str() {
        "addDocument" => C::AddDocument {
            document: document(step),
            ownership: ownership(step),
        },
        "addAndPinDocument" => C::AddAndPinDocument {
            document: document(step),
            ownership: ownership(step),
        },
        "addDocumentsAndSetPins" => C::AddDocumentsAndSetPins {
            documents: serde_json::from_value(step["documents"].clone()).unwrap(),
            pin_ids: strings(step, "pinIds"),
            ownership: ownership(step),
        },
        "replaceDocument" => C::ReplaceDocument {
            document: document(step),
            ownership: ownership(step),
        },
        "replaceDocuments" => C::ReplaceDocuments {
            remove_ids: strings(step, "removeIds"),
            documents: serde_json::from_value(step["documents"].clone()).unwrap(),
            ownership: ownership(step),
            preserve_replacement_pins: step["preserveReplacementPins"].as_bool().unwrap_or(false),
        },
        "removeDocument" => C::RemoveDocument {
            id: string(step, "id"),
        },
        "removeDocuments" => C::RemoveDocuments {
            ids: strings(step, "ids"),
        },
        "pin" => C::Pin {
            id: string(step, "id"),
        },
        "unpin" => C::Unpin {
            id: string(step, "id"),
        },
        "setPins" => C::SetPins {
            ids: strings(step, "ids"),
            pinned: step["pinned"].as_bool().unwrap(),
        },
        "updateProperties" => C::UpdateProperties {
            properties: serde_json::from_value::<NotebookProperties>(step["properties"].clone())
                .unwrap(),
        },
        "refreshExternal" => C::RefreshExternal {
            document: document(step),
        },
        "reconcile" => C::Reconcile {
            candidate: Box::new(serde_json::from_value(step["candidate"].clone()).unwrap()),
        },
        _ => return None,
    })
}

fn run(scenario: &Value) -> Vec<String> {
    let name = string(scenario, "name");
    let notebook = &scenario["notebook"];
    let notebook_name = string(notebook, "name");
    let root = tempfile::tempdir().unwrap();
    let repository = JsonManifestRepository::new(root.path(), root.path().join("local"));
    let store = repository.store_dir(&notebook_name);
    for (path, content) in scenario["files"].as_object().unwrap() {
        let file = store.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, content.as_str().unwrap()).unwrap();
    }
    let id = CanonicalAggregateId::parse(string(notebook, "id")).unwrap();
    repository
        .write(
            &RagNotebookManifest::empty(id, &notebook_name).unwrap(),
            None,
        )
        .unwrap();
    let manager = RagNotebookManifestManager::load(
        Box::new(JsonManifestRepository::new(
            root.path(),
            root.path().join("local"),
        )),
        &notebook_name,
        false,
    )
    .unwrap();
    let mut service = RagNotebookApplicationService::new(
        manager,
        Box::new(JsonNotebookAuditJournal::for_store(&store, &notebook_name)),
        Box::new(NoProjection),
        Box::new(FsResourceFingerprints::new(&store)),
        Box::new(FixedClock),
        Box::new(SequentialIds::default()),
    );

    let mut results = Vec::new();
    for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        if let Some(file) = step["file"].as_str() {
            let file = store.join(file);
            if step["delete"].as_bool() == Some(true) {
                fs::remove_file(file).unwrap();
            } else {
                fs::write(file, string(step, "write")).unwrap();
            }
            continue;
        }
        let context = OperationContext::new(
            step["actor"].as_str().unwrap_or("nicola"),
            serde_json::from_value::<OperationChannel>(
                step.get("channel").cloned().unwrap_or(json!("CLI")),
            )
            .unwrap(),
            step["expectedRevision"].as_i64().unwrap(),
            Some(format!("corr-{index}")),
        )
        .unwrap();
        let result = match command(step) {
            Some(command) => service.execute(&command, &context),
            None => match string(step, "command").as_str() {
                "rebuildProjections" => service.rebuild_projections(&context),
                "undo" => service.undo_unavailable(&context),
                other => panic!("comando sconosciuto {other}"),
            },
        };
        let mut encoded = serde_json::Map::new();
        encoded.insert("step".into(), json!(index));
        encoded.extend(
            serde_json::to_value(&result)
                .unwrap()
                .as_object()
                .unwrap()
                .clone(),
        );
        results.push(Value::Object(encoded));
    }

    let expected = format!("notebook/scenarios/expected/{name}");
    let mut divergences = Vec::new();
    let compare = |label: &str, actual: String, divergences: &mut Vec<String>| {
        let golden = golden::read(&format!("{expected}/{label}"));
        if actual != golden {
            divergences.push(format!(
                "{name}/{label}\n--- kotlin\n{golden}\n--- rust\n{actual}"
            ));
        }
    };
    compare(
        "manifest.json",
        read(&repository.manifest_file(&notebook_name)),
        &mut divergences,
    );
    compare(
        "audit.json",
        read(&store.join(format!("{notebook_name}-audit.json"))),
        &mut divergences,
    );
    let golden_results = golden::json(&format!("{expected}/results.json"))["results"].clone();
    for (actual, kotlin) in results.iter().zip(golden_results.as_array().unwrap()) {
        if actual != kotlin {
            divergences.push(format!(
                "{name} risultato\n--- kotlin\n{kotlin}\n--- rust\n{actual}"
            ));
        }
    }
    if results.len() != golden_results.as_array().unwrap().len() {
        divergences.push(format!("{name}: numero di risultati diverso"));
    }
    divergences
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn command_scenarios_match_kotlin() {
    let scenarios = golden::json("notebook/scenarios/scenarios.json");
    let divergences = scenarios["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(run)
        .collect();
    golden::assert_no_divergence("scenari di comandi", divergences);
}

#[test]
fn command_types_print_their_kotlin_names() {
    assert_eq!(
        RagNotebookCommandType::AddAndPinDocuments.to_string(),
        "ADD_AND_PIN_DOCUMENTS"
    );
}
