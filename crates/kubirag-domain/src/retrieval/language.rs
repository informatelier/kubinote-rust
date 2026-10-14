/// Lingua rilevata da [`detect_language`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Italian,
    English,
    Unknown,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Self::Italian => "ITALIAN",
            Self::English => "ENGLISH",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LanguageDetection {
    pub language: Language,
    /// Quota di stopword della lingua dominante sul totale delle stopword riconosciute.
    pub confidence: f32,
}

impl LanguageDetection {
    const UNKNOWN: Self = Self {
        language: Language::Unknown,
        confidence: 0.0,
    };

    /// Il retrieval indicizza come inglese solo un testo inglese con confidenza ≥ 0.60.
    pub fn is_confident_english(&self) -> bool {
        self.language == Language::English && self.confidence >= CONFIDENCE_THRESHOLD
    }
}

const MIN_WORDS: usize = 5;
pub const CONFIDENCE_THRESHOLD: f32 = 0.60;

const ITALIAN_STOPWORDS: &[&str] = &[
    "il", "lo", "la", "i", "gli", "le", "un", "uno", "una", "e", "di", "a", "da", "in", "con",
    "su", "per", "tra", "fra", "che", "non", "si", "ma", "come", "questo", "questa", "questi",
    "queste", "essere", "sono", "anche", "più", "già", "molto", "quando", "dove", "tutto", "tutti",
    "tutte", "cosa", "dopo", "prima", "sempre", "ogni", "dei", "delle", "degli", "del", "della",
    "al", "alla", "agli", "alle", "dal", "dalla", "dagli", "dalle", "nel", "nella", "negli",
    "nelle", "sul", "sulla", "sugli", "sulle", "col", "coi", "se", "ho", "hai", "ha", "abbiamo",
    "hanno", "era", "erano", "sarà", "saranno", "ci", "ne", "li", "mi", "ti", "vi", "ce", "te",
    "lui", "lei", "noi", "voi", "loro", "quello", "quella", "quelli", "quelle", "stesso", "stessa",
];

const ENGLISH_STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "are", "was", "were", "be", "been", "have", "has", "had", "do", "does", "did",
    "not", "this", "that", "it", "as", "its", "we", "you", "they", "he", "she", "which", "who",
    "when", "where", "what", "how", "all", "each", "more", "also", "than", "then", "can", "will",
    "would", "should", "their", "there", "these", "those", "such", "into", "about", "after", "if",
    "so", "my", "your", "our", "his", "her", "any", "some", "no", "up", "out", "one", "two",
    "three", "may", "could", "must",
];

/// `LanguageDetector.detect`: conta le stopword italiane e inglesi.
///
/// Come in Kotlin le parole sono separate da `\W+` di Java, che è ASCII: `più` diventa `pi`, e
/// le stopword accentate non vengono mai riconosciute. Il comportamento è replicato apposta,
/// perché decide in quale campo (e con quale analyzer) finisce ogni chunk.
pub fn detect_language(text: &str) -> LanguageDetection {
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| word.encode_utf16().count() > 1)
        .collect();
    if words.len() < MIN_WORDS {
        return LanguageDetection::UNKNOWN;
    }
    let italian = words
        .iter()
        .filter(|w| ITALIAN_STOPWORDS.contains(w))
        .count();
    let english = words
        .iter()
        .filter(|w| ENGLISH_STOPWORDS.contains(w))
        .count();
    let total = italian + english;
    let detection = |language, hits: usize| LanguageDetection {
        language,
        confidence: hits as f32 / total as f32,
    };
    match italian.cmp(&english) {
        _ if total == 0 => LanguageDetection::UNKNOWN,
        std::cmp::Ordering::Greater => detection(Language::Italian, italian),
        std::cmp::Ordering::Less => detection(Language::English, english),
        std::cmp::Ordering::Equal => LanguageDetection::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_texts_are_unknown() {
        assert_eq!(detect_language("the a an").language, Language::Unknown);
    }

    #[test]
    fn the_dominant_stopwords_win() {
        let detection = detect_language("the notebook and the sources of the project");
        assert!(detection.is_confident_english());
        assert_eq!(
            detect_language("il notebook e le note della guida").language,
            Language::Italian
        );
    }
}
