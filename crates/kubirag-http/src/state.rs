//! Stato del server: il notebook attivo, la configurazione e gli archivi condivisi tra le
//! richieste. Ogni richiesta che tocca il notebook lavora sotto un unico lock, in un thread
//! bloccante, come i controller Javalin sotto il `synchronized` del gateway di governance.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use kubi_kernel::governance::OperationChannel;
use kubirag_app::conversation::{ChatSessionDirectory, RoleStore};
use kubirag_app::notebook::RandomIds;
use kubirag_app::retrieval::{
    import_sources, GenerateRequest, LlmError, LlmGateway, ModelCatalog, ModelContext,
    SourceImportOptions,
};
use kubirag_app::studio::TemplateLibrary;
use kubirag_domain::config::RagConfig;
use kubirag_llm::OllamaGateway;
use kubirag_store_json::{
    FsSourceLoader, FsTemplateLibrary, JsonChatSessionStore, JsonRoleStore, RagConfigCodec,
};
use kubirag_workspace::{Notebook, Workspace, WorkspaceRoots};

/// Come avviare il server: radici del workspace, directory di lavoro (dove `POST
/// /documents/upload` salva i file, come la directory corrente della JVM), home dell'utente
/// (libreria dei modelli), notebook iniziale e configurazione. Senza notebook si apre quello
/// predefinito e nessuno risulta attivo in `/stores`, come `RagServer` senza `storeName`.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub roots: WorkspaceRoots,
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub store: Option<String>,
    pub config: RagConfig,
}

/// Il gateway Ollama della configurazione corrente: `PUT /config` può cambiare
/// `ollamaBaseUrl`, e Kotlin lo rilegge a ogni chiamata.
pub struct ConfigurableLlm {
    current: RwLock<Arc<OllamaGateway>>,
}

impl ConfigurableLlm {
    pub fn new(base_url: &str) -> Self {
        Self {
            current: RwLock::new(Arc::new(OllamaGateway::from_config(base_url))),
        }
    }

    pub fn reconfigure(&self, base_url: &str) {
        let gateway = Arc::new(OllamaGateway::from_config(base_url));
        *self.current.write().unwrap_or_else(|e| e.into_inner()) = gateway;
    }

    fn gateway(&self) -> Arc<OllamaGateway> {
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl LlmGateway for ConfigurableLlm {
    fn generate(
        &self,
        request: &GenerateRequest,
        on_chunk: &mut dyn FnMut(&str),
    ) -> Result<String, LlmError> {
        self.gateway().generate(request, on_chunk)
    }

    fn embed(&self, model: &str, text: &str) -> Result<Vec<f32>, LlmError> {
        self.gateway().embed(model, text)
    }
}

impl ModelCatalog for ConfigurableLlm {
    fn models(&self) -> Vec<ModelContext> {
        self.gateway().models()
    }
}

/// Lo stato mutabile del server.
pub struct Server {
    pub workspace: Workspace,
    pub cwd: PathBuf,
    pub config: RagConfig,
    pub llm: Arc<ConfigurableLlm>,
    pub notebook: Notebook,
    /// `CtrlStores.currentStoreName`: `None` dopo la cancellazione del notebook attivo.
    pub current_store: Option<String>,
    /// `notebookEmbeddingModelOverrides`: il modello del profilo del notebook, scelto con
    /// `useNotebookProfile`, vince su quello della configurazione.
    pub embedding_overrides: HashMap<String, String>,
    pub roles: Arc<dyn RoleStore>,
    pub templates: Arc<dyn TemplateLibrary>,
}

impl Server {
    fn open(options: ServerOptions) -> Result<Self, String> {
        let llm = Arc::new(ConfigurableLlm::new(&options.config.ollama_base_url));
        let notebook = Notebook::open(
            options.roots.clone(),
            options
                .store
                .as_deref()
                .unwrap_or(kubirag_workspace::DEFAULT_STORE),
            options.config.clone(),
            llm.clone(),
            OperationChannel::ApiWeb,
        )?;
        Ok(Self {
            workspace: Workspace::new(options.roots.clone()),
            cwd: options.cwd,
            config: options.config,
            llm,
            notebook,
            current_store: options.store,
            embedding_overrides: HashMap::new(),
            roles: Arc::new(JsonRoleStore::new(&options.roots.local)),
            templates: Arc::new(FsTemplateLibrary::new(&options.roots.local, &options.home)),
        })
    }

    pub fn roots(&self) -> &WorkspaceRoots {
        self.workspace.roots()
    }

    /// Il nome del notebook caricato nel runtime (`ragSystem.currentStoreName`).
    pub fn active_name(&self) -> String {
        self.notebook.manifest().name().to_owned()
    }

    /// `withActiveService` rilegge il manifest a ogni richiesta: le modifiche scritte da un
    /// altro processo (anche Kotlin) arrivano nel runtime prima di rispondere.
    pub fn sync(&mut self) -> Result<(), String> {
        self.notebook.service().refresh().map(|_| ())
    }

    /// `effectiveEmbeddingConfig`: la configurazione con l'eventuale modello del profilo.
    pub fn effective_config(&self) -> RagConfig {
        let mut config = self.config.clone();
        if let Some(model) = self.embedding_overrides.get(&self.active_name()) {
            config.embedding_model_name = model.clone();
        }
        config
    }

    /// `ragConfig.storeConfig()`.
    pub fn store_config(&self) -> Result<(), String> {
        RagConfigCodec::write(&self.config, &self.roots().local)
    }

    /// `ragSystem.restore(name)` + `activateStore(name)`: il runtime passa al notebook.
    pub fn activate(&mut self, name: &str) -> Result<(), String> {
        self.notebook = Notebook::open(
            self.roots().clone(),
            name,
            self.config.clone(),
            self.llm.clone(),
            OperationChannel::ApiWeb,
        )?;
        self.current_store = Some(name.to_owned());
        Ok(())
    }
}

struct Shared {
    server: Mutex<Server>,
    sessions: ChatSessionDirectory,
}

/// Lo stato condiviso dagli handler.
#[derive(Clone)]
pub struct AppState {
    shared: Arc<Shared>,
}

impl AppState {
    pub fn open(options: ServerOptions) -> Result<Self, String> {
        let sessions =
            ChatSessionDirectory::new(Arc::new(JsonChatSessionStore::new(&options.roots.local)));
        Ok(Self {
            shared: Arc::new(Shared {
                server: Mutex::new(Server::open(options)?),
                sessions,
            }),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Server> {
        self.shared.server.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Esegue `action` sullo stato in un thread bloccante (I/O su file, Lucene, Ollama).
    pub async fn run<T: Send + 'static>(
        &self,
        action: impl FnOnce(&mut Server) -> T + Send + 'static,
    ) -> T {
        let state = self.clone();
        tokio::task::spawn_blocking(move || action(&mut state.lock()))
            .await
            .expect("a server task panicked")
    }

    /// Come [`run`](Self::run), dal thread bloccante in cui ci si trova già.
    pub fn blocking<T>(&self, action: impl FnOnce(&mut Server) -> T) -> T {
        action(&mut self.lock())
    }

    /// `--load`: importa sorgenti nel notebook attivo prima di servire (`server-bootstrap`).
    pub fn import_sources(&self, options: &SourceImportOptions) -> Result<String, String> {
        self.blocking(|s| {
            import_sources(
                s.notebook.service(),
                &FsSourceLoader,
                &RandomIds,
                options,
                "server-bootstrap",
                OperationChannel::Cli,
                None,
            )
            .map(|summary| summary.to_string())
        })
    }

    /// Le sessioni di chat, con il loro lock: non bloccano il notebook.
    pub fn sessions(&self) -> &ChatSessionDirectory {
        &self.shared.sessions
    }

    /// Esegue `action` sulle sessioni in un thread bloccante.
    pub async fn run_sessions<T: Send + 'static>(
        &self,
        action: impl FnOnce(&ChatSessionDirectory) -> T + Send + 'static,
    ) -> T {
        let state = self.clone();
        tokio::task::spawn_blocking(move || action(state.sessions()))
            .await
            .expect("a session task panicked")
    }
}
