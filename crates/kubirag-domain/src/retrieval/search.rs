use indexmap::IndexMap;

/// Un risultato del primo stadio (BM25) o del secondo. `document_id` è l'id del documento
/// padre: i chunk sono un dettaglio dell'indice e non escono dal motore.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub rank: usize,
    pub document_id: String,
    /// Testo del chunk che ha prodotto il punteggio (usato dal rerank LLM).
    pub content: String,
    pub score: f32,
    pub explanation: String,
}

/// Strategia del secondo stadio, tra la ricerca BM25 e la selezione del contesto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RerankMode {
    /// Ordine BM25 puro.
    None,
    /// Fusione BM25 + coseno ristretta ai candidati BM25: riordina, non aggiunge.
    Embedding,
    /// Fusione BM25 + ricerca vettoriale su tutto lo store: può introdurre documenti nuovi.
    Hybrid,
    /// Rerank listwise via LLM con indici numerici e output JSON.
    Llm,
}

impl RerankMode {
    pub const ALL: [Self; 4] = [Self::None, Self::Embedding, Self::Hybrid, Self::Llm];

    /// `RerankMode.parse`: nome senza distinzione di maiuscole, spazi esterni ignorati.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name().eq_ignore_ascii_case(value.trim()))
    }

    /// Nome della costante Kotlin (`NONE`, `EMBEDDING`…).
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Embedding => "EMBEDDING",
            Self::Hybrid => "HYBRID",
            Self::Llm => "LLM",
        }
    }

    /// Come lo mostra `/stat`.
    pub fn label(self) -> String {
        self.name().to_ascii_lowercase()
    }

    /// Default risolto: `embedding` se il rerank è abilitato, altrimenti `none`.
    pub fn default_for(re_ranking_enabled: bool) -> Self {
        if re_ranking_enabled {
            Self::Embedding
        } else {
            Self::None
        }
    }
}

/// Esito osservabile del secondo stadio. Ogni fallimento degrada esplicitamente all'ordine BM25.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RerankOutcome {
    /// L'ordine è cambiato rispetto a BM25.
    Reordered,
    /// Lo stadio ha lavorato e ha confermato l'ordine BM25.
    Unchanged,
    /// LLM: tempo scaduto, ordine BM25 intatto.
    Timeout,
    /// LLM: risposta non interpretabile, ordine BM25 intatto.
    ParseFailed,
    /// Errore (servizio di embedding non disponibile, vettori incompatibili…), ordine BM25 intatto.
    Error,
    /// Stadio non eseguito (mode `none`, 0-1 candidati, nessun embedding utilizzabile).
    Skipped,
}

impl RerankOutcome {
    pub const ALL: [Self; 6] = [
        Self::Reordered,
        Self::Unchanged,
        Self::Timeout,
        Self::ParseFailed,
        Self::Error,
        Self::Skipped,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Reordered => "REORDERED",
            Self::Unchanged => "UNCHANGED",
            Self::Timeout => "TIMEOUT",
            Self::ParseFailed => "PARSE_FAILED",
            Self::Error => "ERROR",
            Self::Skipped => "SKIPPED",
        }
    }

    /// `REORDERED` se l'ordine dei documenti è cambiato, altrimenti `UNCHANGED`.
    pub fn comparing(before: &[SearchResult], after: &[SearchResult]) -> Self {
        let ids = |results: &[SearchResult]| {
            results
                .iter()
                .map(|r| r.document_id.clone())
                .collect::<Vec<_>>()
        };
        if ids(before) == ids(after) {
            Self::Unchanged
        } else {
            Self::Reordered
        }
    }
}

/// Risultato del secondo stadio con la telemetria per `/stat`.
#[derive(Debug, Clone, PartialEq)]
pub struct SecondStageResult {
    pub results: Vec<SearchResult>,
    pub mode: RerankMode,
    pub outcome: RerankOutcome,
    pub elapsed_ms: u128,
    /// Candidati con embedding utilizzabile / candidati totali (embedding e hybrid).
    pub embeddings_used: usize,
    pub embeddings_total: usize,
    /// Embedding creati in modo lazy durante questa query.
    pub embeddings_created: usize,
    /// Similarità coseno grezza per documento, dove disponibile.
    pub vector_scores: IndexMap<String, f32>,
    /// Punteggio fuso per documento, dove calcolato.
    pub fused_scores: IndexMap<String, f32>,
}

impl SecondStageResult {
    /// Lo stadio non ha cambiato nulla: i risultati sono i candidati BM25.
    pub fn passthrough(
        candidates: Vec<SearchResult>,
        mode: RerankMode,
        outcome: RerankOutcome,
    ) -> Self {
        Self {
            results: candidates,
            mode,
            outcome,
            elapsed_ms: 0,
            embeddings_used: 0,
            embeddings_total: 0,
            embeddings_created: 0,
            vector_scores: IndexMap::new(),
            fused_scores: IndexMap::new(),
        }
    }

    pub fn with_embeddings(mut self, total: usize, created: usize) -> Self {
        self.embeddings_total = total;
        self.embeddings_created = created;
        self
    }
}

/// Statistiche della cache del lettore dell'indice, come quelle del `DirectoryReader` Lucene:
/// la prima ricerca dopo una reindicizzazione è un miss, le successive sono hit.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CacheStats {
    pub hits: u32,
    pub misses: u32,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f32 / total as f32
        }
    }
}

/// Opzioni del primo stadio (`SearchOptimizationConfig`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SearchOptions {
    pub max_results: usize,
    pub title_boost: f32,
    pub query_expansion_enabled: bool,
    pub diversification_enabled: bool,
    pub phrase_query_enabled: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 3,
            title_boost: 2.0,
            query_expansion_enabled: true,
            diversification_enabled: true,
            phrase_query_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse_case_insensitively() {
        assert_eq!(RerankMode::parse(" Hybrid "), Some(RerankMode::Hybrid));
        assert_eq!(RerankMode::parse("llm"), Some(RerankMode::Llm));
        assert_eq!(RerankMode::parse("bm25"), None);
        assert_eq!(RerankMode::Embedding.label(), "embedding");
    }
}
