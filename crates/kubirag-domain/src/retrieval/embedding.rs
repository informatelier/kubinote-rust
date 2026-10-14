//! Proiezione degli embedding: un vettore per documento (media dei vettori dei chunk) e il
//! descrittore portabile `embedding-profile.json` che dice come interpretare i vettori.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub const CHUNK_MEAN_V1: &str = "chunk-mean-v1";

/// Un embedding di documento (`Embedding`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub vector: Vec<f32>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub metadata: IndexMap<String, String>,
}

/// Media componente per componente dei vettori dei chunk, accumulata in `f64` come in Kotlin.
pub fn chunk_mean(vectors: &[Vec<f32>]) -> Result<Vec<f32>, String> {
    if vectors.is_empty() || vectors.iter().any(Vec::is_empty) {
        return Err("Embedder returned an empty vector".into());
    }
    let dimensions = vectors[0].len();
    if vectors.iter().any(|v| v.len() != dimensions) {
        return Err("Embedder returned vectors with inconsistent dimensions".into());
    }
    Ok((0..dimensions)
        .map(|i| (vectors.iter().map(|v| v[i] as f64).sum::<f64>() / vectors.len() as f64) as f32)
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmbeddingProjectionState {
    Missing,
    Unknown,
    Building,
    Current,
    Stale,
    Partial,
    Failed,
}

impl EmbeddingProjectionState {
    pub fn name(self) -> &'static str {
        match self {
            Self::Missing => "MISSING",
            Self::Unknown => "UNKNOWN",
            Self::Building => "BUILDING",
            Self::Current => "CURRENT",
            Self::Stale => "STALE",
            Self::Partial => "PARTIAL",
            Self::Failed => "FAILED",
        }
    }
}

fn default_version() -> String {
    "1".into()
}

fn default_algorithm() -> String {
    CHUNK_MEAN_V1.into()
}

/// `EmbeddingProjectionDescriptor`: viaggia con il notebook condiviso; i vettori restano locali.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingProjectionDescriptor {
    #[serde(default = "default_version")]
    pub version: String,
    pub model: String,
    #[serde(default)]
    pub vector_dimensions: Option<i32>,
    pub chunk_size: i32,
    pub chunk_overlap: i32,
    #[serde(default = "default_algorithm")]
    pub algorithm_version: String,
    pub state: EmbeddingProjectionState,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub total_documents: i32,
    #[serde(default)]
    pub completed_documents: i32,
    #[serde(default)]
    pub failed_documents: i32,
    #[serde(default)]
    pub failures: Vec<String>,
}

impl EmbeddingProjectionDescriptor {
    /// Il descrittore scritto all'inizio di un backfill.
    pub fn building(
        model: &str,
        chunk_size: i32,
        chunk_overlap: i32,
        total_documents: usize,
    ) -> Self {
        Self {
            version: default_version(),
            model: model.into(),
            vector_dimensions: None,
            chunk_size,
            chunk_overlap,
            algorithm_version: default_algorithm(),
            state: EmbeddingProjectionState::Building,
            completed_at: None,
            total_documents: total_documents as i32,
            completed_documents: 0,
            failed_documents: 0,
            failures: Vec::new(),
        }
    }

    /// `EmbeddingProjectionStore.completed`: `CURRENT` senza errori, altrimenti `PARTIAL`.
    pub fn completed(
        &self,
        completed: usize,
        failures: Vec<String>,
        dimensions: Option<usize>,
        completed_at: String,
    ) -> Self {
        Self {
            state: if failures.is_empty() {
                EmbeddingProjectionState::Current
            } else {
                EmbeddingProjectionState::Partial
            },
            vector_dimensions: dimensions.map(|d| d as i32),
            completed_at: Some(completed_at),
            completed_documents: completed as i32,
            failed_documents: failures.len() as i32,
            failures,
            ..self.clone()
        }
    }
}

/// La configurazione corrente con cui confrontare il descrittore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddingSettings<'a> {
    pub model: &'a str,
    pub chunk_size: i32,
    pub chunk_overlap: i32,
}

/// `EmbeddingProjectionStatus`: i vettori sono confrontabili con la configurazione?
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingProjectionStatus {
    pub state: EmbeddingProjectionState,
    pub compatible: bool,
    pub configured_model: String,
    pub descriptor: Option<EmbeddingProjectionDescriptor>,
    pub reasons: Vec<String>,
}

impl EmbeddingProjectionStatus {
    /// `EmbeddingProjectionStore.status`.
    pub fn evaluate(
        descriptor: Option<EmbeddingProjectionDescriptor>,
        settings: EmbeddingSettings,
    ) -> Self {
        let Some(descriptor) = descriptor else {
            return Self {
                state: EmbeddingProjectionState::Unknown,
                compatible: false,
                configured_model: settings.model.into(),
                descriptor: None,
                reasons: vec![
                    "Embedding legacy o assenti: manca il descrittore della proiezione".into(),
                ],
            };
        };
        let mut reasons = Vec::new();
        if descriptor.model != settings.model {
            reasons.push(format!(
                "Modello: {} -> {}",
                descriptor.model, settings.model
            ));
        }
        if descriptor.chunk_size != settings.chunk_size {
            reasons.push(format!(
                "Chunk size: {} -> {}",
                descriptor.chunk_size, settings.chunk_size
            ));
        }
        if descriptor.chunk_overlap != settings.chunk_overlap {
            reasons.push(format!(
                "Chunk overlap: {} -> {}",
                descriptor.chunk_overlap, settings.chunk_overlap
            ));
        }
        if descriptor.algorithm_version != CHUNK_MEAN_V1 {
            reasons.push(format!(
                "Algoritmo non supportato: {}",
                descriptor.algorithm_version
            ));
        }
        if descriptor.state != EmbeddingProjectionState::Current {
            reasons.push(format!("Stato proiezione: {}", descriptor.state.name()));
        }
        let compatible = reasons.is_empty();
        Self {
            state: if compatible {
                EmbeddingProjectionState::Current
            } else {
                EmbeddingProjectionState::Stale
            },
            compatible,
            configured_model: settings.model.into(),
            descriptor: Some(descriptor),
            reasons,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: EmbeddingSettings = EmbeddingSettings {
        model: "qwen3:4b",
        chunk_size: 768,
        chunk_overlap: 128,
    };

    #[test]
    fn the_mean_rejects_empty_and_ragged_vectors() {
        assert_eq!(
            chunk_mean(&[vec![1.0, 2.0], vec![3.0, 4.0]]),
            Ok(vec![2.0, 3.0])
        );
        assert!(chunk_mean(&[]).is_err());
        assert!(chunk_mean(&[vec![1.0], vec![1.0, 2.0]]).is_err());
    }

    #[test]
    fn a_missing_descriptor_is_unknown_and_incompatible() {
        let status = EmbeddingProjectionStatus::evaluate(None, SETTINGS);
        assert_eq!(status.state, EmbeddingProjectionState::Unknown);
        assert!(!status.compatible);
    }

    #[test]
    fn a_building_or_different_profile_is_stale() {
        let building = EmbeddingProjectionDescriptor::building("qwen3:4b", 768, 128, 3);
        let status = EmbeddingProjectionStatus::evaluate(Some(building.clone()), SETTINGS);
        assert_eq!(status.reasons, ["Stato proiezione: BUILDING"]);
        let done = building.completed(3, vec![], Some(6), "t".into());
        assert!(EmbeddingProjectionStatus::evaluate(Some(done.clone()), SETTINGS).compatible);
        let other = EmbeddingSettings {
            model: "nomic",
            ..SETTINGS
        };
        let status = EmbeddingProjectionStatus::evaluate(Some(done), other);
        assert_eq!(status.state, EmbeddingProjectionState::Stale);
        assert_eq!(status.reasons, ["Modello: qwen3:4b -> nomic"]);
    }

    #[test]
    fn failures_make_the_projection_partial() {
        let base = EmbeddingProjectionDescriptor::building("m", 768, 128, 2);
        let done = base.completed(1, vec!["x".into()], None, "t".into());
        assert_eq!(done.state, EmbeddingProjectionState::Partial);
        assert_eq!(done.failed_documents, 1);
    }
}
