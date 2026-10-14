use indexmap::IndexMap;

/// Un assistente: il prompt di sistema che sostituisce quello della configurazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleEntry {
    pub role_name: String,
    pub role_prompt: String,
    pub file_patterns: Vec<String>,
    pub voice_name: Option<String>,
}

impl RoleEntry {
    /// Una voce vuota non è una voce (`voiceName?.takeIf { it.isNotBlank() }`).
    pub fn new(
        role_name: impl Into<String>,
        role_prompt: impl Into<String>,
        file_patterns: Vec<String>,
        voice_name: Option<String>,
    ) -> Self {
        Self {
            role_name: role_name.into(),
            role_prompt: role_prompt.into(),
            file_patterns,
            voice_name: voice_name.filter(|v| !v.trim().is_empty()),
        }
    }
}

/// Il catalogo degli assistenti (`qb-roles.json`), in ordine di inserimento.
pub type Roles = IndexMap<String, RoleEntry>;

/// `RoleLoader.parseAtMention`: `@nome resto` diventa `(Some(nome), resto)`; un testo che non
/// inizia con `@` resta com'è.
pub fn parse_at_mention(input: &str) -> (Option<&str>, &str) {
    let Some(rest) = input.strip_prefix('@') else {
        return (None, input);
    };
    match rest.find(' ') {
        None => (Some(rest), ""),
        Some(space) => (Some(&rest[..space]), rest[space + 1..].trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_split_the_role_from_the_question() {
        assert_eq!(
            parse_at_mention("@poeta  parlami "),
            (Some("poeta"), "parlami")
        );
        assert_eq!(parse_at_mention("@party"), (Some("party"), ""));
        assert_eq!(parse_at_mention("ciao @poeta"), (None, "ciao @poeta"));
    }

    #[test]
    fn a_blank_voice_is_no_voice() {
        assert_eq!(
            RoleEntry::new("a", "b", vec![], Some(" ".into())).voice_name,
            None
        );
    }
}
