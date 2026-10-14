//! Notebook Governance: le sorgenti entrano nel manifest con un `REPLACE_DOCUMENTS`
//! governato (`/load`) e si elencano dal runtime (`/list`).

use std::io::Write;
use std::path::Path;

use indexmap::{IndexMap, IndexSet};
use kubirag_app::notebook::RandomIds;
use kubirag_app::retrieval::{import_sources, SourceImportOptions};
use kubirag_domain::notebook::RagDocument;
use kubirag_domain::retrieval::{CHUNK_INDEX, PARENT_ID};
use kubirag_store_json::FsSourceLoader;

use super::{emit, exception, governed_command, Command, CommandError, CommandResult};
use crate::bootstrap::Notebook;

/// `File.extension`: il testo dopo l'ultimo punto del nome, o vuoto.
fn extension(file_name: &str) -> String {
    let name = Path::new(file_name)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    name.rsplit_once('.')
        .map_or_else(String::new, |(_, ext)| ext.to_lowercase())
}

pub struct Load;

impl Command for Load {
    fn name(&self) -> &'static str {
        "/load"
    }

    fn help_line(&self) -> String {
        "Loads and indexes a single document file. PDFs are split into page-range chunks.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} <filename> - Loads the specified file. PDFs prompt for pages-per-chunk configuration.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/load README.md  or  /load document.pdf".into()
    }

    fn execute(
        &self,
        tokens: &[String],
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> CommandResult {
        let (arguments, context) = governed_command(tokens, notebook)?;
        let Some(file_name) = arguments.get(1) else {
            emit(out, "Parameter filename required.\n")?;
            return Ok(format!("{} Done.", self.name()));
        };
        let extension = extension(file_name);
        if extension == "pdf" {
            return Err(CommandError::Failure(format!(
                "{}: PDF import is not available in the Rust CLI yet",
                self.name()
            )));
        }
        let options = SourceImportOptions {
            paths: vec![file_name.clone()],
            recursive: false,
            included_extensions: IndexSet::from([extension]),
        };
        let actor = notebook.actor();
        let channel = notebook.channel;
        let summary = import_sources(
            notebook.service(),
            &FsSourceLoader,
            &RandomIds,
            &options,
            actor,
            channel,
            Some(context.expected_revision()),
        )
        .map_err(exception)?;
        Ok(format!("{} done. {summary}", self.name()))
    }
}

pub struct List;

impl Command for List {
    fn name(&self) -> &'static str {
        "/list"
    }

    fn help_line(&self) -> String {
        "Lists all loaded documents. PDF chunk groups are shown as folder nodes.".into()
    }

    fn help_usage(&self) -> String {
        format!(
            "{} - No arguments required. Displays all indexed documents; PDF chunks grouped by parent.",
            self.name()
        )
    }

    fn help_example(&self) -> String {
        "/list".into()
    }

    fn execute(&self, _: &[String], notebook: &mut Notebook, _: &mut dyn Write) -> CommandResult {
        let runtime = notebook.runtime().map_err(exception)?;
        let documents: Vec<&RagDocument> = runtime.documents().collect();
        Ok(render_list(&documents).trim_end().to_owned())
    }
}

/// `CmdList`: i chunk raggruppati per `parent_id` (in ordine di comparsa), poi i documenti
/// singoli come `id . path`.
fn render_list(documents: &[&RagDocument]) -> String {
    let meta = |d: &RagDocument, key: &str| d.metadata.get(key).cloned();
    let mut groups: IndexMap<String, Vec<&RagDocument>> = IndexMap::new();
    for document in documents
        .iter()
        .filter(|d| d.metadata.contains_key(PARENT_ID))
    {
        groups
            .entry(document.metadata[PARENT_ID].clone())
            .or_default()
            .push(document);
    }
    let mut text = String::new();
    for chunks in groups.values_mut() {
        chunks.sort_by_key(|d| {
            d.metadata
                .get(CHUNK_INDEX)
                .and_then(|i| i.parse::<i32>().ok())
                .unwrap_or(0)
        });
        let group = meta(chunks[0], "group_name").unwrap_or_else(|| "?".into());
        let total = chunks.len();
        text.push_str(&format!("📄 {group}  ({total} chunks)\n"));
        for (i, chunk) in chunks.iter().enumerate() {
            let pages = meta(chunk, "page_range").unwrap_or_else(|| "?".into());
            let prefix = if i < total - 1 { "├─" } else { "└─" };
            text.push_str(&format!(
                "   {prefix} [{}/{total}] pp. {pages}  ({})\n",
                i + 1,
                chunk.id
            ));
        }
    }
    for document in documents
        .iter()
        .filter(|d| !d.metadata.contains_key(PARENT_ID))
    {
        let path = meta(document, "path").unwrap_or_else(|| "null".into());
        text.push_str(&format!("{} . {path}\n", document.id));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_is_what_follows_the_last_dot_of_the_name() {
        assert_eq!(extension("docs/README.MD"), "md");
        assert_eq!(extension("archive.tar.gz"), "gz");
        assert_eq!(extension("dir.d/Makefile"), "");
    }

    #[test]
    fn chunks_are_grouped_under_their_parent() {
        let chunk = |id: &str, index: &str| {
            let mut d = RagDocument::new(id, "x");
            d.metadata.insert(PARENT_ID.into(), "p".into());
            d.metadata.insert(CHUNK_INDEX.into(), index.into());
            d.metadata.insert("group_name".into(), "guide".into());
            d
        };
        let (second, first) = (chunk("c1", "1"), chunk("c0", "0"));
        let mut single = RagDocument::new("s", "y");
        single.metadata.insert("path".into(), "/a.md".into());
        let text = render_list(&[&second, &first, &single]);
        assert_eq!(
            text,
            "📄 guide  (2 chunks)\n   ├─ [1/2] pp. ?  (c0)\n   └─ [2/2] pp. ?  (c1)\ns . /a.md\n"
        );
    }
}
