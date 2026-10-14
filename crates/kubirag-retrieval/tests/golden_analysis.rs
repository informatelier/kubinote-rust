//! Gli analyzer e l'espansione della query contro i golden della JVM (Lucene 10.4).

use std::path::PathBuf;

use kubirag_retrieval::analysis::Analyzer;
use serde_json::Value;

fn golden(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/golden/retrieval")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn pairs(value: &Value) -> Vec<(String, u32)> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p[0].as_str().unwrap().to_owned(),
                p[1].as_u64().unwrap() as u32,
            )
        })
        .collect()
}

fn analyzed(analyzer: Analyzer, text: &str) -> Vec<(String, u32)> {
    analyzer
        .analyze(text)
        .into_iter()
        .map(|t| (t.text, t.position))
        .collect()
}

#[test]
fn samples_match_lucene_terms_and_positions() {
    let golden = golden("analyzers.json");
    let mut divergences = Vec::new();
    for sample in golden["samples"].as_array().unwrap() {
        let text = sample["text"].as_str().unwrap();
        for (analyzer, key) in [
            (Analyzer::Italian, "italian"),
            (Analyzer::English, "english"),
        ] {
            let expected = pairs(&sample[key]);
            let actual = analyzed(analyzer, text);
            if actual != expected {
                divergences.push(format!(
                    "{key} {text:?}\n  atteso {expected:?}\n  ottenuto {actual:?}"
                ));
            }
        }
    }
    assert!(divergences.is_empty(), "{}", divergences.join("\n"));
}

#[test]
fn corpus_chunks_match_lucene_terms() {
    let golden = golden("analyzers.json");
    let chunks = golden::chunk_texts();
    let mut divergences = Vec::new();
    for chunk in golden["chunks"].as_array().unwrap() {
        let id = chunk["id"].as_str().unwrap();
        let text = &chunks[id];
        for (analyzer, key) in [
            (Analyzer::Italian, "italian"),
            (Analyzer::English, "english"),
        ] {
            let expected: Vec<&str> = chunk[key]
                .as_str()
                .unwrap()
                .split(' ')
                .filter(|t| !t.is_empty())
                .collect();
            let tokens = analyzer.analyze(text);
            let actual: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
            let last = tokens.last().map_or(0, |t| t.position as u64);
            let expected_last = chunk[format!("{key}LastPosition")].as_u64().unwrap();
            if actual != expected || last != expected_last {
                let first = actual.iter().zip(&expected).position(|(a, e)| a != e);
                divergences.push(format!(
                    "{key} {id}: primo scarto {:?} ({:?}), posizione finale {last} vs {expected_last}",
                    first.map(|i| (actual[i], expected[i])),
                    first.map(|i| expected[i.saturating_sub(3)..(i + 3).min(expected.len())].join(" ")),
                ));
            }
        }
    }
    assert!(divergences.is_empty(), "{}", divergences.join("\n"));
}

mod golden {
    use std::collections::HashMap;

    use kubirag_domain::notebook::RagDocument;
    use kubirag_domain::retrieval::DocumentChunker;

    /// I chunk di default del corpus, con gli id della baseline.
    pub fn chunk_texts() -> HashMap<String, String> {
        let corpus = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../contracts/golden/retrieval/corpus");
        let mut texts = HashMap::new();
        for dir in ["guides", "manuals"] {
            for entry in std::fs::read_dir(corpus.join(dir)).unwrap() {
                let path = entry.unwrap().path();
                let id = format!("{dir}/{}", path.file_name().unwrap().to_str().unwrap());
                let document = RagDocument::new(id, std::fs::read_to_string(&path).unwrap());
                for chunk in DocumentChunker::default().chunk(&document) {
                    texts.insert(chunk.id, chunk.content);
                }
            }
        }
        texts
    }
}
