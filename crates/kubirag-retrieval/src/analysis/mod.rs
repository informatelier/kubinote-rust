//! Gli analyzer di Lucene 10.4 usati da KubiRAG, riscritti con la stessa semantica:
//!
//! | Analyzer | Catena |
//! |---|---|
//! | `ItalianAnalyzer` | StandardTokenizer → Elision → LowerCase → Stop (279) → ItalianLightStem |
//! | `EnglishAnalyzer` | StandardTokenizer → EnglishPossessive → LowerCase → Stop (33) → Porter |
//!
//! Le posizioni conservano i buchi lasciati dalle stopword, come gli incrementi di posizione
//! Lucene: servono alle phrase query.

mod english;
mod italian;
mod stopwords;
mod tokenizer;

pub use tokenizer::{Token, MAX_TOKEN_LENGTH};

/// Analyzer per campo: `content` e `title` usano l'italiano, `content_en` l'inglese.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Analyzer {
    Italian,
    English,
}

impl Analyzer {
    pub fn analyze(self, text: &str) -> Vec<Token> {
        tokenizer::standard_tokens(text)
            .into_iter()
            .filter_map(|raw| {
                let term = match self {
                    Self::Italian => {
                        let lower = lowercase(italian::strip_elision(raw.text));
                        (!stopwords::ITALIAN.contains(&lower.as_str()))
                            .then(|| italian::light_stem(&lower))
                    }
                    Self::English => {
                        let lower = lowercase(english::strip_possessive(raw.text));
                        (!stopwords::ENGLISH.contains(&lower.as_str()))
                            .then(|| english::porter_stem(&lower))
                    }
                }?;
                Some(Token {
                    text: term,
                    position: raw.position,
                })
            })
            .collect()
    }

    /// Solo i termini, nell'ordine.
    pub fn terms(self, text: &str) -> Vec<String> {
        self.analyze(text).into_iter().map(|t| t.text).collect()
    }
}

/// `LowerCaseFilter`: `Character.toLowerCase` per code point (mappatura semplice: `İ` → `i`).
pub(crate) fn lowercase(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{130}' => 'i',
            c => {
                let mut lower = c.to_lowercase();
                match (lower.next(), lower.next()) {
                    (Some(single), None) => single,
                    _ => c,
                }
            }
        })
        .collect()
}
