//! Metriche dell'obiettivo O2 (retrieval almeno pari a quello attuale).
//!
//! - KR1: overlap top-5 ≥ 80% rispetto a Lucene e nDCG@5 non inferiore di oltre 2 punti.
//!
//! Le metriche lavorano sulle **sorgenti** (file del corpus), non sui chunk: due motori con
//! chunking diverso devono restare confrontabili.

use std::collections::HashSet;

/// Porta che il motore di retrieval Rust (F2) implementerà per entrare nella suite.
pub trait Retriever {
    /// Id dei chunk nell'ordine di rank, al più `k`.
    fn search(&self, query: &str, k: usize) -> Vec<String>;
}

/// `guides/tuning-guide.md_chunk_3` → `guides/tuning-guide.md`.
pub fn source_of(chunk_id: &str) -> &str {
    chunk_id
        .split_once("_chunk_")
        .map_or(chunk_id, |(source, _)| source)
}

/// Sorgenti distinte nell'ordine del primo chunk che le rappresenta.
pub fn ranked_sources<'a>(chunk_ids: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut seen = HashSet::new();
    chunk_ids
        .into_iter()
        .map(source_of)
        .filter(|source| seen.insert(*source))
        .collect()
}

/// Frazione delle prime `k` sorgenti del riferimento presenti nelle prime `k` del candidato.
pub fn overlap_at_k(reference: &[&str], candidate: &[&str], k: usize) -> f64 {
    let expected: HashSet<&str> = reference.iter().take(k).copied().collect();
    if expected.is_empty() {
        return 1.0;
    }
    let found = candidate
        .iter()
        .take(k)
        .filter(|source| expected.contains(*source))
        .count();
    found as f64 / expected.len() as f64
}

/// nDCG@k con rilevanza binaria rispetto alle sorgenti giudicate rilevanti.
pub fn ndcg_at_k(ranked: &[&str], relevant: &[&str], k: usize) -> f64 {
    let gain = |position: usize| 1.0 / ((position + 2) as f64).log2();
    let dcg: f64 = ranked
        .iter()
        .take(k)
        .enumerate()
        .filter(|(_, source)| relevant.contains(source))
        .map(|(position, _)| gain(position))
        .sum();
    let ideal: f64 = (0..relevant.len().min(k)).map(gain).sum();
    if ideal == 0.0 {
        0.0
    } else {
        dcg / ideal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_collapse_to_their_source() {
        let ranked = ranked_sources(["a.md_chunk_1", "b.md", "a.md_chunk_0"]);
        assert_eq!(ranked, ["a.md", "b.md"]);
    }

    #[test]
    fn overlap_counts_shared_sources_in_the_top_k() {
        assert_eq!(overlap_at_k(&["a", "b"], &["b", "c"], 2), 0.5);
    }

    #[test]
    fn perfect_ranking_has_ndcg_one() {
        assert!((ndcg_at_k(&["a", "b", "c"], &["a", "b"], 5) - 1.0).abs() < 1e-12);
        assert_eq!(ndcg_at_k(&["c"], &["a"], 5), 0.0);
    }
}
