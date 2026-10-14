use std::fs;
use std::path::Path;

use kubi_kernel::json::JsonDialect;
use kubirag_domain::config::RagConfig;

pub const RAG_CONFIG_FILENAME: &str = "qb-rag.json";

/// `.qb/qb-rag.json`: `Json { prettyPrint }` di kotlinx, 4 spazi, valori di default omessi,
/// nessun newline finale. In lettura le chiavi sconosciute sono ignorate.
pub struct RagConfigCodec;

impl RagConfigCodec {
    pub fn encode(config: &RagConfig) -> serde_json::Result<String> {
        JsonDialect::CONFIG.encode(config)
    }

    pub fn decode(text: &str) -> serde_json::Result<RagConfig> {
        serde_json::from_str(text)
    }

    /// `restoreConfig` senza effetti collaterali: `None` se il file non c'è.
    pub fn read(qb_local_root: &Path) -> Result<Option<RagConfig>, String> {
        match fs::read_to_string(qb_local_root.join(RAG_CONFIG_FILENAME)) {
            Ok(text) => Self::decode(&text).map(Some).map_err(|e| e.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    /// `storeConfig`: Kotlin usa `writeText`, non una sostituzione atomica; qui sì.
    pub fn write(config: &RagConfig, qb_local_root: &Path) -> Result<(), String> {
        let text = Self::encode(config).map_err(|e| e.to_string())?;
        let target = qb_local_root.join(RAG_CONFIG_FILENAME);
        kubi_kernel::store::write_verified::<std::io::Error>(
            &target,
            qb_local_root,
            "qb-rag",
            &text,
            |_| Ok(()),
        )
        .map_err(|e| e.to_string())
    }
}
