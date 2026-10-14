//! Notebook Governance: i pin sono comandi governati sul manifest (`/pin`, `/unpin`) e una
//! vista del runtime (`/pinned`).

use std::io::Write;

use kubirag_domain::notebook::RagNotebookCommand;

use super::{as_command_result, emit, governed_command, Command, CommandResult};
use crate::bootstrap::Notebook;

fn governed_pin(
    name: &str,
    tokens: &[String],
    notebook: &mut Notebook,
    out: &mut dyn Write,
    command: fn(String) -> RagNotebookCommand,
) -> CommandResult {
    let (arguments, context) = governed_command(tokens, notebook)?;
    match arguments.get(1) {
        None => {
            emit(out, "required parameter: DocumentID \n")?;
            Ok(format!("{name} done."))
        }
        Some(id) => {
            let result = notebook.service().execute(&command(id.clone()), &context);
            as_command_result(&result, name)
        }
    }
}

pub struct Pin;

impl Command for Pin {
    fn name(&self) -> &'static str {
        "/pin"
    }

    fn help_line(&self) -> String {
        "Pins a document to always include it in RAG context.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} <document-id> - Pins the specified document so it's always included in AI context.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/pin doc-123".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        governed_pin(self.name(), tokens, notebook, out, |id| {
            RagNotebookCommand::Pin { id }
        })
    }
}

pub struct Unpin;

impl Command for Unpin {
    fn name(&self) -> &'static str {
        "/unpin"
    }

    fn help_line(&self) -> String {
        "Removes a document from the pinned list.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} <document-id> - Unpins the specified document so it's no longer always included in context.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/unpin doc-123".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        governed_pin(self.name(), tokens, notebook, out, |id| {
            RagNotebookCommand::Unpin { id }
        })
    }
}

pub struct Pinned;

impl Command for Pinned {
    fn name(&self) -> &'static str {
        "/pinned"
    }

    fn help_line(&self) -> String {
        "Lists all currently pinned documents.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} - No arguments required. Shows all documents currently pinned for RAG context.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/pinned".into()
    }

    fn execute(&self, _: &[String], notebook: &mut Notebook, out: &mut dyn Write) -> CommandResult {
        let pinned = notebook
            .runtime()
            .map_err(super::exception)?
            .pinned_ids()
            .to_vec();
        for (index, id) in pinned.iter().enumerate() {
            emit(out, &format!("{index}) {id}\n"))?;
        }
        Ok(format!("{} done.", self.name()))
    }
}
