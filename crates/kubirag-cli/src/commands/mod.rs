//! I comandi della CLI (`RagollamaCommand`), raggruppati per bounded context:
//! [`sources`] (Notebook Governance: import e elenco), [`pins`] (Notebook Governance: pin),
//! [`search`] (Retrieval: query, statistiche, embedding).

use std::io::Write;

use kubi_kernel::governance::{OperationContext, OperationResult};

use crate::bootstrap::Notebook;

pub mod pins;
pub mod search;
pub mod sources;

/// Perché un comando non è riuscito. `Failure` è un `Result.failure` (stampa il messaggio),
/// `Exception` un'eccezione che il terminale riporta come `Error: …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    Failure(String),
    Exception(String),
}

pub type CommandResult = Result<String, CommandError>;

/// `RagollamaCommand`.
pub trait Command {
    fn name(&self) -> &'static str;
    fn help_line(&self) -> String;
    fn help_usage(&self) -> String;
    fn help_example(&self) -> String;
    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult;
}

/// Tutti i comandi della CLI; l'ultimo, `/lucene`, è anche il comando di default.
pub fn all() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(sources::Load),
        Box::new(sources::List),
        Box::new(pins::Pin),
        Box::new(pins::Unpin),
        Box::new(pins::Pinned),
        Box::new(search::Stat),
        Box::new(search::Embed),
        Box::new(search::Lucene),
    ]
}

pub(crate) fn exception(error: impl ToString) -> CommandError {
    CommandError::Exception(error.to_string())
}

/// Scrive sull'output del terminale; un errore di scrittura è un'eccezione.
pub(crate) fn emit(out: &mut dyn Write, text: &str) -> Result<(), CommandError> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(exception)
}

const EXPECTED_REVISION: &str = "--expected-revision=";

/// `governedCommand`: toglie `--expected-revision=N` dagli argomenti e costruisce il contesto
/// dell'operazione (senza opzione vale la revisione corrente del manifest).
pub(crate) fn governed_command(
    tokens: &[String],
    notebook: &mut Notebook,
) -> Result<(Vec<String>, OperationContext), CommandError> {
    let option = tokens
        .iter()
        .find_map(|t| t.strip_prefix(EXPECTED_REVISION));
    let parsed = option.and_then(|value| value.parse::<i64>().ok());
    let expected = parsed.unwrap_or_else(|| notebook.service().snapshot().revision());
    if expected < 0 {
        return Err(exception("Expected revision cannot be negative"));
    }
    if option.is_some() && parsed.is_none() {
        return Err(exception("Invalid expected revision option"));
    }
    let arguments = tokens
        .iter()
        .filter(|t| !t.starts_with(EXPECTED_REVISION))
        .cloned()
        .collect();
    let context = OperationContext::new(notebook.actor(), notebook.channel, expected, None)
        .map_err(exception)?;
    Ok((arguments, context))
}

/// `OperationResult.asCommandResult`.
pub(crate) fn as_command_result(result: &OperationResult, name: &str) -> CommandResult {
    if result.applied() {
        let revision = result
            .current_revision()
            .map_or_else(|| "null".to_owned(), |r| r.to_string());
        Ok(format!(
            "{name} done at revision {revision}. {}",
            result.message()
        ))
    } else {
        let status = serde_json::to_value(result.status())
            .ok()
            .and_then(|v| v.as_str().map(str::to_lowercase))
            .unwrap_or_default();
        Err(CommandError::Failure(format!(
            "{name} {status}: {}",
            result.message()
        )))
    }
}
