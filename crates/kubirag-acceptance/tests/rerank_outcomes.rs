//! O2-KR3: i sei `RerankOutcome` sono tutti raggiungibili dalla pipeline reale (motore BM25,
//! vettori in memoria, runtime) e ogni esito di fallimento lascia l'ordine BM25 intatto.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use kubi_kernel::identity::CanonicalAggregateId;
use kubirag_app::notebook::SystemClock;
use kubirag_app::retrieval::{
    GenerateRequest, LlmError, LlmGateway, ProjectionStore, RetrievalRuntime,
};
use kubirag_domain::config::RagConfig;
use kubirag_domain::notebook::{RagDocument, RagNotebookManifest};
use kubirag_domain::retrieval::{
    Embedding, EmbeddingProjectionDescriptor, RerankMode, RerankOutcome, SecondStageResult,
};
use kubirag_retrieval::{InMemoryVectorStore, LuceneCompatibleSearchEngine};

/// Ollama finto: la risposta di generazione è scritta dal test; l'embedding mette vicino alla
/// domanda solo i testi che contengono "alfa".
struct ScriptedLlm {
    generation: Result<String, LlmError>,
    embeddings_available: bool,
    dimensions: usize,
}

impl LlmGateway for ScriptedLlm {
    fn generate(&self, _: &GenerateRequest, _: &mut dyn FnMut(&str)) -> Result<String, LlmError> {
        self.generation.clone()
    }

    fn embed(&self, _: &str, text: &str) -> Result<Vec<f32>, LlmError> {
        if !self.embeddings_available {
            return Err(LlmError::Unavailable("embedding service down".into()));
        }
        let mut vector = vec![0.0; self.dimensions];
        let close = text.contains("alfa") || text.starts_with("query:");
        vector[usize::from(!close)] = 1.0;
        Ok(vector)
    }
}

/// Proiezioni in memoria: conserva solo il profilo degli embedding.
#[derive(Default)]
struct MemoryProjections {
    profile: Mutex<Option<EmbeddingProjectionDescriptor>>,
}

impl ProjectionStore for MemoryProjections {
    fn save_documents(&self, _: &str, _: &[RagDocument]) -> Result<(), String> {
        Ok(())
    }

    fn load_embeddings(&self, _: &str) -> Result<Vec<Embedding>, String> {
        Ok(Vec::new())
    }

    fn save_embeddings(&self, _: &str, _: &[Embedding]) -> Result<(), String> {
        Ok(())
    }

    fn read_profile(&self, _: &str) -> Option<EmbeddingProjectionDescriptor> {
        self.profile.lock().unwrap().clone()
    }

    fn write_profile(
        &self,
        _: &str,
        descriptor: &EmbeddingProjectionDescriptor,
    ) -> Result<(), String> {
        *self.profile.lock().unwrap() = Some(descriptor.clone());
        Ok(())
    }
}

const QUESTION: &str = "query: kubirag";

/// Quattro documenti con frequenze decrescenti del termine: l'ordine BM25 è d1, d2, d3, d4.
fn documents() -> Vec<RagDocument> {
    [
        ("d1", "kubirag kubirag kubirag kubirag note"),
        ("d2", "kubirag kubirag kubirag note note"),
        ("d3", "kubirag kubirag note note note"),
        ("d4", "kubirag alfa note note note note"),
    ]
    .into_iter()
    .map(|(id, content)| {
        let mut document = RagDocument::new(id, content);
        document.metadata.insert("path".into(), format!("{id}.md"));
        document
    })
    .collect()
}

fn runtime(llm: ScriptedLlm) -> RetrievalRuntime {
    let mut runtime = RetrievalRuntime::new(
        Box::new(LuceneCompatibleSearchEngine::new()),
        Box::new(InMemoryVectorStore::new()),
        Arc::new(llm),
        Arc::new(MemoryProjections::default()),
        Arc::new(SystemClock),
    );
    let manifest = RagNotebookManifest::empty(CanonicalAggregateId::random(), "kr3").unwrap();
    runtime.open(&manifest, documents()).unwrap();
    runtime
}

/// Pesi di fusione sbilanciati sul vettore, così il documento vicino alla domanda sale.
fn vector_heavy() -> RagConfig {
    RagConfig {
        hybrid_search_lucene_weight: 0.3,
        hybrid_search_vector_weight: 0.7,
        ..RagConfig::default()
    }
}

fn llm(generation: Result<&str, LlmError>) -> ScriptedLlm {
    ScriptedLlm {
        generation: generation.map(str::to_owned),
        embeddings_available: true,
        dimensions: 2,
    }
}

fn ids(results: &[kubirag_domain::retrieval::SearchResult]) -> Vec<String> {
    results.iter().map(|r| r.document_id.clone()).collect()
}

/// Esegue il secondo stadio e lo confronta con il primo.
fn second_stage(
    runtime: &mut RetrievalRuntime,
    config: &RagConfig,
    mode: Option<RerankMode>,
) -> (Vec<String>, SecondStageResult) {
    let selection = runtime.select_context(config, QUESTION, mode);
    let bm25 = ids(&selection.search_results);
    assert_eq!(bm25, ["d1", "d2", "d3", "d4"], "ordine BM25 di partenza");
    (bm25, selection.second_stage)
}

struct Scenario {
    name: &'static str,
    expected: RerankOutcome,
    run: fn() -> (Vec<String>, SecondStageResult),
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "llm riordina",
            expected: RerankOutcome::Reordered,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok(r#"{"ranking":[4,1,2,3]}"#))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "embedding riordina",
            expected: RerankOutcome::Reordered,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok(""))),
                    &vector_heavy(),
                    Some(RerankMode::Embedding),
                )
            },
        },
        Scenario {
            name: "llm conferma l'ordine",
            expected: RerankOutcome::Unchanged,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok(r#"{"ranking":[1,2,3,4]}"#))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "llm fuori tempo",
            expected: RerankOutcome::Timeout,
            run: || {
                second_stage(
                    &mut runtime(llm(Err(LlmError::Timeout))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "llm risponde senza indici",
            expected: RerankOutcome::ParseFailed,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok("non saprei proprio"))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "llm non raggiungibile",
            expected: RerankOutcome::Error,
            run: || {
                second_stage(
                    &mut runtime(llm(Err(LlmError::Unavailable("connection refused".into())))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "llm con indice fuori da Int",
            expected: RerankOutcome::Error,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok(r#"{"ranking":[99999999999]}"#))),
                    &RagConfig::default(),
                    Some(RerankMode::Llm),
                )
            },
        },
        Scenario {
            name: "embedding non disponibile",
            expected: RerankOutcome::Error,
            run: || {
                let mut down = llm(Ok(""));
                down.embeddings_available = false;
                second_stage(
                    &mut runtime(down),
                    &RagConfig::default(),
                    Some(RerankMode::Embedding),
                )
            },
        },
        Scenario {
            name: "hybrid con vettori di dimensione diversa",
            expected: RerankOutcome::Error,
            // Vettori dei documenti a 2 dimensioni, domanda a 3: la ricerca vettoriale li rifiuta.
            run: || {
                second_stage(
                    &mut runtime_with_vectors_of(2, 3),
                    &RagConfig::default(),
                    Some(RerankMode::Hybrid),
                )
            },
        },
        Scenario {
            name: "mode none",
            expected: RerankOutcome::Skipped,
            run: || {
                second_stage(
                    &mut runtime(llm(Ok(""))),
                    &RagConfig::default(),
                    Some(RerankMode::None),
                )
            },
        },
        Scenario {
            name: "default embedding senza profilo compatibile",
            expected: RerankOutcome::Skipped,
            run: || second_stage(&mut runtime(llm(Ok(""))), &RagConfig::default(), None),
        },
    ]
}

/// Runtime con i vettori dei documenti a `stored` dimensioni e domande a `query` dimensioni.
fn runtime_with_vectors_of(stored: usize, query: usize) -> RetrievalRuntime {
    struct Switching {
        stored: usize,
        query: usize,
    }
    impl LlmGateway for Switching {
        fn generate(
            &self,
            _: &GenerateRequest,
            _: &mut dyn FnMut(&str),
        ) -> Result<String, LlmError> {
            Ok(String::new())
        }
        fn embed(&self, _: &str, text: &str) -> Result<Vec<f32>, LlmError> {
            let size = if text.starts_with("query:") {
                self.query
            } else {
                self.stored
            };
            let mut vector = vec![0.0; size];
            vector[0] = 1.0;
            Ok(vector)
        }
    }
    let config = RagConfig::default();
    let mut runtime = RetrievalRuntime::new(
        Box::new(LuceneCompatibleSearchEngine::new()),
        Box::new(InMemoryVectorStore::new()),
        Arc::new(Switching { stored, query }),
        Arc::new(MemoryProjections::default()),
        Arc::new(SystemClock),
    );
    let manifest = RagNotebookManifest::empty(CanonicalAggregateId::random(), "kr3").unwrap();
    runtime.open(&manifest, documents()).unwrap();
    runtime.backfill_embeddings(&config, false).unwrap();
    runtime
}

#[test]
fn every_rerank_outcome_is_reachable_and_failures_keep_the_bm25_order() {
    let mut covered = HashSet::new();
    let mut divergences = Vec::new();
    for scenario in scenarios() {
        let (bm25, stage) = (scenario.run)();
        covered.insert(stage.outcome);
        if stage.outcome != scenario.expected {
            divergences.push(format!(
                "{}: {} invece di {}",
                scenario.name,
                stage.outcome.name(),
                scenario.expected.name()
            ));
        }
        let keeps_bm25 = ids(&stage.results) == bm25;
        let must_keep = !matches!(stage.outcome, RerankOutcome::Reordered);
        if must_keep && !keeps_bm25 {
            divergences.push(format!(
                "{}: esito {} ma ordine {:?} invece di BM25 {bm25:?}",
                scenario.name,
                stage.outcome.name(),
                ids(&stage.results)
            ));
        }
        if stage.outcome == RerankOutcome::Reordered && keeps_bm25 {
            divergences.push(format!("{}: REORDERED con l'ordine BM25", scenario.name));
        }
    }
    let missing: Vec<_> = RerankOutcome::ALL
        .iter()
        .filter(|o| !covered.contains(o))
        .map(|o| o.name())
        .collect();
    assert!(missing.is_empty(), "esiti non coperti: {missing:?}");
    kubirag_acceptance::golden::assert_no_divergence("O2-KR3 RerankOutcome", divergences);
}

#[test]
fn the_default_embedding_mode_runs_once_the_profile_is_current() {
    let config = vector_heavy();
    let mut runtime = runtime(llm(Ok("")));
    let (created, total) = runtime.backfill_embeddings(&config, false).unwrap();
    assert_eq!((created, total), (4, 4));
    let (_, stage) = second_stage(&mut runtime, &config, None);
    assert_eq!(stage.mode, RerankMode::Embedding);
    assert_eq!(stage.outcome, RerankOutcome::Reordered);
    assert_eq!(
        ids(&stage.results)[0],
        "d4",
        "il documento vicino alla domanda sale"
    );
    assert_eq!((stage.embeddings_used, stage.embeddings_total), (4, 4));
}
