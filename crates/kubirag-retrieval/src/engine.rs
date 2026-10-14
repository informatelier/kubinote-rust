use kubi_kernel::json::kotlin_float_to_string as float;
use kubirag_app::retrieval::SearchEngine;
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::{
    detect_language, diversify, CacheStats, SearchOptions, SearchResult, PARENT_ID,
};

use crate::expansion::expand_query;
use crate::index::{bm25, byte4_to_int, idf, Bm25Index, Field, FieldIndex, StoredChunk, B, K1};

/// `LuceneSearchEngine`: un documento Lucene per chunk, testo in `content` (italiano) o
/// `content_en` (inglese con confidenza ≥ 0.6), `title` dai metadati.
///
/// La query (`RagLucene.buildOptimizedQuery`) è la disgiunzione di: phrase query sulla prima
/// frase tra virgolette (campo `content`, termini solo minuscoli), query espansa analizzata
/// per `content`, `content_en` e `title`. Le clausole ripetute si fondono sommando il boost,
/// come la riscrittura di `BooleanQuery`.
#[derive(Debug, Default)]
pub struct LuceneCompatibleSearchEngine {
    options: SearchOptions,
    index: Bm25Index,
    reader_open: bool,
    cache: CacheStats,
}

/// Una clausola `SHOULD` su un termine, con il boost delle copie fuse.
#[derive(Debug, Clone, PartialEq)]
struct TermClause {
    field: Field,
    term: String,
    boost: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Query {
    phrase: Option<Vec<String>>,
    terms: Vec<TermClause>,
}

/// Il contributo di una clausola al punteggio di un documento (per la spiegazione).
struct Contribution<'a> {
    label: String,
    score: f32,
    boost: f32,
    idf: f32,
    doc_freq: usize,
    doc_count: u64,
    freq: f32,
    field: &'a FieldIndex,
}

impl LuceneCompatibleSearchEngine {
    pub fn new() -> Self {
        Self::default()
    }

    fn expanded(&self, query: &str) -> String {
        if self.options.query_expansion_enabled {
            expand_query(query)
        } else {
            query.to_owned()
        }
    }

    /// `None` quando il `QueryParser` fallirebbe (query espansa vuota): nessun risultato.
    fn build_query(&self, query: &str) -> Option<Query> {
        let expanded = self.expanded(query);
        let pieces: Vec<&str> = expanded
            .split([' ', '\t', '\n', '\r', '\u{3000}'])
            .filter(|p| !p.is_empty())
            .collect();
        if pieces.is_empty() {
            return None;
        }
        let mut terms: Vec<TermClause> = Vec::new();
        for field in Field::ALL {
            for piece in &pieces {
                for term in field.analyzer().terms(piece) {
                    match terms
                        .iter_mut()
                        .find(|c| c.field == field && c.term == term)
                    {
                        Some(clause) => clause.boost += 1.0,
                        None => terms.push(TermClause {
                            field,
                            term,
                            boost: 1.0,
                        }),
                    }
                }
            }
        }
        Some(Query {
            phrase: phrase_terms(query),
            terms,
        })
    }

    fn contributions(&self, query: &Query, doc: u32) -> Vec<Contribution<'_>> {
        let mut contributions = Vec::new();
        if let Some((field, score)) = query.phrase.as_ref().and_then(|terms| {
            let field = self.index.field(Field::Content)?;
            let freq = phrase_freq(field, terms, doc)?;
            let (idf, _) = phrase_idf(field, terms)?;
            let cache = field.norm_cache();
            Some((
                field,
                (bm25(idf, freq, cache[field.norm(doc) as usize]), idf, freq),
            ))
        }) {
            let (value, idf, freq) = score;
            contributions.push(Contribution {
                label: format!(
                    "weight(content:\"{}\" in {doc})",
                    query.phrase.as_ref().map_or(String::new(), |t| t.join(" "))
                ),
                score: value,
                boost: 1.0,
                idf,
                doc_freq: 0,
                doc_count: field.doc_count(),
                freq,
                field,
            });
        }
        for clause in &query.terms {
            let Some(field) = self.index.field(clause.field) else {
                continue;
            };
            let postings = field.postings(&clause.term);
            let Some(posting) = postings.iter().find(|p| p.doc == doc) else {
                continue;
            };
            let term_idf = idf(postings.len() as u64, field.doc_count());
            let weight = clause.boost * term_idf;
            let freq = posting.positions.len() as f32;
            let cache = field.norm_cache();
            contributions.push(Contribution {
                label: format!("weight({}:{} in {doc})", clause.field.name(), clause.term),
                score: bm25(weight, freq, cache[field.norm(doc) as usize]),
                boost: clause.boost,
                idf: term_idf,
                doc_freq: postings.len(),
                doc_count: field.doc_count(),
                freq,
                field,
            });
        }
        contributions
    }

    /// Punteggi di tutti i documenti che soddisfano almeno una clausola.
    fn score_all(&self, query: &Query) -> Vec<(u32, f32)> {
        let mut totals: Vec<Option<f64>> = vec![None; self.index.docs.len()];
        let mut add = |doc: u32, score: f32| {
            let slot = &mut totals[doc as usize];
            *slot = Some(slot.unwrap_or(0.0) + f64::from(score));
        };
        if let (Some(terms), Some(field)) = (&query.phrase, self.index.field(Field::Content)) {
            if let Some((weight, first)) = phrase_idf(field, terms) {
                let cache = field.norm_cache();
                for posting in first {
                    if let Some(freq) = phrase_freq(field, terms, posting.doc) {
                        add(
                            posting.doc,
                            bm25(weight, freq, cache[field.norm(posting.doc) as usize]),
                        );
                    }
                }
            }
        }
        for clause in &query.terms {
            let Some(field) = self.index.field(clause.field) else {
                continue;
            };
            let postings = field.postings(&clause.term);
            if postings.is_empty() {
                continue;
            }
            let weight = clause.boost * idf(postings.len() as u64, field.doc_count());
            let cache = field.norm_cache();
            for posting in postings {
                let freq = posting.positions.len() as f32;
                add(
                    posting.doc,
                    bm25(weight, freq, cache[field.norm(posting.doc) as usize]),
                );
            }
        }
        totals
            .into_iter()
            .enumerate()
            .filter_map(|(doc, total)| total.map(|t| (doc as u32, t as f32)))
            .collect()
    }

    fn explain(&self, query: &Query, doc: u32, total: f32) -> String {
        let mut lines = vec![format!("{} = sum of:", float(total))];
        for c in self.contributions(query, doc) {
            let dl = byte4_to_int(c.field.norm(doc)) as f32;
            let avgdl = c.field.avgdl();
            let tf = c.freq / (c.freq + K1 * (1.0 - B + B * dl / avgdl));
            lines.push(format!(
                "  {} = {} [BM25Similarity], result of:",
                float(c.score),
                c.label
            ));
            lines.push(format!(
                "    {} = score(freq={}), computed as boost * idf * tf from:",
                float(c.score),
                float(c.freq)
            ));
            if c.boost != 1.0 {
                lines.push(format!("      {} = boost", float(c.boost)));
            }
            lines.push(format!(
                "      {} = idf, computed as log(1 + (N - n + 0.5) / (n + 0.5)) from:",
                float(c.idf)
            ));
            lines.push(format!(
                "        {} = n, number of documents containing term",
                c.doc_freq
            ));
            lines.push(format!(
                "        {} = N, total number of documents with field",
                c.doc_count
            ));
            lines.push(format!(
                "      {} = tf, computed as freq / (freq + k1 * (1 - b + b * dl / avgdl)) from:",
                float(tf)
            ));
            lines.push(format!(
                "        {} = freq, occurrences of term within document",
                float(c.freq)
            ));
            lines.push(format!(
                "        {} = k1, term saturation parameter",
                float(K1)
            ));
            lines.push(format!(
                "        {} = b, length normalization parameter",
                float(B)
            ));
            lines.push(format!(
                "        {} = dl, length of field (approximate)",
                float(dl)
            ));
            lines.push(format!(
                "        {} = avgdl, average length of field",
                float(avgdl)
            ));
        }
        lines.join("\n") + "\n"
    }
}

/// Prima frase tra virgolette, spezzata su `\s+` (ASCII) e messa in minuscolo, senza analyzer.
fn phrase_terms(query: &str) -> Option<Vec<String>> {
    let start = query.find('"')?;
    let rest = &query[start + 1..];
    let end = rest.find('"')?;
    if end == 0 {
        // `"([^"]+)"`: virgolette vuote; si riprova dalla seconda.
        return phrase_terms(rest);
    }
    Some(
        split_kotlin_whitespace(&rest[..end])
            .map(str::to_lowercase)
            .collect(),
    )
}

/// `split(Regex("\\s+"))` di Kotlin: le parti vuote iniziali e finali restano.
fn split_kotlin_whitespace(text: &str) -> impl Iterator<Item = &str> {
    let is_space = |c: char| matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r');
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if is_space(c) {
            pieces.push(&text[start..index]);
            let mut next = index + 1;
            while let Some((i, c)) = chars.peek().copied() {
                if !is_space(c) {
                    break;
                }
                next = i + 1;
                chars.next();
            }
            start = next;
        }
    }
    pieces.push(&text[start..]);
    pieces.into_iter()
}

/// Somma degli idf dei termini (in `f64`, poi `f32`) e le posizioni del primo termine.
fn phrase_idf<'a>(
    field: &'a FieldIndex,
    terms: &[String],
) -> Option<(f32, &'a [crate::index::Posting])> {
    let mut sum = 0f64;
    for term in terms {
        let postings = field.postings(term);
        if postings.is_empty() {
            return None;
        }
        sum += f64::from(idf(postings.len() as u64, field.doc_count()));
    }
    Some((sum as f32, field.postings(&terms[0])))
}

/// Occorrenze esatte (slop 0) della frase nel documento.
fn phrase_freq(field: &FieldIndex, terms: &[String], doc: u32) -> Option<f32> {
    let positions: Vec<&[u32]> = terms
        .iter()
        .map(|term| {
            field
                .postings(term)
                .iter()
                .find(|p| p.doc == doc)
                .map(|p| p.positions.as_slice())
        })
        .collect::<Option<_>>()?;
    let count = positions[0]
        .iter()
        .filter(|start| {
            positions
                .iter()
                .enumerate()
                .all(|(offset, list)| list.contains(&(**start + offset as u32)))
        })
        .count();
    (count > 0).then_some(count as f32)
}

impl SearchEngine for LuceneCompatibleSearchEngine {
    fn configure(&mut self, options: SearchOptions) {
        self.options = options;
    }

    fn index_all(&mut self, chunks: &[RagDocument]) {
        let mut index = Bm25Index::default();
        for chunk in chunks {
            let english = detect_language(&chunk.content).is_confident_english();
            let content_field = if english {
                Field::ContentEn
            } else {
                Field::Content
            };
            let title = chunk
                .metadata
                .get("title")
                .or_else(|| chunk.metadata.get("metaTitle"))
                .map_or("", String::as_str);
            let mut fields = vec![(content_field, chunk.content.as_str())];
            if !title.trim().is_empty() {
                fields.push((Field::Title, title));
            }
            let parent_id = chunk.metadata.get(PARENT_ID).unwrap_or(&chunk.id).clone();
            index.add(
                &fields,
                StoredChunk {
                    parent_id,
                    text: chunk.content.clone(),
                },
            );
        }
        self.index = index;
        self.reader_open = false;
        self.cache = CacheStats::default();
    }

    fn search(&mut self, query: &str, max_results: usize) -> Vec<SearchResult> {
        if self.reader_open {
            self.cache.hits += 1;
        } else {
            self.reader_open = true;
            self.cache.misses += 1;
        }
        let Some(parsed) = self.build_query(query) else {
            return Vec::new();
        };
        let mut scored = self.score_all(&parsed);
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(&b.0))
        });
        scored.truncate(max_results * 2);
        let results: Vec<SearchResult> = scored
            .into_iter()
            .enumerate()
            .map(|(index, (doc, score))| {
                let stored = &self.index.docs[doc as usize];
                SearchResult {
                    rank: index + 1,
                    document_id: stored.parent_id.clone(),
                    content: stored.text.clone(),
                    score,
                    explanation: self.explain(&parsed, doc, score),
                }
            })
            .collect();
        let mut results = if self.options.diversification_enabled {
            diversify(results)
        } else {
            results
        };
        results.truncate(max_results);
        results
    }

    fn expanded_query(&self, query: &str) -> String {
        self.expanded(query)
    }

    fn cache_stats(&self) -> CacheStats {
        self.cache
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_quoted_phrase_is_lowercased_but_not_analyzed() {
        assert_eq!(
            phrase_terms(r#"cerca "Note Markdown" e "altro""#),
            Some(vec!["note".into(), "markdown".into()])
        );
        assert_eq!(phrase_terms("senza virgolette"), None);
        let reopened = phrase_terms(r#"vuote "" poi "x""#).unwrap();
        assert_eq!(
            reopened,
            ["", "poi", ""],
            "la seconda virgoletta apre la frase, come la regex"
        );
        assert_eq!(
            phrase_terms(r#"" spazio""#),
            Some(vec!["".into(), "spazio".into()])
        );
    }

    #[test]
    fn duplicate_terms_merge_into_one_boosted_clause() {
        let engine = LuceneCompatibleSearchEngine {
            options: SearchOptions::default(),
            ..Default::default()
        };
        let query = engine.build_query("kotlin").unwrap();
        let kotlin = query
            .terms
            .iter()
            .find(|c| c.field == Field::Content && c.term == "kotlin")
            .unwrap();
        assert_eq!(kotlin.boost, 3.0);
        assert!(engine.build_query("il lo la").is_none());
    }
}
