//! Filtri dell'`EnglishAnalyzer`: `EnglishPossessiveFilter` e `PorterStemmer` (l'algoritmo
//! originale di Porter, come la versione Java di Lucene: non è lo Snowball "Porter2").

/// Toglie `'s`, `’s` o `＇s` finali.
pub(crate) fn strip_possessive(token: &str) -> &str {
    for apostrophe in ['\'', '\u{2019}', '\u{FF07}'] {
        for s in ['s', 'S'] {
            let suffix = format!("{apostrophe}{s}");
            if let Some(stripped) = token.strip_suffix(suffix.as_str()) {
                return stripped;
            }
        }
    }
    token
}

pub(crate) fn porter_stem(token: &str) -> String {
    let b: Vec<u16> = token.encode_utf16().collect();
    if b.len() <= 2 {
        return token.to_owned();
    }
    let mut stemmer = Porter {
        k: b.len() as isize - 1,
        j: 0,
        b,
    };
    stemmer.step1();
    stemmer.step2();
    stemmer.step3();
    stemmer.step4();
    stemmer.step5();
    stemmer.step6();
    String::from_utf16_lossy(&stemmer.b[..=stemmer.k as usize])
}

/// Lo stato di `PorterStemmer` di Lucene: `b[0..=k]` è la parola, `j` l'ultimo indice prima
/// del suffisso riconosciuto da [`Porter::ends`] (può valere -1).
struct Porter {
    b: Vec<u16>,
    k: isize,
    j: isize,
}

fn unit(c: char) -> u16 {
    c as u16
}

impl Porter {
    fn at(&self, i: isize) -> u16 {
        self.b[i as usize]
    }

    fn cons(&self, i: isize) -> bool {
        match char::from_u32(u32::from(self.at(i))) {
            Some('a' | 'e' | 'i' | 'o' | 'u') => false,
            Some('y') => i == 0 || !self.cons(i - 1),
            _ => true,
        }
    }

    /// Numero di sequenze vocale-consonante in `b[0..=j]`.
    fn m(&self) -> usize {
        let mut n = 0;
        let mut i = 0;
        loop {
            if i > self.j {
                return n;
            }
            if !self.cons(i) {
                break;
            }
            i += 1;
        }
        i += 1;
        loop {
            loop {
                if i > self.j {
                    return n;
                }
                if self.cons(i) {
                    break;
                }
                i += 1;
            }
            i += 1;
            n += 1;
            loop {
                if i > self.j {
                    return n;
                }
                if !self.cons(i) {
                    break;
                }
                i += 1;
            }
            i += 1;
        }
    }

    fn vowel_in_stem(&self) -> bool {
        (0..=self.j).any(|i| !self.cons(i))
    }

    fn double_consonant(&self, j: isize) -> bool {
        j >= 1 && self.at(j) == self.at(j - 1) && self.cons(j)
    }

    /// consonante-vocale-consonante, con l'ultima diversa da w, x, y.
    fn cvc(&self, i: isize) -> bool {
        if i < 2 || !self.cons(i) || self.cons(i - 1) || !self.cons(i - 2) {
            return false;
        }
        let c = self.at(i);
        !(c == unit('w') || c == unit('x') || c == unit('y'))
    }

    fn ends(&mut self, suffix: &str) -> bool {
        let suffix: Vec<u16> = suffix.encode_utf16().collect();
        let len = suffix.len() as isize;
        let offset = self.k - len + 1;
        if offset < 0 || self.b[offset as usize..=self.k as usize] != suffix[..] {
            return false;
        }
        self.j = self.k - len;
        true
    }

    fn set_to(&mut self, replacement: &str) {
        self.b.truncate((self.j + 1) as usize);
        self.b.extend(replacement.encode_utf16());
        self.k = self.j + replacement.len() as isize;
    }

    fn r(&mut self, replacement: &str) {
        if self.m() > 0 {
            self.set_to(replacement);
        }
    }

    fn last_is(&self, c: char) -> bool {
        self.at(self.k) == unit(c)
    }

    /// Plurali e -ed/-ing.
    fn step1(&mut self) {
        if self.last_is('s') {
            if self.ends("sses") {
                self.k -= 2;
            } else if self.ends("ies") {
                self.set_to("i");
            } else if self.at(self.k - 1) != unit('s') {
                self.k -= 1;
            }
        }
        if self.ends("eed") {
            if self.m() > 0 {
                self.k -= 1;
            }
        } else if (self.ends("ed") || self.ends("ing")) && self.vowel_in_stem() {
            self.k = self.j;
            if self.ends("at") {
                self.set_to("ate");
            } else if self.ends("bl") {
                self.set_to("ble");
            } else if self.ends("iz") {
                self.set_to("ize");
            } else if self.double_consonant(self.k) {
                let c = self.at(self.k);
                if !(c == unit('l') || c == unit('s') || c == unit('z')) {
                    self.k -= 1;
                }
            } else if self.m() == 1 && self.cvc(self.k) {
                self.set_to("e");
            }
        }
    }

    /// y → i quando c'è un'altra vocale nella radice.
    fn step2(&mut self) {
        if self.ends("y") && self.vowel_in_stem() {
            let k = self.k as usize;
            self.b[k] = unit('i');
        }
    }

    /// Suffissi doppi → singoli (-ization → -ize…).
    fn step3(&mut self) {
        if self.k == 0 {
            return;
        }
        let rules: &[(&str, &str)] = match char::from_u32(u32::from(self.at(self.k - 1))) {
            Some('a') => &[("ational", "ate"), ("tional", "tion")],
            Some('c') => &[("enci", "ence"), ("anci", "ance")],
            Some('e') => &[("izer", "ize")],
            Some('l') => &[
                ("bli", "ble"),
                ("alli", "al"),
                ("entli", "ent"),
                ("eli", "e"),
                ("ousli", "ous"),
            ],
            Some('o') => &[("ization", "ize"), ("ation", "ate"), ("ator", "ate")],
            Some('s') => &[
                ("alism", "al"),
                ("iveness", "ive"),
                ("fulness", "ful"),
                ("ousness", "ous"),
            ],
            Some('t') => &[("aliti", "al"), ("iviti", "ive"), ("biliti", "ble")],
            Some('g') => &[("logi", "log")],
            _ => &[],
        };
        self.apply_first(rules);
    }

    /// -ic-, -full, -ness…
    fn step4(&mut self) {
        let rules: &[(&str, &str)] = match char::from_u32(u32::from(self.at(self.k))) {
            Some('e') => &[("icate", "ic"), ("ative", ""), ("alize", "al")],
            Some('i') => &[("iciti", "ic")],
            Some('l') => &[("ical", "ic"), ("ful", "")],
            Some('s') => &[("ness", "")],
            _ => &[],
        };
        self.apply_first(rules);
    }

    fn apply_first(&mut self, rules: &[(&str, &str)]) {
        for (suffix, replacement) in rules {
            if self.ends(suffix) {
                self.r(replacement);
                return;
            }
        }
    }

    /// -ant, -ence… quando m > 1.
    fn step5(&mut self) {
        if self.k == 0 {
            return;
        }
        let matched = match char::from_u32(u32::from(self.at(self.k - 1))) {
            Some('a') => self.ends("al"),
            Some('c') => self.ends("ance") || self.ends("ence"),
            Some('e') => self.ends("er"),
            Some('i') => self.ends("ic"),
            Some('l') => self.ends("able") || self.ends("ible"),
            Some('n') => {
                self.ends("ant") || self.ends("ement") || self.ends("ment") || self.ends("ent")
            }
            Some('o') => {
                (self.ends("ion")
                    && self.j >= 0
                    && (self.at(self.j) == unit('s') || self.at(self.j) == unit('t')))
                    || self.ends("ou")
            }
            Some('s') => self.ends("ism"),
            Some('t') => self.ends("ate") || self.ends("iti"),
            Some('u') => self.ends("ous"),
            Some('v') => self.ends("ive"),
            Some('z') => self.ends("ize"),
            _ => false,
        };
        if matched && self.m() > 1 {
            self.k = self.j;
        }
    }

    /// -e finale e -ll.
    fn step6(&mut self) {
        self.j = self.k;
        if self.last_is('e') {
            let a = self.m();
            if a > 1 || (a == 1 && !self.cvc(self.k - 1)) {
                self.k -= 1;
            }
        }
        if self.last_is('l') && self.double_consonant(self.k) && self.m() > 1 {
            self.k -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porter_reference_words() {
        for (word, stem) in [
            ("caresses", "caress"),
            ("ponies", "poni"),
            ("cats", "cat"),
            ("feed", "feed"),
            ("agreed", "agre"),
            ("plastered", "plaster"),
            ("motoring", "motor"),
            ("sing", "sing"),
            ("conflated", "conflat"),
            ("troubled", "troubl"),
            ("sized", "size"),
            ("hopping", "hop"),
            ("falling", "fall"),
            ("filing", "file"),
            ("happy", "happi"),
            ("relational", "relat"),
            ("conditional", "condit"),
            ("generalization", "gener"),
            ("electrical", "electr"),
            ("hopefulness", "hope"),
            ("adjustment", "adjust"),
            ("adoption", "adopt"),
            ("controll", "control"),
            ("roll", "roll"),
            ("probate", "probat"),
            ("rate", "rate"),
            ("easily", "easili"),
            ("is", "is"),
        ] {
            assert_eq!(porter_stem(word), stem, "{word}");
        }
    }
}
