//! Formattazione come la JVM: `"%.Nf".format(x)` e `padStart`/`padEnd`/`takeLast` di Kotlin,
//! che contano in unità UTF-16.

use kubirag_domain::retrieval::text::{take_utf16, utf16_len};

/// `String.format("%.Nf", value)` in `Locale.ROOT`: le cifre più corte che identificano il
/// `double` (`Double.toString`), poi arrotondamento `HALF_UP` a `decimals` cifre.
pub fn java_fixed(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust scientific notation always has an exponent");
    let exponent: i32 = exponent.parse().expect("integer exponent");
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect();
    // Cifre prima della virgola; con un esponente negativo si antepongono zeri.
    let mut integer_digits = exponent + 1;
    if integer_digits <= 0 {
        let zeros = usize::try_from(1 - integer_digits).unwrap_or(0);
        digits.splice(0..0, std::iter::repeat_n(0, zeros));
        integer_digits = 1;
    }
    let mut integer_digits = usize::try_from(integer_digits).unwrap_or(1);
    let cut = integer_digits + decimals;
    if digits.len() <= cut {
        digits.resize(cut, 0);
    } else {
        let round_up = digits[cut] >= 5;
        digits.truncate(cut);
        if round_up && carry(&mut digits) {
            digits.insert(0, 1);
            integer_digits += 1;
        }
    }
    let mut text = String::with_capacity(digits.len() + 2);
    if value.is_sign_negative() {
        text.push('-');
    }
    for (index, digit) in digits.iter().enumerate() {
        if index == integer_digits {
            text.push('.');
        }
        text.push(char::from(b'0' + digit));
    }
    text
}

/// Somma uno all'ultima cifra; `true` se il riporto esce dalla prima.
fn carry(digits: &mut [u8]) -> bool {
    for digit in digits.iter_mut().rev() {
        if *digit == 9 {
            *digit = 0;
        } else {
            *digit += 1;
            return false;
        }
    }
    true
}

/// `padEnd(length)`.
pub fn pad_end(text: &str, length: usize) -> String {
    let missing = length.saturating_sub(utf16_len(text));
    format!("{text}{}", " ".repeat(missing))
}

/// `padStart(length)`.
pub fn pad_start(text: &str, length: usize) -> String {
    let missing = length.saturating_sub(utf16_len(text));
    format!("{}{text}", " ".repeat(missing))
}

/// `take(n)`.
pub fn take(text: &str, n: usize) -> &str {
    take_utf16(text, n)
}

/// `takeLast(n)`.
pub fn take_last(text: &str, n: usize) -> &str {
    let mut units = 0;
    for (index, c) in text.char_indices().rev() {
        units += c.len_utf16();
        if units > n {
            return &text[index + c.len_utf8()..];
        }
    }
    text
}

/// Un percorso accorciato a `width` unità con `…` davanti, oppure allineato a sinistra.
pub fn short_path(path: &str, width: usize) -> String {
    if utf16_len(path) > width {
        format!("…{}", take_last(path, width - 1))
    } else {
        pad_end(path, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_rounds_half_up_on_the_shortest_digits() {
        assert_eq!(java_fixed(0.125, 2), "0.13");
        assert_eq!(java_fixed(0.15, 1), "0.2");
        assert_eq!(java_fixed(2.5, 0), "3");
        assert_eq!(java_fixed(9.99995, 4), "10.0000");
        assert_eq!(java_fixed(0.00004, 4), "0.0000");
        assert_eq!(java_fixed(0.00005, 4), "0.0001");
        assert_eq!(java_fixed(12.0, 1), "12.0");
        assert_eq!(java_fixed(-1.25, 1), "-1.3");
        assert_eq!(java_fixed(0.0, 3), "0.000");
        assert_eq!(java_fixed(f64::from(1.2345678_f32), 4), "1.2346");
        // Un `Float` è allargato a `double` prima della formattazione (verificato su Java 21).
        assert_eq!(java_fixed(f64::from(1.00005_f32), 4), "1.0000");
        assert_eq!(java_fixed(f64::from(0.1235_f32), 3), "0.123");
    }

    #[test]
    fn padding_and_truncation_count_utf16_units() {
        assert_eq!(pad_end("ab", 4), "ab  ");
        assert_eq!(pad_start("7", 2), " 7");
        assert_eq!(take_last("abcdef", 3), "def");
        assert_eq!(take_last("a😀", 2), "😀");
        assert_eq!(short_path("abcdef", 4), "…def");
        assert_eq!(short_path("ab", 4), "ab  ");
    }
}
