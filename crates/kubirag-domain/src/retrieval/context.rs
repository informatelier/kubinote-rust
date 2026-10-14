//! Assemblaggio del contesto e del prompt di risposta (`selectSearchContext`,
//! `queryLuceneAsyncLLM`): prima i documenti pinned, poi quelli pertinenti, mai gli artefatti
//! (quiz, flashcard, report…) che l'applicazione stessa ha generato.

use crate::notebook::RagDocument;

use super::text::{is_kotlin_whitespace, kotlin_trim_indent, take_utf16, utf16_len};

const ARTIFACT_KINDS: [&str; 4] = ["flashcards", "quiz", "think", "report"];
const ARTIFACT_MARKER: &str = "kubi-type:";
const ARTIFACT_HEAD_CHARS: usize = 2048;
const HISTORY_MESSAGE_CHARS: usize = 500;

/// `ArtifactKind.isArtifactDocument`: percorso sotto `artifacts/` o marker `kubi-type:` nei
/// primi 2048 caratteri.
pub fn is_artifact_document(content: &str, path: &str) -> bool {
    format!("/{}", path.replace('\\', "/")).contains("/artifacts/")
        || has_artifact_marker(take_utf16(content, ARTIFACT_HEAD_CHARS))
}

pub fn is_artifact(document: &RagDocument) -> bool {
    is_artifact_document(
        &document.content,
        document.metadata.get("path").map_or("", String::as_str),
    )
}

/// `(?mi)^kubi-type:\s*(flashcards|quiz|think|report)\s*$` (`\s` ASCII, terminatori Java).
fn has_artifact_marker(head: &str) -> bool {
    let is_terminator = |c: char| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}');
    let is_space = |c: char| matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r');
    let line_starts = std::iter::once(0).chain(head.char_indices().filter_map(|(i, c)| {
        let next = i + c.len_utf8();
        let crlf_middle = c == '\r' && head[next..].starts_with('\n');
        (is_terminator(c) && !crlf_middle).then_some(next)
    }));
    line_starts.into_iter().any(|start| {
        let line = &head[start..];
        let Some(rest) = strip_prefix_ignore_ascii_case(line, ARTIFACT_MARKER) else {
            return false;
        };
        let rest = rest.trim_start_matches(is_space);
        ARTIFACT_KINDS.iter().any(|kind| {
            strip_prefix_ignore_ascii_case(rest, kind).is_some_and(|after| {
                let trailing = after.len() - after.trim_start_matches(is_space).len();
                // `\s*$`: un qualsiasi prefisso degli spazi deve finire su un terminatore o in fondo.
                (0..=trailing)
                    .filter(|n| after.is_char_boundary(*n))
                    .any(|n| after[n..].chars().next().is_none_or(is_terminator))
            })
        })
    })
}

fn strip_prefix_ignore_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Una voce del contesto: `PINNED <id>: <metaContent> <content>`.
pub fn context_entry(label: &str, document: &RagDocument) -> String {
    let meta = document
        .metadata
        .get("metaContent")
        .map_or("", String::as_str);
    format!("{label} {}: {meta} {}", document.id, document.content)
}

pub fn context_block(label: &str, documents: &[RagDocument]) -> String {
    documents
        .iter()
        .map(|d| context_entry(label, d))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Il contesto per l'LLM: blocco pinned e blocco pertinenti, quelli vuoti omessi.
pub fn build_context(pinned: &[RagDocument], relevant: &[RagDocument]) -> String {
    [
        context_block("PINNED", pinned),
        context_block("RELEVANT", relevant),
    ]
    .into_iter()
    .filter(|block| !block.chars().all(is_kotlin_whitespace))
    .collect::<Vec<_>>()
    .join("\n\n")
}

/// Caratteri stimati del contesto (`estimatedContextChars`): somma delle lunghezze UTF-16.
pub fn estimated_context_chars(pinned: &[RagDocument], relevant: &[RagDocument]) -> usize {
    relevant
        .iter()
        .chain(pinned)
        .map(|d| utf16_len(&d.content))
        .sum()
}

/// Un messaggio dello storico della conversazione.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

/// Il prompt di `queryLuceneAsyncLLM`. Il Kotlin usa `trimIndent` (non `trimMargin`) *dopo* la
/// sostituzione delle variabili: i `|` restano e, se il contesto ha righe a colonna 0,
/// l'indentazione del template non viene tolta. Lo replichiamo alla lettera.
pub fn answer_prompt(context: &str, history: &[ChatTurn], window: usize, question: &str) -> String {
    let history_section = if history.is_empty() {
        String::new()
    } else {
        let lines = history[history.len().saturating_sub(window)..]
            .iter()
            .map(|turn| {
                format!(
                    "[{}]: {}",
                    turn.role,
                    take_utf16(&turn.content, HISTORY_MESSAGE_CHARS)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n###Storico conversazione:\n{lines}\n")
    };
    let indent = " ".repeat(12);
    let template = [
        String::new(),
        format!("{indent}|Considera il seguente documento:"),
        format!("{indent}|###Documento:"),
        format!("{indent}|{context}"),
        format!("{indent}{history_section}"),
        format!("{indent}|Rispondi alla seguente domanda usando il contesto come riferimento,"),
        format!("{indent}|###Domanda:"),
        format!("{indent}|{question}"),
        format!("{indent}|"),
        format!("{indent}|### Risposta con riferimento ai documenti in contesto,"),
        format!("{indent}|includi fino a 3 possibili applicazioni del contenuto del documento."),
        format!("{indent}|Se non hai documenti di riferimento scusati dicendo che non hai sufficienti informazioni."),
        " ".repeat(8),
    ]
    .join("\n");
    kotlin_trim_indent(&template)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, content: &str) -> RagDocument {
        RagDocument::new(id, content)
    }

    #[test]
    fn artifacts_are_detected_by_path_or_marker() {
        assert!(is_artifact_document("", "notes/artifacts/q.md"));
        assert!(is_artifact_document("", "artifacts\\q.md"));
        assert!(is_artifact_document("---\nKUBI-TYPE:  Quiz \n---", ""));
        assert!(
            is_artifact_document("kubi-type:\nquiz", ""),
            "\\s attraversa la riga"
        );
        assert!(!is_artifact_document("kubi-type: quizzes", ""));
        assert!(!is_artifact_document(" kubi-type: quiz", ""));
        assert!(!is_artifact_document(
            &format!("{}\nkubi-type: quiz", "x".repeat(2048)),
            ""
        ));
    }

    #[test]
    fn the_context_lists_pinned_before_relevant() {
        let mut pinned = doc("p", "uno");
        pinned.metadata.insert("metaContent".into(), "meta".into());
        assert_eq!(
            build_context(&[pinned], &[doc("r", "due")]),
            "PINNED p: meta uno\n\nRELEVANT r:  due"
        );
        assert_eq!(build_context(&[], &[]), "");
    }

    #[test]
    fn a_single_line_context_strips_the_template_indent() {
        let prompt = answer_prompt("RELEVANT a:  x", &[], 10, "q?");
        assert!(prompt.starts_with(
            "|Considera il seguente documento:\n|###Documento:\n|RELEVANT a:  x\n\n|Rispondi"
        ));
        assert!(prompt.ends_with("|Se non hai documenti di riferimento scusati dicendo che non hai sufficienti informazioni."));
    }

    #[test]
    fn a_multi_line_context_keeps_the_template_indent() {
        let prompt = answer_prompt("RELEVANT a:  x\ny", &[], 10, "q?");
        assert!(prompt.starts_with("            |Considera il seguente documento:\n"));
        assert!(prompt.contains("\ny\n            \n            |Rispondi"));
    }
}
