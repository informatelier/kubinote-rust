//! Composition root: risolve il workspace come `KubiConfig` e collega gli adattatori
//! (JSON, motore BM25, vettori in memoria, Ollama) ai servizi applicativi.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use kubi_kernel::governance::OperationChannel;
use kubi_kernel::workspace::QbWorkspaceRootResolver;
use kubirag_app::notebook::{
    RagNotebookApplicationService, RagNotebookManifestManager, RandomIds, SystemClock,
};
use kubirag_app::retrieval::{
    DocumentMaterializer, LlmGateway, RetrievalRuntime, RuntimeProjection,
};
use kubirag_domain::config::RagConfig;
use kubirag_retrieval::{InMemoryVectorStore, LuceneCompatibleSearchEngine};
use kubirag_store_json::{
    FsDocumentMaterializer, FsResourceFingerprints, JsonManifestRepository,
    JsonNotebookAuditJournal, JsonProjectionStore, RagConfigCodec,
};

/// Notebook aperto quando non se ne sceglie uno (`NotePaths.DEFAULT_STORE`).
pub const DEFAULT_STORE: &str = "kubirag";

/// Le due radici del workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoots {
    /// `.qb/`: configurazione e proiezioni locali.
    pub local: PathBuf,
    /// `_qb/` (o quella di `qb-project.json`): manifest, journal, note.
    pub authority: PathBuf,
}

impl WorkspaceRoots {
    /// `KubiConfig.qbLocalPath()` e `qbAuthorityDir()`: `QB_ROOT_PATH` sceglie la radice
    /// locale (default `.qb/` nella directory corrente), `QB_AUTHORITY_PATH` quella di
    /// autorità; senza override l'autorità viene da `qb-project.json` del progetto.
    pub fn resolve(cwd: &Path, env: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let local = env("QB_ROOT_PATH").map_or_else(|| cwd.join(".qb"), |p| cwd.join(p));
        let authority = match env("QB_AUTHORITY_PATH") {
            Some(path) => cwd.join(path),
            None => {
                let project = QbWorkspaceRootResolver::discover_project_root(cwd);
                let roots = QbWorkspaceRootResolver::resolve(&project, false);
                if !roots.usable() {
                    return Err(roots
                        .issues
                        .iter()
                        .map(|i| format!("{}: {}", i.code(), i.message()))
                        .collect::<Vec<_>>()
                        .join("; "));
                }
                roots.authority_root
            }
        };
        Ok(Self { local, authority })
    }
}

/// `qb-rag.json` della radice locale, o la configurazione di default.
pub fn read_config(roots: &WorkspaceRoots) -> Result<RagConfig, String> {
    Ok(RagConfigCodec::read(&roots.local)?.unwrap_or_default())
}

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
        let repository = JsonManifestRepository::new(&roots.authority, &roots.local);
        let store_dir = repository.store_dir(store);
        let manager = RagNotebookManifestManager::load(Box::new(repository), store, true)
            .map_err(|e| e.to_string())?;
        let service = RagNotebookApplicationService::new(
            manager,
            Box::new(JsonNotebookAuditJournal::for_store(&store_dir, store)),
            Box::new(RuntimeProjection::new(
                runtime.clone(),
                materializer.clone(),
            )),
            Box::new(FsResourceFingerprints::new(&store_dir)),
            Box::new(SystemClock),
            Box::new(RandomIds),
        );
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
        if self.channel == OperationChannel::Cli {
            "cli"
        } else {
            "repl"
        }
    }
}

fn lock(runtime: &Mutex<RetrievalRuntime>) -> Result<MutexGuard<'_, RetrievalRuntime>, String> {
    runtime
        .lock()
        .map_err(|_| "retrieval runtime lock poisoned".to_owned())
}
