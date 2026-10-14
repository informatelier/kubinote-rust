use std::path::{Path, PathBuf};

use kubi_kernel::store::sha256_file;
use kubirag_domain::notebook::{ResourceFingerprints, ResourceOwnership};

use crate::paths::java_child;

/// Impronte dei file sorgente letti dal disco, relative allo store del notebook.
pub struct FsResourceFingerprints {
    store_dir: PathBuf,
}

impl FsResourceFingerprints {
    pub fn new(store_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_dir: store_dir.into(),
        }
    }
}

fn file_sha256(path: &Path) -> Option<String> {
    path.is_file().then(|| sha256_file(path).ok()).flatten()
}

impl ResourceFingerprints for FsResourceFingerprints {
    /// Kotlin: `MANAGED` relativo → dentro lo store; tutto il resto così com'è (un relativo
    /// `EXTERNAL` è quindi relativo alla directory di lavoro del processo).
    fn document_source(&self, path: &str, ownership: ResourceOwnership) -> Option<String> {
        let raw = Path::new(path);
        if ownership == ResourceOwnership::Managed && !raw.is_absolute() {
            file_sha256(&self.store_dir.join(path))
        } else {
            file_sha256(raw)
        }
    }

    fn notebook_asset(&self, path: &str) -> Option<String> {
        file_sha256(&java_child(&self.store_dir, path))
    }
}
