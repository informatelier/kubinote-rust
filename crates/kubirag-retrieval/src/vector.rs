use kubirag_app::retrieval::VectorStore;
use kubirag_domain::retrieval::{cosine, Embedding};

/// `InMemoryVectorStore`: lista in ordine di inserimento, coseno a scansione completa.
#[derive(Debug, Default)]
pub struct InMemoryVectorStore {
    embeddings: Vec<Embedding>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl VectorStore for InMemoryVectorStore {
    fn add(&mut self, embedding: Embedding) {
        self.embeddings.push(embedding);
    }

    fn remove(&mut self, id: &str) -> Option<Embedding> {
        let index = self.embeddings.iter().position(|e| e.id == id)?;
        Some(self.embeddings.remove(index))
    }

    fn get(&self, id: &str) -> Option<&Embedding> {
        self.embeddings.iter().find(|e| e.id == id)
    }

    fn find_similar(
        &self,
        query: &[f32],
        threshold: f64,
        limit: usize,
    ) -> Result<Vec<Embedding>, String> {
        let mut scored = Vec::with_capacity(self.embeddings.len());
        for embedding in &self.embeddings {
            if embedding.vector.len() != query.len() {
                return Err("I vettori devono avere la stessa dimensione".into());
            }
            scored.push((embedding, cosine(query, &embedding.vector)));
        }
        scored.retain(|(_, similarity)| f64::from(*similarity) >= threshold);
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(embedding, _)| embedding.clone())
            .collect())
    }

    fn embeddings(&self) -> Vec<Embedding> {
        self.embeddings.clone()
    }

    fn replace_all(&mut self, embeddings: Vec<Embedding>) {
        self.embeddings = embeddings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedding(id: &str, vector: &[f32]) -> Embedding {
        Embedding {
            id: id.into(),
            vector: vector.to_vec(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn similar_vectors_are_filtered_and_sorted() {
        let mut store = InMemoryVectorStore::new();
        store.add(embedding("a", &[1.0, 0.0]));
        store.add(embedding("b", &[1.0, 1.0]));
        store.add(embedding("c", &[0.0, 1.0]));
        let ids: Vec<_> = store
            .find_similar(&[0.0, 1.0], 0.5, 5)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, ["c", "b"]);
        assert!(store.find_similar(&[1.0], 0.0, 5).is_err());
    }
}
