//! Fusione dei punteggi BM25 e vettoriali del secondo stadio. L'aritmetica è in `f32` come in
//! Kotlin (`Float`), così gli ordinamenti coincidono anche nei pareggi.

use std::collections::HashSet;

use indexmap::IndexMap;

use super::{DocumentChunker, SearchResult};

/// Al più un risultato per documento padre, nell'ordine di arrivo (`ResultDiversifier`).
pub fn diversify(results: Vec<SearchResult>) -> Vec<SearchResult> {
    let mut seen = HashSet::new();
    results
        .into_iter()
        .filter(|result| seen.insert(DocumentChunker::parent_id(&result.document_id).to_owned()))
        .collect()
}

/// Similarità coseno: accumulo in `f32`, radici in `f64`, come `RAGSystem.cosine`.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let (mut dot, mut norm_a, mut norm_b) = (0f32, 0f32, 0f32);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 0.0 || norm_b <= 0.0 {
        0.0
    } else {
        dot / ((norm_a as f64).sqrt() * (norm_b as f64).sqrt()) as f32
    }
}

/// Min-max in [0,1]; valori tutti uguali → 0.5, così l'ordine resta invariato.
pub fn normalize(scores: &[f32]) -> Vec<f32> {
    let Some(min) = scores.iter().copied().reduce(f32::min) else {
        return Vec::new();
    };
    let max = scores.iter().copied().reduce(f32::max).unwrap_or(min);
    if max == min {
        return vec![0.5; scores.len()];
    }
    scores.iter().map(|s| (s - min) / (max - min)).collect()
}

/// Pesi della fusione (`hybridSearchLuceneWeight` / `hybridSearchVectorWeight`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FusionWeights {
    pub lexical: f32,
    pub vector: f32,
}

/// Esito della fusione ristretta ai candidati (modalità `embedding`).
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateFusion {
    pub results: Vec<SearchResult>,
    pub fused_scores: IndexMap<String, f32>,
}

/// Fusione BM25 + coseno sui soli candidati: un candidato senza vettore riceve come punteggio
/// vettoriale il proprio BM25 normalizzato (neutro, mai penalizzato).
pub fn fuse_candidates(
    candidates: &[SearchResult],
    vector_scores: &IndexMap<String, f32>,
    weights: FusionWeights,
) -> CandidateFusion {
    let lexical = normalize(&candidates.iter().map(|c| c.score).collect::<Vec<_>>());
    let vector: IndexMap<&str, f32> = vector_scores
        .keys()
        .map(String::as_str)
        .zip(normalize(
            &vector_scores.values().copied().collect::<Vec<_>>(),
        ))
        .collect();
    let mut fused: Vec<(SearchResult, f32)> = candidates
        .iter()
        .zip(lexical)
        .map(|(candidate, nb)| {
            let nv = vector
                .get(candidate.document_id.as_str())
                .copied()
                .unwrap_or(nb);
            (
                candidate.clone(),
                nb * weights.lexical + nv * weights.vector,
            )
        })
        .collect();
    sort_descending(&mut fused);
    let fused_scores = fused
        .iter()
        .map(|(c, score)| (c.document_id.clone(), *score))
        .collect();
    let results = fused
        .into_iter()
        .enumerate()
        .map(|(index, (candidate, score))| SearchResult {
            rank: index + 1,
            score,
            ..candidate
        })
        .collect();
    CandidateFusion {
        results,
        fused_scores,
    }
}

/// `HybridSearchService.fuseScores`: fonde i candidati BM25 con i risultati vettoriali; chi
/// compare da una sola parte prende solo il peso di quella parte.
pub fn fuse_hybrid(
    lexical: &[SearchResult],
    vector: &[SearchResult],
    weights: FusionWeights,
) -> Vec<SearchResult> {
    if lexical.is_empty() {
        return vector.to_vec();
    }
    if vector.is_empty() {
        return lexical.to_vec();
    }
    let by_id = |results: Vec<SearchResult>| -> IndexMap<String, SearchResult> {
        results
            .into_iter()
            .map(|r| (r.document_id.clone(), r))
            .collect()
    };
    let lexical = by_id(normalize_results(lexical));
    let vector = by_id(normalize_results(vector));
    let mut ids: Vec<&String> = lexical.keys().collect();
    ids.extend(vector.keys().filter(|id| !lexical.contains_key(*id)));
    let mut fused: Vec<(SearchResult, f32)> = ids
        .into_iter()
        .map(|id| match (lexical.get(id), vector.get(id)) {
            (Some(l), Some(v)) => (
                l.clone(),
                l.score * weights.lexical + v.score * weights.vector,
            ),
            (Some(l), None) => (l.clone(), l.score * weights.lexical),
            (None, Some(v)) => (v.clone(), v.score * weights.vector),
            (None, None) => unreachable!("every id comes from one of the two maps"),
        })
        .collect();
    sort_descending(&mut fused);
    fused
        .into_iter()
        .enumerate()
        .map(|(index, (result, score))| SearchResult {
            rank: index + 1,
            score,
            ..result
        })
        .collect()
}

/// Min-max dei punteggi; liste di un elemento o di punteggi uguali restano invariate.
fn normalize_results(results: &[SearchResult]) -> Vec<SearchResult> {
    if results.len() <= 1 {
        return results.to_vec();
    }
    let scores: Vec<f32> = results.iter().map(|r| r.score).collect();
    let min = scores.iter().copied().reduce(f32::min).unwrap_or(0.0);
    let max = scores.iter().copied().reduce(f32::max).unwrap_or(1.0);
    if max == min {
        return results.to_vec();
    }
    results
        .iter()
        .map(|r| SearchResult {
            score: (r.score - min) / (max - min),
            ..r.clone()
        })
        .collect()
}

/// `sortedByDescending`: stabile, i pareggi restano nell'ordine di partenza.
fn sort_descending(scored: &mut [(SearchResult, f32)]) {
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: &str, score: f32) -> SearchResult {
        SearchResult {
            rank: 0,
            document_id: id.into(),
            content: String::new(),
            score,
            explanation: String::new(),
        }
    }

    const WEIGHTS: FusionWeights = FusionWeights {
        lexical: 0.7,
        vector: 0.3,
    };

    #[test]
    fn equal_scores_normalize_to_one_half() {
        assert_eq!(normalize(&[2.0, 2.0]), [0.5, 0.5]);
        assert_eq!(normalize(&[1.0, 3.0, 2.0]), [0.0, 1.0, 0.5]);
    }

    #[test]
    fn a_candidate_without_vector_keeps_a_neutral_score() {
        let candidates = [result("a", 3.0), result("b", 2.0), result("c", 1.0)];
        let vectors: IndexMap<String, f32> = [("b".into(), 0.9), ("c".into(), 0.1)].into();
        let fusion = fuse_candidates(&candidates, &vectors, WEIGHTS);
        let order: Vec<_> = fusion
            .results
            .iter()
            .map(|r| r.document_id.as_str())
            .collect();
        assert_eq!(order, ["a", "b", "c"]);
        assert!(
            (fusion.fused_scores["a"] - 1.0).abs() < 1e-6,
            "neutro: 1.0*0.7 + 1.0*0.3"
        );
        assert!((fusion.fused_scores["b"] - 0.65).abs() < 1e-6);
    }

    #[test]
    fn hybrid_fusion_can_introduce_documents() {
        let lexical = [result("a", 2.0), result("b", 1.0)];
        let vector = [result("c", 0.9), result("b", 0.8), result("a", 0.1)];
        let fused = fuse_hybrid(&lexical, &vector, WEIGHTS);
        let order: Vec<_> = fused
            .iter()
            .map(|r| (r.document_id.as_str(), r.rank))
            .collect();
        assert_eq!(
            order,
            [("a", 1), ("c", 2), ("b", 3)],
            "b: 0*0.7 + 0.875*0.3 < c: 1*0.3"
        );
        assert!(
            (fused[1].score - 0.3).abs() < 1e-6,
            "solo vettoriale: 1.0 * 0.3"
        );
    }

    #[test]
    fn cosine_of_orthogonal_and_null_vectors() {
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert!((cosine(&[1.0, 1.0], &[2.0, 2.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn diversification_keeps_the_first_chunk_of_each_parent() {
        let kept = diversify(vec![
            result("a_chunk_1", 3.0),
            result("b", 2.0),
            result("a_chunk_0", 1.0),
        ]);
        let ids: Vec<_> = kept.iter().map(|r| r.document_id.as_str()).collect();
        assert_eq!(ids, ["a_chunk_1", "b"]);
    }
}
