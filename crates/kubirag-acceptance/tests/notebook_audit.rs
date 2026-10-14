//! O1-KR1: il journal di audit del notebook (domain event) è scritto e letto come in Kotlin.

use std::fs;

use kubi_kernel::governance::AtomicAuditJournal;
use kubi_kernel::json::JsonDialect;
use kubirag_acceptance::golden;
use kubirag_domain::notebook::RagNotebookAuditEvent;
use serde_json::Value;

fn journal_events(relative: &str) -> Vec<RagNotebookAuditEvent> {
    let root = golden::json(relative);
    serde_json::from_value(root["events"].clone()).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

#[test]
fn appending_the_golden_events_writes_the_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("journal.json");
    let journal = AtomicAuditJournal::new(&file, "RAG Notebook", RagNotebookAuditEvent::event_id)
        .with_lock_target(dir.path().join("locks/rag-audit"))
        .with_staging_dir(dir.path().join("staging"));
    for event in journal_events("notebook/audit/notebook-audit.json") {
        journal.append(event).unwrap();
    }
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        golden::read("notebook/audit/notebook-audit.json")
    );
}

#[test]
fn the_real_kubirag_journal_round_trips() {
    let relative = "notebook/audit/real-kubirag-audit.json";
    let events = journal_events(relative);
    assert!(!events.is_empty());
    let envelope = serde_json::json!({ "version": "1", "events": events });
    let written = JsonDialect::GOVERNANCE.pretty(&envelope).unwrap();
    let original: Value = golden::json(relative);
    assert_eq!(
        serde_json::from_str::<Value>(&written).unwrap(),
        original,
        "semantica diversa"
    );
    assert_eq!(written, golden::read(relative), "byte diversi");
}
