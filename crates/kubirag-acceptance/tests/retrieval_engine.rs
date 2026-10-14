//! O2-KR1 e O2-KR2: il motore BM25 Rust contro la baseline Lucene.
//!
//! - KR1: overlap top-5 ≥ 80% rispetto a Lucene e nDCG@5 non più di 2 punti sotto 0.8855.
//! - KR2: p95 della latenza ≤ quella di Lucene e cold start < 1 s, misurati come il
//!   generatore JVM (`RetrievalContracts.writeBenchmark`). I tempi contano solo con le
//!   ottimizzazioni attive (`cargo test --release`); in debug vengono solo stampati.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use indexmap::IndexMap;
use kubirag_acceptance::golden;
use kubirag_acceptance::retrieval::{ndcg_at_k, overlap_at_k, ranked_sources, Retriever};
use kubirag_app::retrieval::SearchEngine;
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::{DocumentChunker, SearchOptions};
use kubirag_retrieval::LuceneCompatibleSearchEngine;
use serde_json::Value;

const LUCENE_NDCG_AT_5: f64 = 0.8855;

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

/// Il corpus come lo indicizza il generatore: chunking di default, id = percorso relativo.
fn corpus_chunks() -> Vec<RagDocument> {
    let corpus = golden::dir().join("retrieval/corpus");
    let mut files = Vec::new();
    markdown_files(&corpus, &mut files);
    let relative = |p: &Path| {
        p.strip_prefix(&corpus)
            .unwrap()
            .components()
            .map(|c| c.as_os_str().to_str().unwrap().to_owned())
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

fn engine(chunks: &[RagDocument]) -> LuceneCompatibleSearchEngine {
    let mut engine = LuceneCompatibleSearchEngine::new();
    engine.configure(SearchOptions {
        max_results: 10,
        ..SearchOptions::default()
    });
    engine.index_all(chunks);
    engine
}

/// Il motore Rust dietro la porta della suite di accettazione.
struct RustRetriever(RefCell<LuceneCompatibleSearchEngine>);

impl Retriever for RustRetriever {
    fn search(&self, query: &str, k: usize) -> Vec<String> {
        self.0
            .borrow_mut()
            .search(query, k)
            .into_iter()
            .map(|r| r.document_id)
            .collect()
    }
}

fn queries() -> Vec<Value> {
    golden::json("retrieval/queries.json")["queries"]
        .as_array()
        .unwrap()
        .clone()
}

fn lucene_results() -> HashMap<String, Vec<String>> {
    golden::json("retrieval/lucene-baseline.json")["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            let chunks = q["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["chunkId"].as_str().unwrap().to_owned())
                .collect();
            (q["id"].as_str().unwrap().to_owned(), chunks)
        })
        .collect()
}

#[test]
fn kr1_rust_ranking_matches_lucene_on_the_baseline_queries() {
    let retriever = RustRetriever(RefCell::new(engine(&corpus_chunks())));
    let lucene = lucene_results();
    let queries = queries();
    let (mut overlap_sum, mut overlap_min, mut ndcg_sum) = (0.0, 1.0_f64, 0.0);
    let mut low_overlap = Vec::new();
    for query in &queries {
        let id = query["id"].as_str().unwrap();
        let rust = retriever.search(query["text"].as_str().unwrap(), 10);
        let rust_sources = ranked_sources(rust.iter().map(String::as_str));
        let lucene_sources = ranked_sources(lucene[id].iter().map(String::as_str));
        let relevant: Vec<&str> = query["relevant"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        let overlap = overlap_at_k(&lucene_sources, &rust_sources, 5);
        if overlap < 0.8 {
            low_overlap.push(format!("{id}: overlap@5 {overlap:.2}"));
        }
        overlap_sum += overlap;
        overlap_min = overlap_min.min(overlap);
        ndcg_sum += ndcg_at_k(&rust_sources, &relevant, 5);
    }
    let count = queries.len() as f64;
    let (overlap, ndcg) = (overlap_sum / count, ndcg_sum / count);
    println!(
        "Rust: overlap@5 medio {overlap:.4} (minimo {overlap_min:.2}), nDCG@5 medio {ndcg:.4} (Lucene {LUCENE_NDCG_AT_5})"
    );
    golden::assert_no_divergence("O2-KR1 overlap@5 per query", low_overlap);
    assert!(
        ndcg >= LUCENE_NDCG_AT_5 - 0.02,
        "nDCG@5 {ndcg:.4} più di 2 punti sotto Lucene"
    );
}

#[test]
fn kr2_latency_and_cold_start_are_within_the_lucene_reference() {
    let reference = golden::json("retrieval/lucene-latency.json");
    let chunks = corpus_chunks();
    let texts: Vec<String> = queries()
        .iter()
        .map(|q| q["text"].as_str().unwrap().to_owned())
        .collect();

    let cold = Instant::now();
    let mut engine = engine(&chunks);
    engine.search(&texts[0], 6);
    let cold_ms = cold.elapsed().as_secs_f64() * 1e3;

    for _ in 0..50 {
        for text in &texts {
            engine.search(text, 6);
        }
    }
    let mut samples: Vec<f64> = (0..40)
        .flat_map(|_| texts.iter())
        .map(|text| {
            let start = Instant::now();
            engine.search(text, 6);
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    let p50 = samples[samples.len() / 2];
    let p95 = samples[samples.len() * 95 / 100];
    let lucene_p95 = reference["p95Ms"].as_f64().unwrap();
    println!(
        "Rust: {} query, p50 {p50:.4} ms, p95 {p95:.4} ms (Lucene {lucene_p95:.4}), cold start {cold_ms:.1} ms (Lucene {:.1})",
        samples.len(),
        reference["coldStartMs"].as_f64().unwrap()
    );
    assert_eq!(
        samples.len(),
        reference["queries"].as_u64().unwrap() as usize
    );
    if cfg!(debug_assertions) {
        println!("build di debug: soglie di KR2 non verificate (usa --release)");
        return;
    }
    assert!(
        p95 <= lucene_p95,
        "p95 {p95:.4} ms oltre Lucene {lucene_p95:.4} ms"
    );
    assert!(cold_ms < 1000.0, "cold start {cold_ms:.1} ms");
}
