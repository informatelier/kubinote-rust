//! Locator delle risorse del notebook e regole di portabilità del writer Kotlin
//! (`RagNotebookManifestRepository.portableResource`), nella parte che non tocca il disco.
//!
//! Un manifest deve poter essere copiato su un'altra macchina: gli URL remoti non possono
//! contenere credenziali, i locator `bound://` vengono normalizzati, i percorsi locali vengono
//! resi relativi allo store dall'adattatore di persistenza.

use kubi_kernel::identity::ResourceUri;

use super::NotebookError;

const BOUND_SCHEME: &str = "bound://";

/// Classificazione di un locator, dopo il `trim()` che il writer applica al percorso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortableLocator {
    /// `http(s)://`, senza credenziali: la risorsa resta `EXTERNAL`.
    Remote(String),
    /// `bound://<bound>/<percorso>` normalizzato: la risorsa resta `EXTERNAL`.
    Bound(String),
    /// Percorso di file, assoluto o relativo allo store: lo risolve l'adattatore.
    Local(String),
}

impl PortableLocator {
    pub fn classify(path: &str) -> Result<Self, NotebookError> {
        let path = path.trim();
        if is_remote(path) {
            return portable_remote(path).map(Self::Remote);
        }
        if is_bound(path) {
            return portable_bound(path).map(Self::Bound);
        }
        Ok(Self::Local(path.to_owned()))
    }
}

/// `startsWith("http://", true) || startsWith("https://", true)`.
pub fn is_remote(path: &str) -> bool {
    starts_with_ignore_case(path, "http://") || starts_with_ignore_case(path, "https://")
}

/// `isBoundResourceLocator`.
pub fn is_bound(path: &str) -> bool {
    starts_with_ignore_case(path, BOUND_SCHEME)
}

fn starts_with_ignore_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

const SECRET_KEYS: [&str; 7] = [
    "token",
    "key",
    "api_key",
    "apikey",
    "secret",
    "password",
    "signature",
];

fn portable_remote(path: &str) -> Result<String, NotebookError> {
    if ResourceUri::parse(path).is_err() {
        return Err(NotebookError::Rejected(
            "Invalid remote resource URL".to_owned(),
        ));
    }
    let after_scheme = path.split_once("://").map_or("", |(_, rest)| rest);
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let has_user_info = after_scheme[..authority_end]
        .split_once('@')
        .is_some_and(|(user_info, _)| !user_info.is_empty());
    let query = path
        .split_once('?')
        .map_or("", |(_, rest)| rest.split('#').next().unwrap_or(""));
    let has_secret = percent_decode(query).split('&').any(|parameter| {
        let key = parameter
            .split('=')
            .next()
            .unwrap_or("")
            .to_lowercase()
            .replace('-', "_");
        SECRET_KEYS.contains(&key.as_str())
            || key.ends_with("_token")
            || key.ends_with("_key")
            || key.ends_with("_signature")
    });
    if has_user_info || has_secret {
        return Err(NotebookError::Rejected(
            "RAG_NON_PORTABLE_SECRET_REFERENCE: remote URL contains credentials".to_owned(),
        ));
    }
    Ok(path.to_owned())
}

fn portable_bound(path: &str) -> Result<String, NotebookError> {
    let locator = &path[BOUND_SCHEME.len()..];
    let (bound_name, relative_path) = locator.split_once('/').unwrap_or((locator, ""));
    let forbidden = |c: char| c == '\\' || c == '?' || c == '#';
    let valid = !bound_name.trim().is_empty()
        && !relative_path.trim().is_empty()
        && !bound_name.contains(forbidden)
        && !relative_path.contains(forbidden)
        && relative_path
            .split('/')
            .all(|segment| !segment.trim().is_empty() && segment != "." && segment != "..");
    if valid {
        Ok(format!("{BOUND_SCHEME}{bound_name}/{relative_path}"))
    } else {
        Err(NotebookError::Rejected(
            "Invalid bound resource locator".to_owned(),
        ))
    }
}

/// Decodifica `%xx` come `URI.getQuery()`; le sequenze non UTF-8 diventano U+FFFD.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| value.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected(path: &str) -> String {
        PortableLocator::classify(path).unwrap_err().to_string()
    }

    #[test]
    fn remote_urls_are_trimmed_and_kept() {
        assert_eq!(
            PortableLocator::classify("  https://informatelier.it/a?q=1  "),
            Ok(PortableLocator::Remote(
                "https://informatelier.it/a?q=1".into()
            ))
        );
    }

    #[test]
    fn remote_urls_with_credentials_are_not_portable() {
        let secret = "RAG_NON_PORTABLE_SECRET_REFERENCE: remote URL contains credentials";
        assert_eq!(rejected("https://user:pw@host/x"), secret);
        assert_eq!(rejected("https://host/x?api-key=1"), secret);
        assert_eq!(rejected("HTTPS://host/x?a=1&Access_Token=2"), secret);
        assert_eq!(rejected("https://host/x?x%5Fsignature=2"), secret);
        assert!(PortableLocator::classify("https://host/x?keyword=1").is_ok());
        assert_eq!(rejected("https://host/a b"), "Invalid remote resource URL");
    }

    #[test]
    fn bound_locators_are_normalized() {
        assert_eq!(
            PortableLocator::classify("BOUND://glossary/terms.md"),
            Ok(PortableLocator::Bound("bound://glossary/terms.md".into()))
        );
        for invalid in [
            "bound://glossary",
            "bound:///x",
            "bound://g/../x",
            "bound://g/a//b",
            "bound://g/a?b",
        ] {
            assert_eq!(
                rejected(invalid),
                "Invalid bound resource locator",
                "{invalid}"
            );
        }
    }
}
