use std::sync::{Arc, Mutex};

use kubirag_domain::notebook::RagNotebookManifest;

use super::{DocumentMaterializer, RetrievalRuntime};
use crate::notebook::NotebookProjection;

/// `RagRuntimeProjection`: dopo un commit materializza i documenti e ricostruisce il runtime.
pub struct RuntimeProjection {
    runtime: Arc<Mutex<RetrievalRuntime>>,
    materializer: Arc<dyn DocumentMaterializer>,
}

impl RuntimeProjection {
    pub fn new(
        runtime: Arc<Mutex<RetrievalRuntime>>,
        materializer: Arc<dyn DocumentMaterializer>,
    ) -> Self {
        Self {
            runtime,
            materializer,
        }
    }
}

impl NotebookProjection for RuntimeProjection {
    fn rebuild(
        &self,
        before: &RagNotebookManifest,
        after: &RagNotebookManifest,
    ) -> Result<(), String> {
        let documents = self.materializer.materialize(after)?;
        self.runtime
            .lock()
            .map_err(|_| "retrieval runtime lock poisoned".to_owned())?
            .rebuild(before, after, documents)
    }
}
