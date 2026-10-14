use crate::notebook::RagDocument;

pub const CHUNK_INDEX: &str = "chunk_index";
pub const PARENT_ID: &str = "parent_id";
pub const PARENT_CHUNK_COUNT: &str = "parent_chunk_count";
const CHUNK_SEPARATOR: &str = "_chunk_";

/// Strategia di chunking (`ChunkConfig`): finestre di parole con sovrapposizione.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkConfig {
    chunk_size: usize,
    overlap: usize,
    min_chunk_size: usize,
}

impl ChunkConfig {
    pub fn new(chunk_size: usize, overlap: usize, min_chunk_size: usize) -> Result<Self, String> {
        let check = |ok: bool, message: &str| if ok { Ok(()) } else { Err(message.to_owned()) };
        check(chunk_size > 0, "chunkSize must be positive")?;
        check(overlap < chunk_size, "overlap must be less than chunkSize")?;
        check(min_chunk_size > 0, "minChunkSize must be positive")?;
        check(
            min_chunk_size <= chunk_size,
            "minChunkSize must be less than or equal to chunkSize",
        )?;
        Ok(Self {
            chunk_size,
            overlap,
            min_chunk_size,
        })
    }

    /// `ChunkConfig(chunkSize, overlap)` con `minChunkSize` di default (256).
    pub fn sized(chunk_size: usize, overlap: usize) -> Result<Self, String> {
        Self::new(chunk_size, overlap, Self::default().min_chunk_size)
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn overlap(&self) -> usize {
        self.overlap
    }

    pub fn min_chunk_size(&self) -> usize {
        self.min_chunk_size
    }

    pub fn step_size(&self) -> usize {
        self.chunk_size - self.overlap
    }
}

impl Default for ChunkConfig {
    /// 768 parole, 128 di sovrapposizione, chunk minimo 256.
    fn default() -> Self {
        Self {
            chunk_size: 768,
            overlap: 128,
            min_chunk_size: 256,
        }
    }
}

/// `DocumentChunkingService`: spezza un documento in chunk sovrapposti di parole.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocumentChunker {
    config: ChunkConfig,
}

impl DocumentChunker {
    pub fn new(config: ChunkConfig) -> Self {
        Self { config }
    }

    /// `text.split(Regex("\\s+")).filter { it.isNotBlank() }`: `\s` è lo spazio ASCII di Java,
    /// `isNotBlank` usa lo spazio Unicode.
    pub fn tokenize(text: &str) -> Vec<&str> {
        text.split(is_java_space)
            .filter(|word| !word.chars().all(super::text::is_kotlin_whitespace))
            .collect()
    }

    /// Un documento con al più `minChunkSize` parole resta intero (stesso id); gli altri
    /// diventano `<id>_chunk_<n>` con `parent_id`, `chunk_index` e `parent_chunk_count`.
    pub fn chunk(&self, document: &RagDocument) -> Vec<RagDocument> {
        let words = Self::tokenize(&document.content);
        if words.len() <= self.config.min_chunk_size {
            return vec![document.clone()];
        }
        let mut chunks: Vec<RagDocument> = Vec::new();
        let mut start = 0;
        while start < words.len() {
            let end = (start + self.config.chunk_size).min(words.len());
            let window = &words[start..end];
            if window.len() >= self.config.min_chunk_size || end == words.len() {
                let mut metadata = document.metadata.clone();
                metadata.insert(CHUNK_INDEX.into(), chunks.len().to_string());
                metadata.insert(PARENT_ID.into(), document.id.clone());
                metadata.insert(PARENT_CHUNK_COUNT.into(), String::new());
                chunks.push(RagDocument {
                    id: format!("{}{CHUNK_SEPARATOR}{}", document.id, chunks.len()),
                    content: window.join(" "),
                    metadata,
                    resource_ref: None,
                });
            }
            start += self.config.step_size();
            if end == words.len() {
                break;
            }
        }
        let count = chunks.len().to_string();
        for chunk in &mut chunks {
            chunk
                .metadata
                .insert(PARENT_CHUNK_COUNT.into(), count.clone());
        }
        chunks
    }

    /// `guides/a.md_chunk_3` → `guides/a.md`.
    pub fn parent_id(chunk_id: &str) -> &str {
        chunk_id
            .split_once(CHUNK_SEPARATOR)
            .map_or(chunk_id, |(parent, _)| parent)
    }
}

/// `\s` di `java.util.regex`: solo spazi ASCII.
pub(crate) fn is_java_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(words: usize) -> RagDocument {
        let content = (0..words)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        RagDocument::new("doc", content)
    }

    #[test]
    fn short_documents_are_not_chunked() {
        let chunks = DocumentChunker::default().chunk(&document(256));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].id, "doc");
    }

    #[test]
    fn long_documents_overlap_and_carry_their_parent() {
        let chunker = DocumentChunker::new(ChunkConfig::new(10, 2, 4).unwrap());
        let chunks = chunker.chunk(&document(20));
        let ids: Vec<_> = chunks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["doc_chunk_0", "doc_chunk_1", "doc_chunk_2"]);
        assert!(chunks[1].content.starts_with("w8 w9"));
        assert_eq!(chunks[2].content, "w16 w17 w18 w19");
        assert!(chunks.iter().all(|c| c.metadata[PARENT_CHUNK_COUNT] == "3"));
        assert_eq!(DocumentChunker::parent_id("doc_chunk_2"), "doc");
    }

    #[test]
    fn invalid_configs_are_rejected_with_the_kotlin_messages() {
        assert_eq!(
            ChunkConfig::new(10, 10, 4).unwrap_err(),
            "overlap must be less than chunkSize"
        );
        assert!(
            ChunkConfig::sized(100, 10).is_err(),
            "minChunkSize 256 > 100"
        );
    }
}
