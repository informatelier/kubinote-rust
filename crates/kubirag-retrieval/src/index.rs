//! Indice invertito con la stessa matematica BM25 di Lucene 10.4 (`BM25Similarity`, k1 = 1.2,
//! b = 0.75): norme a un byte (`SmallFloat.intToByte4`), idf e punteggi in `f32`, somma delle
//! clausole in `f64` come `BooleanScorer`. Così i punteggi coincidono con quelli della JVM.

use std::collections::HashMap;

use crate::analysis::Analyzer;

pub(crate) const K1: f32 = 1.2;
pub(crate) const B: f32 = 0.75;

/// I campi ricercabili del documento Lucene di un chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Field {
    /// Italiano o lingua sconosciuta (`ItalianAnalyzer`).
    Content,
    /// Inglese rilevato con confidenza ≥ 0.6 (`EnglishAnalyzer`).
    ContentEn,
    /// `metadata.title` o `metadata.metaTitle` (`ItalianAnalyzer`).
    Title,
}

impl Field {
    pub(crate) const ALL: [Self; 3] = [Self::Content, Self::ContentEn, Self::Title];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::ContentEn => "content_en",
            Self::Title => "title",
        }
    }

    pub(crate) fn analyzer(self) -> Analyzer {
        match self {
            Self::ContentEn => Analyzer::English,
            Self::Content | Self::Title => Analyzer::Italian,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Posting {
    pub doc: u32,
    pub positions: Vec<u32>,
}

#[derive(Debug, Default)]
pub(crate) struct FieldIndex {
    terms: HashMap<String, Vec<Posting>>,
    /// Norma codificata per documento (assente se il campo non ha termini).
    norms: HashMap<u32, u8>,
    sum_total_term_freq: u64,
}

impl FieldIndex {
    fn add(&mut self, doc: u32, text: &str, analyzer: Analyzer) {
        let tokens = analyzer.analyze(text);
        if tokens.is_empty() {
            return;
        }
        let mut by_term: HashMap<String, Vec<u32>> = HashMap::new();
        for token in &tokens {
            by_term
                .entry(token.text.clone())
                .or_default()
                .push(token.position);
        }
        for (term, positions) in by_term {
            self.terms
                .entry(term)
                .or_default()
                .push(Posting { doc, positions });
        }
        self.norms.insert(doc, int_to_byte4(tokens.len() as u32));
        self.sum_total_term_freq += tokens.len() as u64;
    }

    /// Documenti con almeno un termine nel campo (`Terms.getDocCount`).
    pub(crate) fn doc_count(&self) -> u64 {
        self.norms.len() as u64
    }

    pub(crate) fn postings(&self, term: &str) -> &[Posting] {
        self.terms.get(term).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn norm(&self, doc: u32) -> u8 {
        self.norms.get(&doc).copied().unwrap_or(0)
    }

    /// `avgFieldLength`.
    pub(crate) fn avgdl(&self) -> f32 {
        (self.sum_total_term_freq as f64 / self.doc_count() as f64) as f32
    }

    /// `1 / (k1 * ((1 - b) + b * dl / avgdl))` per ognuna delle 256 norme.
    pub(crate) fn norm_cache(&self) -> [f32; 256] {
        let avgdl = self.avgdl();
        let mut cache = [0f32; 256];
        for (index, entry) in cache.iter_mut().enumerate() {
            *entry = 1f32 / (K1 * ((1f32 - B) + B * byte4_to_int(index as u8) as f32 / avgdl));
        }
        cache
    }
}

/// Un chunk indicizzato con i campi memorizzati che servono ai risultati.
#[derive(Debug, Clone)]
pub(crate) struct StoredChunk {
    pub parent_id: String,
    /// Il testo del chunk (anche quando è finito in `content_en`).
    pub text: String,
}

#[derive(Debug, Default)]
pub(crate) struct Bm25Index {
    fields: HashMap<Field, FieldIndex>,
    pub docs: Vec<StoredChunk>,
}

impl Bm25Index {
    pub(crate) fn add(&mut self, field_texts: &[(Field, &str)], stored: StoredChunk) {
        let doc = self.docs.len() as u32;
        for (field, text) in field_texts {
            self.fields
                .entry(*field)
                .or_default()
                .add(doc, text, field.analyzer());
        }
        self.docs.push(stored);
    }

    pub(crate) fn field(&self, field: Field) -> Option<&FieldIndex> {
        self.fields.get(&field)
    }
}

/// `idf = ln(1 + (docCount - docFreq + 0.5) / (docFreq + 0.5))`.
pub(crate) fn idf(doc_freq: u64, doc_count: u64) -> f32 {
    (1.0 + (doc_count as f64 - doc_freq as f64 + 0.5) / (doc_freq as f64 + 0.5)).ln() as f32
}

/// `BM25Scorer.score`: `weight - weight / (1 + freq * normInverse)`.
pub(crate) fn bm25(weight: f32, freq: f32, norm_inverse: f32) -> f32 {
    weight - weight / (1f32 + freq * norm_inverse)
}

const MAX_INT4: u32 = 231;
const NUM_FREE_VALUES: u32 = 255 - MAX_INT4;

fn long_to_int4(i: u64) -> u32 {
    let bits = 64 - i.leading_zeros();
    if bits < 4 {
        i as u32
    } else {
        let shift = bits - 4;
        let mut encoded = (i >> shift) as u32 & 0x07;
        encoded |= (shift + 1) << 3;
        encoded
    }
}

fn int4_to_long(i: u32) -> u64 {
    let bits = u64::from(i & 0x07);
    let shift = (i >> 3) as i32 - 1;
    if shift == -1 {
        bits
    } else {
        (bits | 0x08) << shift
    }
}

/// `SmallFloat.intToByte4`: interi piccoli esatti, gli altri con 4 bit significativi.
pub(crate) fn int_to_byte4(i: u32) -> u8 {
    if i < NUM_FREE_VALUES {
        i as u8
    } else {
        (NUM_FREE_VALUES + long_to_int4(u64::from(i - NUM_FREE_VALUES))) as u8
    }
}

/// `SmallFloat.byte4ToInt`.
pub(crate) fn byte4_to_int(b: u8) -> u32 {
    let i = u32::from(b);
    if i < NUM_FREE_VALUES {
        i
    } else {
        (u64::from(NUM_FREE_VALUES) + int4_to_long(i - NUM_FREE_VALUES)).min(i32::MAX as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_float_encodes_lengths_like_lucene() {
        assert_eq!(long_to_int4(i32::MAX as u64), MAX_INT4);
        for length in [0, 1, 23, 24, 25, 100, 761, 768, 5000] {
            let decoded = byte4_to_int(int_to_byte4(length));
            assert!(decoded <= length, "{length} -> {decoded}");
        }
        assert_eq!(byte4_to_int(int_to_byte4(23)), 23);
        assert_eq!(byte4_to_int(int_to_byte4(761)), 728);
    }
}
