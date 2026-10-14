//! `CommandLineTokenizer`: il `DefaultParser` di JLine con le virgolette doppie come
//! delimitatori, più la normalizzazione degli argomenti tra apici singoli e della forma
//! storica `''testo''`.

use kubirag_domain::retrieval::text::is_kotlin_whitespace;

const QUOTE: char = '"';
const ESCAPE: char = '\\';

/// Divide una riga in parole. Una virgoletta non chiusa è un errore, come
/// `eofOnUnclosedQuote(true)`.
pub fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let normalized = normalize_single_quoted_arguments(&normalize_legacy_quotes(line)?)?;
    parse(&normalized)
}

/// `DefaultParser.parse(line, line.length, ACCEPT_LINE).words()`.
fn parse(line: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = line.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote_start: Option<usize> = None;
    let mut quoted_word = false;
    for (i, &c) in chars.iter().enumerate() {
        if quote_start.is_none() && is_quote_char(&chars, i) {
            quote_start = Some(i);
            if current.is_empty() {
                quoted_word = true;
            } else {
                current.push(c);
            }
        } else if quote_start.is_some_and(|start| chars[start] == c) && !is_escaped(&chars, i) {
            if !quoted_word {
                current.push(c);
            }
            quote_start = None;
            quoted_word = false;
        } else if quote_start.is_none() && is_delimiter(&chars, i) {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        } else if !is_escape_char(&chars, i) {
            current.push(c);
        }
    }
    if quote_start.is_some() {
        return Err("Missing closing quote".into());
    }
    // Il cursore è a fine riga: JLine aggiunge sempre l'ultima parola, anche vuota.
    words.push(current);
    if words.len() == 1 && words[0].is_empty() {
        words.clear();
    }
    Ok(words)
}

fn is_escape_char(chars: &[char], pos: usize) -> bool {
    chars[pos] == ESCAPE && !is_escaped(chars, pos)
}

fn is_escaped(chars: &[char], pos: usize) -> bool {
    pos > 0 && is_escape_char(chars, pos - 1)
}

fn is_quote_char(chars: &[char], pos: usize) -> bool {
    chars[pos] == QUOTE && !is_escaped(chars, pos)
}

fn is_delimiter(chars: &[char], pos: usize) -> bool {
    !is_escaped(chars, pos) && is_java_whitespace(chars[pos])
}

/// `Character.isWhitespace`: separatori Unicode tranne gli spazi non separabili, più i
/// controlli di spaziatura ASCII.
fn is_java_whitespace(c: char) -> bool {
    match c {
        '\u{00A0}' | '\u{2007}' | '\u{202F}' => false,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | '\u{1C}'..='\u{1F}' => true,
        _ => is_kotlin_whitespace(c) && !c.is_ascii_control() && c != '\u{85}',
    }
}

fn quote_for_jline(content: &str) -> String {
    format!("\"{}\"", content.replace('\\', "\\\\").replace('"', "\\\""))
}

/// La posizione è dentro un argomento tra virgolette doppie (con gli escape).
fn is_inside_double_quoted_argument(line: &str, position: usize) -> bool {
    let mut inside = false;
    let mut escaped = false;
    for c in line[..position].chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            inside = !inside;
        }
    }
    inside
}

fn preceded_by_boundary(line: &str, index: usize) -> bool {
    line[..index]
        .chars()
        .next_back()
        .is_none_or(is_kotlin_whitespace)
}

fn followed_by_boundary(line: &str, index: usize) -> bool {
    line[index..]
        .chars()
        .next()
        .is_none_or(is_kotlin_whitespace)
}

/// Sostituisce gli argomenti `open…close` che iniziano e finiscono a un confine di parola.
fn normalize_quoted(line: &str, delimiter: &str, unclosed: &str) -> Result<String, String> {
    if !line.contains(delimiter) {
        return Ok(line.to_owned());
    }
    let mut normalized = String::with_capacity(line.len());
    let mut cursor = 0;
    while cursor < line.len() {
        let opening = line[cursor..]
            .match_indices(delimiter)
            .map(|(offset, _)| cursor + offset)
            .find(|&candidate| {
                preceded_by_boundary(line, candidate)
                    && !is_inside_double_quoted_argument(line, candidate)
            });
        let Some(opening) = opening else {
            normalized.push_str(&line[cursor..]);
            break;
        };
        normalized.push_str(&line[cursor..opening]);
        let content_start = opening + delimiter.len();
        let closing = line[content_start..]
            .match_indices(delimiter)
            .map(|(offset, _)| content_start + offset)
            .find(|&candidate| followed_by_boundary(line, candidate + delimiter.len()))
            .ok_or_else(|| unclosed.to_owned())?;
        normalized.push_str(&quote_for_jline(&line[content_start..closing]));
        cursor = closing + delimiter.len();
    }
    Ok(normalized)
}

fn normalize_legacy_quotes(line: &str) -> Result<String, String> {
    normalize_quoted(line, "''", "Unclosed legacy quoted argument.")
}

fn normalize_single_quoted_arguments(line: &str) -> Result<String, String> {
    normalize_quoted(line, "'", "Unclosed single-quoted argument.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        tokenize(line).unwrap()
    }

    #[test]
    fn words_split_on_whitespace_and_double_quotes_group() {
        assert_eq!(words("/stat  a   b"), ["/stat", "a", "b"]);
        assert_eq!(words("/load \"my file.md\""), ["/load", "my file.md"]);
        assert_eq!(words("a\\ b"), ["a b"]);
        assert_eq!(words("say \"x \\\" y\""), ["say", "x \" y"]);
        assert!(words("").is_empty());
    }

    #[test]
    fn a_quote_inside_a_word_stays_literal() {
        assert_eq!(words("a\"b c\" d"), ["a\"b c\"", "d"]);
    }

    #[test]
    fn single_quotes_are_normalized_only_at_word_boundaries() {
        assert_eq!(words("/load 'my file.md'"), ["/load", "my file.md"]);
        assert_eq!(
            words("l'architettura di kubi"),
            ["l'architettura", "di", "kubi"]
        );
        assert_eq!(words("/x ''legacy form''"), ["/x", "legacy form"]);
    }

    #[test]
    fn unclosed_quotes_are_errors() {
        assert_eq!(tokenize("a \"b").unwrap_err(), "Missing closing quote");
        assert_eq!(
            tokenize("a 'b").unwrap_err(),
            "Unclosed single-quoted argument."
        );
    }
}
