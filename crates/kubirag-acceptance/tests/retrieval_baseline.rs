//! O2-KR1: la baseline Lucene è il riferimento del motore Rust (F2).
//! In F0 si verifica che baseline, query e corpus siano coerenti e si fissano i numeri di Lucene.

use std::collections::HashMap;

use kubirag_acceptance::golden;
use kubirag_acceptance::retrieval::{ndcg_at_k, overlap_at_k, ranked_sources};
use serde_json::Value;

struct Baseline {
    queries: Vec<Value>,
    results: HashMap<String, Vec<String>>,
}

fn baseline() -> Baseline {
    let queries = golden::json("retrieval/queries.json")["queries"]
        .as_array()
        .unwrap()
        .clone();
    let results = golden::json("retrieval/lucene-baseline.json")["queries"]
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
        .collect();
    Baseline { queries, results }
}

fn relevant(query: &Value) -> Vec<&str> {
    query["relevant"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect()
}

#[test]
fn every_query_has_a_baseline_on_existing_sources() {
    let baseline = baseline();
    let corpus = golden::dir().join("retrieval/corpus");
    let mut divergences = Vec::new();
    for query in &baseline.queries {
        let id = query["id"].as_str().unwrap();
        let Some(chunks) = baseline.results.get(id) else {
            divergences.push(format!("{id}: nessun risultato Lucene"));
            continue;
        };
        for source in ranked_sources(chunks.iter().map(String::as_str))
            .into_iter()
            .chain(relevant(query))
        {
            if !corpus.join(source).is_file() {
                divergences.push(format!("{id}: sorgente assente dal corpus {source}"));
            }
        }
    }
    golden::assert_no_divergence("retrieval baseline", divergences);
}

/// I numeri di Lucene sono la soglia di O2-KR1: il motore Rust non potrà scendere oltre 2 punti
/// di nDCG@5. Se questo test cambia, è cambiata la baseline, non il motore Rust.
#[test]
fn lucene_reference_metrics_are_fixed() {
    let baseline = baseline();
    let mut hits_at_1 = 0;
    let mut ndcg_sum = 0.0;
    for query in &baseline.queries {
        let chunks = &baseline.results[query["id"].as_str().unwrap()];
        let ranked = ranked_sources(chunks.iter().map(String::as_str));
        let relevant = relevant(query);
        hits_at_1 += usize::from(ranked.first().is_some_and(|top| relevant.contains(top)));
        ndcg_sum += ndcg_at_k(&ranked, &relevant, 5);
        assert_eq!(overlap_at_k(&ranked, &ranked, 5), 1.0);
    }
    let ndcg = ndcg_sum / baseline.queries.len() as f64;
    println!(
        "Lucene: hit@1 {hits_at_1}/{}, nDCG@5 medio {ndcg:.4}",
        baseline.queries.len()
    );
    assert_eq!((hits_at_1, baseline.queries.len()), (18, 24));
    assert!(
        (ndcg - 0.8855).abs() < 5e-4,
        "nDCG@5 medio della baseline: {ndcg:.4}"
    );
}
