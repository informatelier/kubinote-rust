//! Rerank listwise via LLM (`LLMRerankingService`): i candidati sono presentati come `[1]..[N]`
//! e il modello risponde con una permutazione di indici in JSON. Gli id dei documenti non
//! passano mai dal modello: i modelli piccoli storpiano gli UUID e l'errore è silenzioso.

use serde::Deserialize;

use super::text::{take_utf16, utf16_len};
use super::{RerankOutcome, SearchResult};

const PREVIEW_CHARS: usize = 300;

/// Il prompt del rerank: anteprima di 300 caratteri per candidato.
pub fn rerank_prompt(question: &str, candidates: &[SearchResult]) -> String {
    let documents = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let ellipsis = if utf16_len(&candidate.content) > PREVIEW_CHARS {
                "..."
            } else {
                ""
            };
            format!(
                "[{}] {}{ellipsis}",
                index + 1,
                take_utf16(&candidate.content, PREVIEW_CHARS)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "Rank the following documents by relevance to the query: \"{question}\"\n\nDocuments:\n{documents}\n\nRespond with JSON only: the document numbers ordered from most to least relevant.\nExample: {{\"ranking\": [3, 1, 2]}}"
    )
}

#[derive(Deserialize)]
struct RankingResponse {
    ranking: Vec<i32>,
}

/// La risposta conteneva un numero fuori dall'intervallo di `Int`: in Kotlin è un'eccezione,
/// quindi un esito `ERROR` e non `PARSE_FAILED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingOverflow(pub String);

/// Indici 1-based: JSON `{"ranking":[...]}`, altrimenti ogni sequenza di cifre in ordine.
/// Indici fuori intervallo e duplicati sono scartati.
pub fn parse_ranking_indices(
    response: &str,
    candidate_count: usize,
) -> Result<Vec<usize>, RankingOverflow> {
    let trimmed = response.trim_matches(|c: char| c <= ' ');
    let parsed: Vec<i64> = match serde_json::from_str::<RankingResponse>(trimmed) {
        Ok(json) => json.ranking.into_iter().map(i64::from).collect(),
        Err(_) => digit_runs(response)
            .map(|digits| {
                digits
                    .parse::<i32>()
                    .map(i64::from)
                    .map_err(|_| RankingOverflow(digits.to_owned()))
            })
            .collect::<Result<_, _>>()?,
    };
    let mut indices = Vec::new();
    for index in parsed {
        if (1..=candidate_count as i64).contains(&index) && !indices.contains(&(index as usize)) {
            indices.push(index as usize);
        }
    }
    Ok(indices)
}

/// `Regex("\\d+")`: cifre ASCII.
fn digit_runs(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|run| !run.is_empty())
}

/// Riordina per indici 1-based; i candidati non citati seguono nell'ordine originale.
pub fn reorder_by_indices(candidates: &[SearchResult], indices: &[usize]) -> Vec<SearchResult> {
    let picked: Vec<&SearchResult> = indices.iter().map(|i| &candidates[i - 1]).collect();
    let remaining = candidates.iter().filter(|c| !picked.contains(c));
    picked
        .iter()
        .copied()
        .chain(remaining)
        .enumerate()
        .map(|(index, result)| SearchResult {
            rank: index + 1,
            ..result.clone()
        })
        .collect()
}

/// Interpreta la risposta del modello: ordine nuovo con esito, oppure l'ordine BM25 con
/// `PARSE_FAILED`/`ERROR`.
pub fn apply_ranking_response(
    candidates: &[SearchResult],
    response: &str,
) -> (Vec<SearchResult>, RerankOutcome) {
    match parse_ranking_indices(response, candidates.len()) {
        Err(_) => (candidates.to_vec(), RerankOutcome::Error),
        Ok(indices) if indices.is_empty() => (candidates.to_vec(), RerankOutcome::ParseFailed),
        Ok(indices) => {
            let reordered = reorder_by_indices(candidates, &indices);
            let outcome = RerankOutcome::comparing(candidates, &reordered);
            (reordered, outcome)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates(ids: &[&str]) -> Vec<SearchResult> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| SearchResult {
                rank: i + 1,
                document_id: (*id).into(),
                content: format!("contenuto {id}"),
                score: 1.0,
                explanation: String::new(),
            })
            .collect()
    }

    #[test]
    fn json_ranking_is_preferred_and_filtered() {
        assert_eq!(
            parse_ranking_indices(r#" {"ranking":[3,1,3,9,0]} "#, 3),
            Ok(vec![3, 1])
        );
        assert_eq!(parse_ranking_indices("ordine: 2 poi 1", 3), Ok(vec![2, 1]));
        assert_eq!(parse_ranking_indices("nessun numero", 3), Ok(vec![]));
        assert!(parse_ranking_indices("99999999999", 3).is_err());
    }

    #[test]
    fn unmentioned_candidates_keep_their_order() {
        let reordered = reorder_by_indices(&candidates(&["a", "b", "c"]), &[3]);
        let order: Vec<_> = reordered
            .iter()
            .map(|r| (r.document_id.as_str(), r.rank))
            .collect();
        assert_eq!(order, [("c", 1), ("a", 2), ("b", 3)]);
    }

    #[test]
    fn every_response_maps_to_an_outcome() {
        let docs = candidates(&["a", "b"]);
        assert_eq!(
            apply_ranking_response(&docs, r#"{"ranking":[2,1]}"#).1,
            RerankOutcome::Reordered
        );
        assert_eq!(
            apply_ranking_response(&docs, r#"{"ranking":[1,2]}"#).1,
            RerankOutcome::Unchanged
        );
        assert_eq!(
            apply_ranking_response(&docs, "boh").1,
            RerankOutcome::ParseFailed
        );
        assert_eq!(
            apply_ranking_response(&docs, "4294967296").1,
            RerankOutcome::Error
        );
    }

    #[test]
    fn the_prompt_shows_300_characters_per_candidate() {
        let mut docs = candidates(&["a"]);
        docs[0].content = "x".repeat(301);
        let prompt = rerank_prompt("q", &docs);
        assert!(prompt.contains(&format!("[1] {}...\n\nRespond", "x".repeat(300))));
        assert!(prompt.starts_with(
            "Rank the following documents by relevance to the query: \"q\"\n\nDocuments:\n"
        ));
    }
}
