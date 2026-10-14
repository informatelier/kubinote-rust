//! Filtri dell'`ItalianAnalyzer`: `ElisionFilter` e `ItalianLightStemmer`.

/// Articoli elisi di `ItalianAnalyzer.DEFAULT_ARTICLES`.
const ARTICLES: [&str; 21] = [
    "c", "l", "all", "dall", "dell", "nell", "sull", "coll", "pell", "gl", "agl", "dagl", "degl",
    "negl", "sugl", "un", "m", "t", "s", "v", "d",
];

/// Toglie l'articolo prima del primo apostrofo (`'` o `’`), senza distinzione di maiuscole.
pub(crate) fn strip_elision(token: &str) -> &str {
    let Some(index) = token.find(['\'', '\u{2019}']) else {
        return token;
    };
    let article = super::lowercase(&token[..index]);
    if ARTICLES.contains(&article.as_str()) {
        let apostrophe = token[index..].chars().next().map_or(1, char::len_utf8);
        &token[index + apostrophe..]
    } else {
        token
    }
}

/// `ItalianLightStemmer.stem`: solo parole di almeno 6 unità UTF-16; normalizza gli accenti e
/// toglie la vocale finale (o la coppia `ie/he/hi/ii/ia/io`).
pub(crate) fn light_stem(token: &str) -> String {
    let mut s: Vec<u16> = token.encode_utf16().collect();
    let len = s.len();
    if len < 6 {
        return token.to_owned();
    }
    for unit in s.iter_mut() {
        *unit = match char::from_u32(u32::from(*unit)) {
            Some('à' | 'á' | 'â' | 'ä') => 'a' as u16,
            Some('ò' | 'ó' | 'ô' | 'ö') => 'o' as u16,
            Some('è' | 'é' | 'ê' | 'ë') => 'e' as u16,
            Some('ù' | 'ú' | 'û' | 'ü') => 'u' as u16,
            Some('ì' | 'í' | 'î' | 'ï') => 'i' as u16,
            _ => *unit,
        };
    }
    let last = s[len - 1];
    let previous = s[len - 2];
    let is = |unit: u16, c: char| unit == c as u16;
    // `e`/`i` cadono con un `i` o `h` precedente; `a`/`o` solo con un `i` precedente.
    let keep = if is(last, 'e') || is(last, 'i') {
        if is(previous, 'i') || is(previous, 'h') {
            len - 2
        } else {
            len - 1
        }
    } else if is(last, 'a') || is(last, 'o') {
        if is(previous, 'i') {
            len - 2
        } else {
            len - 1
        }
    } else {
        len
    };
    String::from_utf16_lossy(&s[..keep])
}
