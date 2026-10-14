use std::fmt;

use indexmap::{IndexMap, IndexSet};
use kubi_kernel::governance::{OperationChannel, OperationContext, OperationResult};
use kubirag_domain::notebook::{RagDocument, RagNotebookCommand, ResourceOwnership};
use kubirag_domain::retrieval::{CHUNK_INDEX, PARENT_ID};

use crate::notebook::{IdGenerator, RagNotebookApplicationService};

/// Metadato con l'identità stabile (non reversibile) della sorgente: SHA-256 del percorso
/// canonico, così il manifest non espone il percorso della workstation.
pub const SOURCE_KEY_METADATA: &str = "kubi_source_key";
const CHECKSUM_METADATA: &str = "checkSum";
const PAGE_RANGE_METADATA: &str = "page_range";

/// `SourceImportOptions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImportOptions {
    pub paths: Vec<String>,
    pub recursive: bool,
    pub included_extensions: IndexSet<String>,
}

/// Una sorgente letta dal disco: i suoi documenti, già con `kubi_source_key`.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedSource {
    pub identity: String,
    pub documents: Vec<RagDocument>,
}

/// Sorgenti trovate e preparate, più i file scartati da estensione o `.qbignore`.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceDiscovery {
    pub sources: Vec<PreparedSource>,
    pub ignored: usize,
}

/// Porta di lettura delle sorgenti (filesystem, loader di formato).
pub trait SourceLoader {
    /// Scopre e prepara le sorgenti; un errore annulla l'import prima di qualsiasi modifica.
    fn discover(&self, options: &SourceImportOptions) -> Result<SourceDiscovery, String>;
    /// Identità di un documento già nel manifest (`kubi_source_key` o percorso canonico).
    fn identity(&self, document: &RagDocument) -> Option<String>;
}

/// `SourceImportSummary`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceImportSummary {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub ignored: usize,
}

impl fmt::Display for SourceImportSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sources: found={}, added={}, updated={}, unchanged={}, ignored={}",
            self.found, self.added, self.updated, self.unchanged, self.ignored
        )
    }
}

fn chunk_key(document: &RagDocument) -> &str {
    document
        .metadata
        .get(CHUNK_INDEX)
        .map_or("single", String::as_str)
}

/// Stesso numero di documenti e, chunk per chunk, stesso checksum e stesso intervallo di pagine.
fn is_unchanged(existing: &[&RagDocument], prepared: &[RagDocument]) -> bool {
    if existing.len() != prepared.len() || existing.is_empty() {
        return false;
    }
    let by_chunk: IndexMap<&str, &&RagDocument> =
        existing.iter().map(|d| (chunk_key(d), d)).collect();
    prepared.iter().all(|candidate| {
        by_chunk.get(chunk_key(candidate)).is_some_and(|previous| {
            previous.metadata.get(CHECKSUM_METADATA) == candidate.metadata.get(CHECKSUM_METADATA)
                && previous.metadata.get(PAGE_RANGE_METADATA)
                    == candidate.metadata.get(PAGE_RANGE_METADATA)
        })
    })
}

/// Una sorgente già importata mantiene gli id dei documenti (e il `parent_id` dei chunk).
fn preserve_identity(existing: &[&RagDocument], prepared: &[RagDocument]) -> Vec<RagDocument> {
    if existing.is_empty() {
        return prepared.to_vec();
    }
    let by_chunk: IndexMap<&str, &&RagDocument> =
        existing.iter().map(|d| (chunk_key(d), d)).collect();
    let parent_id = existing.iter().find_map(|d| d.metadata.get(PARENT_ID));
    prepared
        .iter()
        .map(|candidate| {
            let mut document = candidate.clone();
            if let Some(previous) = by_chunk.get(chunk_key(candidate)) {
                document.id = previous.id.clone();
            }
            if let Some(parent_id) = parent_id {
                document
                    .metadata
                    .insert(PARENT_ID.into(), parent_id.clone());
            }
            document
        })
        .collect()
}

/// `SourceImportService.import`: confronta le sorgenti col manifest e applica in un solo
/// `REPLACE_DOCUMENTS` le aggiunte e gli aggiornamenti; le sorgenti invariate non cambiano.
pub fn import_sources(
    service: &mut RagNotebookApplicationService,
    loader: &dyn SourceLoader,
    ids: &dyn IdGenerator,
    options: &SourceImportOptions,
    actor: &str,
    channel: OperationChannel,
    expected_revision: Option<i64>,
) -> Result<SourceImportSummary, String> {
    if options.paths.is_empty() {
        return Err("At least one source path is required".into());
    }
    let extensions: IndexSet<String> = options
        .included_extensions
        .iter()
        .map(|e| e.trim().trim_start_matches('.').to_lowercase())
        .filter(|e| !e.trim().is_empty())
        .collect();
    if extensions.is_empty() {
        return Err("At least one source extension is required".into());
    }
    let discovery = loader.discover(&SourceImportOptions {
        included_extensions: extensions,
        ..options.clone()
    })?;

    let current = service.snapshot().clone();
    let mut by_identity: IndexMap<Option<String>, Vec<&RagDocument>> = IndexMap::new();
    for document in current.documents() {
        by_identity
            .entry(loader.identity(document))
            .or_default()
            .push(document);
    }
    let mut remove_ids: IndexSet<String> = IndexSet::new();
    let mut replacements = Vec::new();
    let (mut added, mut updated, mut unchanged) = (0, 0, 0);
    for source in &discovery.sources {
        let existing: &[&RagDocument] = by_identity
            .get(&Some(source.identity.clone()))
            .map_or(&[], Vec::as_slice);
        if is_unchanged(existing, &source.documents) {
            unchanged += 1;
            continue;
        }
        if existing.is_empty() {
            added += 1;
        } else {
            updated += 1;
        }
        remove_ids.extend(existing.iter().map(|d| d.id.clone()));
        replacements.extend(preserve_identity(existing, &source.documents));
    }

    if !remove_ids.is_empty() || !replacements.is_empty() {
        let context = OperationContext::new(
            actor,
            channel,
            expected_revision.unwrap_or(current.revision()),
            Some(format!("source-import-{}", ids.new_id())),
        )
        .map_err(|e| e.to_string())?;
        let command = RagNotebookCommand::ReplaceDocuments {
            remove_ids,
            documents: replacements,
            ownership: ResourceOwnership::External,
            preserve_replacement_pins: true,
        };
        let result: OperationResult = service.execute(&command, &context);
        if !result.applied() {
            return Err(result.message().to_owned());
        }
    }
    Ok(SourceImportSummary {
        found: discovery.sources.len(),
        added,
        updated,
        unchanged,
        ignored: discovery.ignored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, checksum: &str) -> RagDocument {
        let mut document = RagDocument::new(id, "x");
        document
            .metadata
            .insert(CHECKSUM_METADATA.into(), checksum.into());
        document
    }

    #[test]
    fn an_unchanged_source_matches_checksums_chunk_by_chunk() {
        let existing = doc("old", "a");
        assert!(is_unchanged(&[&existing], &[doc("new", "a")]));
        assert!(!is_unchanged(&[&existing], &[doc("new", "b")]));
        assert!(!is_unchanged(&[], &[doc("new", "a")]));
    }

    #[test]
    fn a_reimported_source_keeps_its_document_id() {
        let existing = doc("old", "a");
        let replaced = preserve_identity(&[&existing], &[doc("new", "b")]);
        assert_eq!(replaced[0].id, "old");
    }
}
