//! Punteggi BM25 contro `LuceneSearchEngine` (golden generati dalla JVM): stesso ordine,
//! stessi documenti e gli stessi punteggi `float`, bit per bit.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use kubirag_app::retrieval::SearchEngine;
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::{DocumentChunker, SearchOptions};
use kubirag_retrieval::LuceneCompatibleSearchEngine;
use serde_json::Value;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/golden/retrieval")
}

fn golden(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(golden_dir().join(name)).unwrap()).unwrap()
}

fn markdown_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            markdown_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "md") {
            found.push(path);
        }
    }
}

/// I chunk del corpus come li indicizza il generatore: id = percorso relativo.
fn corpus_chunks() -> Vec<RagDocument> {
    let corpus = golden_dir().join("corpus");
    let mut files = Vec::new();
    markdown_files(&corpus, &mut files);
    let relative = |p: &Path| {
        p.strip_prefix(&corpus)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_str().unwrap())
            .collect::<Vec<_>>()
            .join("/")
    };
    files.sort_by_key(|p| relative(p));
    files
        .iter()
        .flat_map(|file| {
            let path = relative(file);
            let mut document =
                RagDocument::new(path.clone(), std::fs::read_to_string(file).unwrap());
            document.metadata = IndexMap::from([
                (
                    "filename".to_owned(),
                    file.file_name().unwrap().to_str().unwrap().to_owned(),
                ),
                ("path".to_owned(), path),
            ]);
            DocumentChunker::default().chunk(&document)
        })
        .collect()
}

fn engine(documents: &[RagDocument], max_results: usize) -> LuceneCompatibleSearchEngine {
    let mut engine = LuceneCompatibleSearchEngine::new();
    engine.configure(SearchOptions {
        max_results,
        ..SearchOptions::default()
    });
    engine.index_all(documents);
    engine
}

/// Confronta ogni query: espansione, documenti nello stesso ordine, punteggi identici.
fn assert_matches(engine: &mut LuceneCompatibleSearchEngine, queries: &Value) {
    let mut divergences = Vec::new();
    for query in queries.as_array().unwrap() {
        let id = query["id"].as_str().unwrap();
        let text = query["text"].as_str().unwrap();
        let expanded = query["expandedQuery"].as_str().unwrap();
        if engine.expanded_query(text) != expanded {
            divergences.push(format!(
                "{id}: espansione {:?} vs {expanded:?}",
                engine.expanded_query(text)
            ));
        }
        let expected: Vec<(String, f32)> = query["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["documentId"].as_str().unwrap().to_owned(),
                    r["score"].as_f64().unwrap() as f32,
                )
            })
            .collect();
        let actual: Vec<(String, f32)> = engine
            .search(text, 1000)
            .into_iter()
            .map(|r| (r.document_id, r.score))
            .collect();
        let same_order = expected
            .iter()
            .map(|e| &e.0)
            .eq(actual.iter().map(|a| &a.0));
        let worst = expected
            .iter()
            .zip(&actual)
            .map(|(e, a)| ((e.1 - a.1) / e.1.max(f32::MIN_POSITIVE)).abs())
            .fold(0f32, f32::max);
        if !same_order || worst > 0.0 {
            divergences.push(format!(
                "{id} {text:?}: ordine {} errore relativo massimo {worst:e}\n  atteso {expected:?}\n  ottenuto {actual:?}",
                if same_order { "uguale" } else { "diverso" }
            ));
        }
    }
    assert!(divergences.is_empty(), "{}", divergences.join("\n"));
}

#[test]
fn corpus_scores_match_lucene() {
    let golden = golden("lucene-scores.json");
    assert_matches(&mut engine(&corpus_chunks(), 1000), &golden["queries"]);
}

#[test]
fn synthetic_scores_match_lucene() {
    let golden = golden("lucene-synthetic.json");
    let documents: Vec<RagDocument> = golden["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            let mut document =
                RagDocument::new(d["id"].as_str().unwrap(), d["content"].as_str().unwrap());
            document.metadata = d["metadata"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect();
            document
        })
        .collect();
    assert_matches(&mut engine(&documents, 100), &golden["queries"]);
}
