use std::sync::{Arc, Mutex, MutexGuard};

use kubi_kernel::governance::OperationChannel;
use kubirag_app::notebook::{
    NoProjection, NotebookProjection, RagNotebookApplicationService, RagNotebookManifestManager,
    RandomIds, SystemClock,
};
use kubirag_app::retrieval::{
    DocumentMaterializer, LlmGateway, RetrievalRuntime, RuntimeProjection,
};
use kubirag_domain::config::RagConfig;
use kubirag_retrieval::{InMemoryVectorStore, LuceneCompatibleSearchEngine};
use kubirag_store_json::{
    FsDocumentMaterializer, FsResourceFingerprints, JsonManifestRepository,
    JsonNotebookAuditJournal, JsonProjectionStore,
};

use crate::WorkspaceRoots;

/// Il notebook attivo: il servizio governato del manifest e il runtime di retrieval che le
/// sue proiezioni tengono allineato.
pub struct Notebook {
    pub config: RagConfig,
    pub roots: WorkspaceRoots,
    pub channel: OperationChannel,
    service: RagNotebookApplicationService,
    runtime: Arc<Mutex<RetrievalRuntime>>,
}

impl Notebook {
    /// Apre (o crea vuoto) il notebook `store` e ne costruisce il runtime.
    pub fn open(
        roots: WorkspaceRoots,
        store: &str,
        config: RagConfig,
        llm: Arc<dyn LlmGateway>,
        channel: OperationChannel,
    ) -> Result<Self, String> {
        let projections = Arc::new(JsonProjectionStore::new(&roots.local, &roots.authority));
        let runtime = Arc::new(Mutex::new(RetrievalRuntime::new(
            Box::new(LuceneCompatibleSearchEngine::new()),
            Box::new(InMemoryVectorStore::new()),
            llm,
            projections,
            Arc::new(SystemClock),
        )));
        let materializer: Arc<dyn DocumentMaterializer> =
            Arc::new(FsDocumentMaterializer::new(&roots.authority));
        let service = governed_service(
            &roots,
            store,
            true,
            Box::new(RuntimeProjection::new(
                runtime.clone(),
                materializer.clone(),
            )),
        )?;
        let documents = materializer.materialize(service.snapshot())?;
        lock(&runtime)?.open(service.snapshot(), documents)?;
        Ok(Self {
            config,
            roots,
            channel,
            service,
            runtime,
        })
    }

    pub fn service(&mut self) -> &mut RagNotebookApplicationService {
        &mut self.service
    }

    /// Il runtime, da non tenere bloccato mentre si esegue un comando governato: la
    /// proiezione lo ricostruisce dopo il commit.
    pub fn runtime(&self) -> Result<MutexGuard<'_, RetrievalRuntime>, String> {
        lock(&self.runtime)
    }

    /// Attore dei comandi governati (`governedCommand`).
    pub fn actor(&self) -> &'static str {
        match self.channel {
            OperationChannel::Cli => "cli",
            OperationChannel::ApiWeb => "api-web",
            _ => "repl",
        }
    }
}

/// Servizio governato di un notebook con gli stessi percorsi di `RagWorkspace`: lock e
/// staging in `<locale>/rag/notebooks`, journal in `<autorità>/governance/rag/notebooks/<id>.json`.
pub(crate) fn governed_service(
    roots: &WorkspaceRoots,
    store: &str,
    initialize_if_missing: bool,
    projection: Box<dyn NotebookProjection>,
) -> Result<RagNotebookApplicationService, String> {
    let runtime_dir = roots.notebook_runtime_dir();
    let repository = JsonManifestRepository::new(&roots.authority, &runtime_dir);
    let store_dir = repository.store_dir(store);
    let manager =
        RagNotebookManifestManager::load(Box::new(repository), store, initialize_if_missing)
            .map_err(|e| e.to_string())?;
    let journal = roots
        .audit_dir()
        .join(format!("{}.json", manager.snapshot().id().as_str()));
    Ok(RagNotebookApplicationService::new(
        manager,
        Box::new(JsonNotebookAuditJournal::new(journal, &runtime_dir)),
        projection,
        Box::new(FsResourceFingerprints::new(&store_dir)),
        Box::new(SystemClock),
        Box::new(RandomIds),
    ))
}

/// Servizio di un notebook non attivo: nessun runtime da tenere allineato.
pub(crate) fn detached_service(
    roots: &WorkspaceRoots,
    store: &str,
    initialize_if_missing: bool,
) -> Result<RagNotebookApplicationService, String> {
    governed_service(roots, store, initialize_if_missing, Box::new(NoProjection))
}

fn lock(runtime: &Mutex<RetrievalRuntime>) -> Result<MutexGuard<'_, RetrievalRuntime>, String> {
    runtime
        .lock()
        .map_err(|_| "retrieval runtime lock poisoned".to_owned())
}
