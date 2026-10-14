//! Regole pure del Retrieval contro la versione Kotlin: rilevamento della lingua e chunking.

use std::collections::HashMap;

use kubirag_acceptance::golden;
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::text::utf16_len;
use kubirag_domain::retrieval::{detect_language, ChunkConfig, DocumentChunker};

#[test]
fn language_detection_matches_kotlin() {
    let golden = golden::json("retrieval/language.json");
    let chunks = default_chunks();
    let mut divergences = Vec::new();
    for case in golden["cases"].as_array().unwrap() {
        // I casi senza testo sono i chunk di default del corpus, per id.
        let text = case["text"]
            .as_str()
            .unwrap_or_else(|| &chunks[case["id"].as_str().unwrap()]);
        let detection = detect_language(text);
        let expected = (
            case["language"].as_str().unwrap(),
            case["confidence"].as_f64().unwrap() as f32,
        );
        if (detection.language.name(), detection.confidence) != expected {
            divergences.push(format!(
                "{:?}: {:?} invece di {expected:?}",
                text.chars().take(60).collect::<String>(),
                (detection.language.name(), detection.confidence)
            ));
        }
    }
    golden::assert_no_divergence("LanguageDetector", divergences);
}

fn default_chunks() -> HashMap<String, String> {
    let queries = golden::json("retrieval/chunks.json");
    let mut texts = HashMap::new();
    for source in queries["configs"][0]["sources"].as_array().unwrap() {
        let path = source["path"].as_str().unwrap();
        let document = RagDocument::new(path, golden::read(&format!("retrieval/corpus/{path}")));
        for chunk in DocumentChunker::default().chunk(&document) {
            texts.insert(chunk.id, chunk.content);
        }
    }
    texts
}

#[test]
fn chunking_matches_kotlin() {
    let golden = golden::json("retrieval/chunks.json");
    let mut divergences = Vec::new();
    for config in golden["configs"].as_array().unwrap() {
        let size = |key: &str| config[key].as_u64().unwrap() as usize;
        let chunker = DocumentChunker::new(
            ChunkConfig::new(size("chunkSize"), size("overlap"), size("minChunkSize")).unwrap(),
        );
        let label = config["label"].as_str().unwrap();
        for source in config["sources"].as_array().unwrap() {
            let path = source["path"].as_str().unwrap();
            let mut document =
                RagDocument::new(path, golden::read(&format!("retrieval/corpus/{path}")));
            document.metadata.insert("path".into(), path.into());
            let chunks = chunker.chunk(&document);
            let expected = source["chunks"].as_array().unwrap();
            if chunks.len() != expected.len() {
                divergences.push(format!(
                    "{label} {path}: {} chunk invece di {}",
                    chunks.len(),
                    expected.len()
                ));
                continue;
            }
            for (chunk, expected) in chunks.iter().zip(expected) {
                let words = DocumentChunker::tokenize(&chunk.content);
                let metadata: serde_json::Value = serde_json::to_value(&chunk.metadata).unwrap();
                let actual = serde_json::json!({
                    "id": chunk.id,
                    "metadata": metadata,
                    "words": words.len(),
                    "first": words.first(),
                    "last": words.last(),
                    "chars": utf16_len(&chunk.content),
                });
                if &actual != expected {
                    divergences.push(format!(
                        "{label} {}: {actual} invece di {expected}",
                        chunk.id
                    ));
                }
            }
        }
    }
    golden::assert_no_divergence("DocumentChunkingService", divergences);
}
