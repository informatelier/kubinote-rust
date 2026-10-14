use kubi_kernel::json::JsonDialect;
use kubirag_domain::notebook::{RagNotebookManifest, RAG_NOTEBOOK_MANIFEST_VERSION};
use serde::Deserialize;
use serde_json::Value;

/// Esito della lettura del manifest, come `RagNotebookManifestReadResult` in Kotlin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestReadResult {
    Current(Box<RagNotebookManifest>),
    /// Manifest senza `id`: va migrato prima di qualsiasi scrittura governata.
    LegacyMissingIdentity {
        name: String,
    },
    UnsupportedVersion(String),
    Invalid(String),
    Missing,
}

/// Codifica e classificazione del manifest `_qb/notes/<store>/<store>-manifest.json`.
///
/// Il formato su disco è quello di `Json { prettyPrint; encodeDefaults }` di kotlinx
/// ([`JsonDialect::GOVERNANCE`]): 4 spazi, tutti i campi scritti, ordine della data class.
pub struct ManifestCodec;

impl ManifestCodec {
    pub fn encode(manifest: &RagNotebookManifest) -> serde_json::Result<String> {
        JsonDialect::GOVERNANCE.pretty(&serde_json::to_value(manifest)?)
    }

    /// `None` se il file non esiste.
    pub fn classify(text: Option<&str>) -> ManifestReadResult {
        let Some(text) = text else {
            return ManifestReadResult::Missing;
        };
        let root: Value = match serde_json::from_str(text) {
            Ok(root) => root,
            Err(error) => return ManifestReadResult::Invalid(error.to_string()),
        };
        let Some(fields) = root.as_object() else {
            return ManifestReadResult::Invalid("Notebook manifest root must be an object".into());
        };
        let version = match fields.get("version") {
            None => {
                return ManifestReadResult::Invalid("Notebook manifest version is missing".into())
            }
            // `JsonPrimitive.content` di kotlinx: il numero 1 diventa "1", null diventa "null".
            Some(Value::String(version)) => version.clone(),
            Some(Value::Null) => "null".into(),
            Some(Value::Number(version)) => version.to_string(),
            Some(Value::Bool(version)) => version.to_string(),
            Some(_) => {
                return ManifestReadResult::Invalid(
                    "Notebook manifest version is not a primitive".into(),
                )
            }
        };
        if version != RAG_NOTEBOOK_MANIFEST_VERSION {
            return ManifestReadResult::UnsupportedVersion(version);
        }
        if !fields.contains_key("id") {
            return match LegacyManifestRecord::deserialize(&root) {
                Ok(legacy) => ManifestReadResult::LegacyMissingIdentity { name: legacy.name },
                Err(error) => ManifestReadResult::Invalid(error.to_string()),
            };
        }
        match serde_json::from_value::<RagNotebookManifest>(root) {
            Ok(manifest) => ManifestReadResult::Current(Box::new(manifest)),
            Err(error) => ManifestReadResult::Invalid(error.to_string()),
        }
    }
}

/// Basta il nome: un manifest legacy non viene mai riscritto, solo migrato.
#[derive(Deserialize)]
struct LegacyManifestRecord {
    name: String,
}
