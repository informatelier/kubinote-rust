//! Primitive di testo con la semantica di Kotlin/JVM: le lunghezze sono in unità UTF-16 e gli
//! spazi sono quelli di `Char.isWhitespace`.

/// Lunghezza in unità UTF-16 (`String.length`).
pub fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// `String.take(n)`: al più `n` unità UTF-16 (una coppia surrogata non viene spezzata).
pub fn take_utf16(text: &str, n: usize) -> &str {
    let mut units = 0;
    for (index, c) in text.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &text[..index];
        }
    }
    text
}

/// `Char.isWhitespace` (`Character.isWhitespace || Character.isSpaceChar`).
pub fn is_kotlin_whitespace(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || matches!(c, '\u{1C}'..='\u{1F}')
}

/// `String.lines()`: separa su `\r\n`, `\n` e `\r`.
pub fn kotlin_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(&text[start..index]);
                start = index + 1;
            }
            b'\r' => {
                lines.push(&text[start..index]);
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    lines.push(&text[start..]);
    lines
}

/// `String.trimIndent()`: toglie l'indentazione minima delle righe non vuote, e la prima e
/// l'ultima riga se vuote. Le righe sono riunite con `\n`.
pub fn kotlin_trim_indent(text: &str) -> String {
    let lines = kotlin_lines(text);
    let is_blank = |line: &str| line.chars().all(is_kotlin_whitespace);
    let indent = lines
        .iter()
        .filter(|line| !is_blank(line))
        .map(|line| {
            line.chars()
                .take_while(|c| is_kotlin_whitespace(*c))
                .count()
        })
        .min()
        .unwrap_or(0);
    let last = lines.len() - 1;
    lines
        .iter()
        .enumerate()
        .filter(|(index, line)| !((*index == 0 || *index == last) && is_blank(line)))
        .map(|(_, line)| line.chars().skip(indent).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_counts_utf16_units() {
        assert_eq!(take_utf16("città", 4), "citt");
        assert_eq!(
            take_utf16("a😀b", 2),
            "a",
            "la coppia surrogata non si spezza"
        );
        assert_eq!(take_utf16("a😀b", 3), "a😀");
        assert_eq!(utf16_len("a😀"), 3);
    }

    #[test]
    fn lines_split_like_kotlin() {
        assert_eq!(kotlin_lines("a\r\nb\rc\n"), ["a", "b", "c", ""]);
        assert_eq!(kotlin_lines(""), [""]);
    }

    #[test]
    fn trim_indent_uses_the_minimum_indent_of_non_blank_lines() {
        assert_eq!(
            kotlin_trim_indent("\n    |a\n      b\n  \n    c\n  "),
            "|a\n  b\n\nc"
        );
        assert_eq!(
            kotlin_trim_indent("\n    a\nb\n"),
            "    a\nb",
            "una riga a colonna 0 annulla il taglio"
        );
    }
}
