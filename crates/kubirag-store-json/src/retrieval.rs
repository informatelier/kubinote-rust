//! Viste derivate del Retrieval (`DocumentStore`, `InMemoryVectorStore`,
//! `EmbeddingProjectionStore`) e lettura delle sorgenti (`SourceImportService`,
//! `DocumentLoaders`), negli stessi file e formati della versione Kotlin.

use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use kubi_kernel::checksum::sha256_hex;
use kubi_kernel::identity::{ResourceRef, ResourceUri};
use kubi_kernel::json::{kotlin_float, JsonDialect};
use kubi_kernel::store::{md5_file, replace_atomically};
use kubirag_app::retrieval::{
    DocumentMaterializer, PreparedSource, ProjectionStore, SourceDiscovery, SourceImportOptions,
    SourceLoader, SOURCE_KEY_METADATA,
};
use kubirag_domain::notebook::{RagDocument, RagNotebookManifest, ResourceOwnership};
use kubirag_domain::retrieval::{Embedding, EmbeddingProjectionDescriptor};
use serde_json::{Map, Value};

use crate::paths::canonical;

const PROFILE_FILE: &str = "embedding-profile.json";
const DOCUMENT_SOURCE: &str = "document-source:";

/// File delle viste derivate:
/// - `.qb/notes/<store>/<store>-documents.json` e `<store>-embeddings.json` (locali, JSON compatto);
/// - `_qb/notes/<store>/embedding-profile.json` (portabile, 4 spazi, default scritti).
pub struct JsonProjectionStore {
    local_root: PathBuf,
    authority_root: PathBuf,
}

impl JsonProjectionStore {
    pub fn new(local_root: impl Into<PathBuf>, authority_root: impl Into<PathBuf>) -> Self {
        Self {
            local_root: local_root.into(),
            authority_root: authority_root.into(),
        }
    }

    pub fn documents_file(&self, store: &str) -> PathBuf {
        self.local_root
            .join("notes")
            .join(store)
            .join(format!("{store}-documents.json"))
    }

    pub fn embeddings_file(&self, store: &str) -> PathBuf {
        self.local_root
            .join("notes")
            .join(store)
            .join(format!("{store}-embeddings.json"))
    }

    pub fn profile_file(&self, store: &str) -> PathBuf {
        self.authority_root
            .join("notes")
            .join(store)
            .join(PROFILE_FILE)
    }

    /// `Json.encodeToString(List<RagDocument>)`: campi di default omessi in ogni elemento.
    pub fn encode_documents(documents: &[RagDocument]) -> String {
        let tree = Value::Array(
            documents
                .iter()
                .map(|document| {
                    let mut fields = Map::new();
                    fields.insert("id".into(), Value::String(document.id.clone()));
                    fields.insert("content".into(), Value::String(document.content.clone()));
                    if !document.metadata.is_empty() {
                        fields.insert("metadata".into(), json(&document.metadata));
                    }
                    if let Some(reference) = &document.resource_ref {
                        fields.insert("resourceRef".into(), json(reference));
                    }
                    Value::Object(fields)
                })
                .collect(),
        );
        JsonDialect::DEFAULT
            .pretty(&tree)
            .expect("a JSON tree always serializes")
    }

    /// `Json.encodeToString(List<Embedding>)`: i `Float` scritti come `Float.toString()`.
    pub fn encode_embeddings(embeddings: &[Embedding]) -> String {
        let tree = Value::Array(
            embeddings
                .iter()
                .map(|embedding| {
                    let mut fields = Map::new();
                    fields.insert("id".into(), Value::String(embedding.id.clone()));
                    fields.insert(
                        "vector".into(),
                        Value::Array(embedding.vector.iter().map(|v| kotlin_float(*v)).collect()),
                    );
                    if !embedding.metadata.is_empty() {
                        fields.insert("metadata".into(), json(&embedding.metadata));
                    }
                    Value::Object(fields)
                })
                .collect(),
        );
        JsonDialect::DEFAULT
            .pretty(&tree)
            .expect("a JSON tree always serializes")
    }

    pub fn encode_profile(descriptor: &EmbeddingProjectionDescriptor) -> String {
        JsonDialect::GOVERNANCE
            .pretty(&json(descriptor))
            .expect("a JSON tree always serializes")
    }
}

fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("retrieval values always serialize")
}

fn write_file(target: &Path, text: &str) -> Result<(), String> {
    let parent = target.parent().ok_or("target has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::write(target, text).map_err(|e| format!("{}: {e}", target.display()))
}

impl ProjectionStore for JsonProjectionStore {
    fn save_documents(&self, store: &str, documents: &[RagDocument]) -> Result<(), String> {
        write_file(
            &self.documents_file(store),
            &Self::encode_documents(documents),
        )
    }

    fn load_embeddings(&self, store: &str) -> Result<Vec<Embedding>, String> {
        let file = self.embeddings_file(store);
        match fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(format!("{}: {error}", file.display())),
        }
    }

    fn save_embeddings(&self, store: &str, embeddings: &[Embedding]) -> Result<(), String> {
        write_file(
            &self.embeddings_file(store),
            &Self::encode_embeddings(embeddings),
        )
    }

    /// Un profilo illeggibile vale come assente, come `runCatching { … }.getOrNull()`.
    fn read_profile(&self, store: &str) -> Option<EmbeddingProjectionDescriptor> {
        let text = fs::read_to_string(self.profile_file(store)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Scrittura su `.tmp` e rinomina, come `EmbeddingProjectionStore.write`.
    fn write_profile(
        &self,
        store: &str,
        descriptor: &EmbeddingProjectionDescriptor,
    ) -> Result<(), String> {
        let target = self.profile_file(store);
        let staging = target.with_file_name(format!("{PROFILE_FILE}.tmp"));
        write_file(&staging, &Self::encode_profile(descriptor))?;
        replace_atomically(&staging, &target).map_err(|e| e.to_string())
    }
}

/// `DocumentLoaders.loadDocumentFromFile`: testo UTF-8, id casuale, `resourceRef` `urn:uuid`.
pub fn load_document_from_file(file: &Path) -> Result<RagDocument, String> {
    if !file.is_file() {
        return Err(format!(
            "Il file [{}] non è valido",
            canonical(file).display()
        ));
    }
    if file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(format!(
            "{}: i PDF non sono ancora supportati dalla versione Rust",
            file.display()
        ));
    }
    let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let uri = ResourceUri::random();
    let id = uri
        .uuid()
        .expect("a random resource URI is a UUID")
        .to_owned();
    let metadata = IndexMap::from([
        ("filename".to_owned(), file_name(file)),
        ("path".to_owned(), file.to_string_lossy().into_owned()),
        ("metaTitle".to_owned(), String::new()),
        ("metaContent".to_owned(), String::new()),
        (
            "checkSum".to_owned(),
            md5_file(file).map_err(|e| e.to_string())?,
        ),
    ]);
    Ok(RagDocument {
        id,
        content: String::from_utf8_lossy(&bytes).into_owned(),
        metadata,
        resource_ref: Some(ResourceRef::new(uri)),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// Identità non reversibile di una sorgente: SHA-256 del percorso canonico.
pub fn source_identity(path: &Path) -> String {
    sha256_hex(canonical(path).to_string_lossy().as_bytes())
}

/// Sorgenti dal filesystem. File singoli o directory (ricorsive su richiesta, `.qb` escluso);
/// `.qbignore` non è ancora interpretato.
pub struct FsSourceLoader;

impl FsSourceLoader {
    fn candidates(root: &Path, recursive: bool, found: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_file() {
                found.push(path);
            } else if recursive && path.is_dir() && file_name(&path) != ".qb" {
                Self::candidates(&path, recursive, found)?;
            }
        }
        Ok(())
    }
}

impl SourceLoader for FsSourceLoader {
    fn discover(&self, options: &SourceImportOptions) -> Result<SourceDiscovery, String> {
        let mut files: IndexMap<String, PathBuf> = IndexMap::new();
        let mut ignored = 0;
        for raw in &options.paths {
            let source = Path::new(raw);
            if !source.exists() {
                return Err(format!(
                    "Source path does not exist: {}",
                    canonical(source).display()
                ));
            }
            if source.is_file() {
                if !options.included_extensions.contains(&extension(source)) {
                    return Err(format!(
                        "Source file extension .{} is not included: {raw}",
                        extension(source)
                    ));
                }
                let file = canonical(source);
                files.insert(file.to_string_lossy().into_owned(), file);
                continue;
            }
            let mut candidates = Vec::new();
            Self::candidates(&canonical(source), options.recursive, &mut candidates)?;
            for candidate in candidates {
                let name = file_name(&candidate);
                let accepted = name != ".qbignore"
                    && name != ".gitignore"
                    && options.included_extensions.contains(&extension(&candidate));
                if accepted {
                    let file = canonical(&candidate);
                    files.insert(file.to_string_lossy().into_owned(), file);
                } else {
                    ignored += 1;
                }
            }
        }
        files.sort_keys();
        let mut sources = Vec::new();
        let mut failures = Vec::new();
        for file in files.values() {
            match load_document_from_file(file) {
                Ok(mut document) => {
                    let identity = source_identity(file);
                    document
                        .metadata
                        .insert(SOURCE_KEY_METADATA.into(), identity.clone());
                    sources.push(PreparedSource {
                        identity,
                        documents: vec![document],
                    });
                }
                Err(error) => failures.push(error),
            }
        }
        if !failures.is_empty() {
            return Err(format!(
                "Source import failed before applying changes:\n{}",
                failures.join("\n")
            ));
        }
        Ok(SourceDiscovery { sources, ignored })
    }

    fn identity(&self, document: &RagDocument) -> Option<String> {
        if let Some(key) = document.metadata.get(SOURCE_KEY_METADATA) {
            return Some(key.clone());
        }
        document
            .metadata
            .get("path")
            .map(|path| source_identity(Path::new(path)))
    }
}

/// `materializeManagedDocuments`: un documento con risorsa `MANAGED` relativa è riletto dal
/// file dentro lo store; gli altri restano come nel manifest.
pub struct FsDocumentMaterializer {
    authority_root: PathBuf,
}

impl FsDocumentMaterializer {
    pub fn new(authority_root: impl Into<PathBuf>) -> Self {
        Self {
            authority_root: authority_root.into(),
        }
    }
}

impl DocumentMaterializer for FsDocumentMaterializer {
    fn materialize(&self, manifest: &RagNotebookManifest) -> Result<Vec<RagDocument>, String> {
        let store_dir = canonical(&self.authority_root.join("notes").join(manifest.name()));
        let by_document: IndexMap<&str, _> = manifest
            .resources()
            .iter()
            .filter_map(|r| Some((r.kind().strip_prefix(DOCUMENT_SOURCE)?, r)))
            .collect();
        manifest
            .documents()
            .iter()
            .map(|document| {
                let Some(resource) = by_document.get(document.id.as_str()) else {
                    return Ok(document.clone());
                };
                if resource.ownership() != ResourceOwnership::Managed
                    || Path::new(resource.path()).is_absolute()
                {
                    return Ok(document.clone());
                }
                let source = canonical(&store_dir.join(resource.path()));
                if !source.starts_with(&store_dir) || !source.is_file() {
                    return Err(format!("RAG_MANAGED_NOTE_MISSING: {}", resource.path()));
                }
                let loaded = load_document_from_file(&source)?;
                let mut materialized = document.clone();
                materialized.content = loaded.content;
                materialized.metadata.extend(loaded.metadata);
                materialized
                    .metadata
                    .insert("path".into(), source.to_string_lossy().into_owned());
                Ok(materialized)
            })
            .collect()
    }
}
