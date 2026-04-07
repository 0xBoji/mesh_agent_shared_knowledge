use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::output::QueryResult;

#[derive(Debug, Clone)]
pub struct IndexConfig {
    pub extensions: Vec<String>,
    pub chunk_line_limit: usize,
    pub chunk_char_limit: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            extensions: vec![
                "rs".into(),
                "md".into(),
                "txt".into(),
                "toml".into(),
                "json".into(),
            ],
            chunk_line_limit: 40,
            chunk_char_limit: 2000,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub file_path: String,
    pub content: String,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct KnowledgeBase {
    chunks: Vec<Chunk>,
}

impl KnowledgeBase {
    #[must_use]
    pub fn new(chunks: Vec<Chunk>) -> Self {
        Self { chunks }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    #[must_use]
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<QueryResult> {
        let mut ranked = self
            .chunks
            .iter()
            .filter_map(|chunk| {
                cosine_similarity(query_embedding, &chunk.embedding).map(|similarity_score| {
                    QueryResult {
                        file_path: chunk.file_path.clone(),
                        content: chunk.content.clone(),
                        similarity_score,
                    }
                })
            })
            .collect::<Vec<_>>();

        ranked.sort_by(|left, right| {
            match right.similarity_score.partial_cmp(&left.similarity_score) {
                Some(ordering) => ordering,
                None => Ordering::Equal,
            }
        });
        ranked.truncate(top_k.min(ranked.len()));
        ranked
    }
}

pub trait EmbeddingBackend: Send + 'static {
    fn embed(&mut self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>>;
}

struct FastEmbedBackend {
    model: TextEmbedding,
}

impl EmbeddingBackend for FastEmbedBackend {
    fn embed(&mut self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.model.embed(inputs, None)
    }
}

#[derive(Clone)]
pub struct EmbeddingClient {
    backend: Arc<Mutex<Box<dyn EmbeddingBackend>>>,
}

impl EmbeddingClient {
    pub async fn new_fastembed() -> Result<Self> {
        let backend = tokio::task::spawn_blocking(|| -> Result<Box<dyn EmbeddingBackend>> {
            let options = TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(false);
            let model = TextEmbedding::try_new(options)
                .context("failed to initialize local fastembed model")?;
            Ok(Box::new(FastEmbedBackend { model }))
        })
        .await
        .map_err(|error| anyhow!("embedding task join error: {error}"))??;

        Ok(Self {
            backend: Arc::new(Mutex::new(backend)),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_backend(backend: impl EmbeddingBackend) -> Self {
        Self {
            backend: Arc::new(Mutex::new(Box::new(backend))),
        }
    }

    pub async fn embed_texts(&self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let backend = Arc::clone(&self.backend);
        tokio::task::spawn_blocking(move || {
            let mut backend = backend
                .lock()
                .map_err(|_| anyhow!("embedding backend mutex was poisoned"))?;
            backend.embed(inputs)
        })
        .await
        .map_err(|error| anyhow!("embedding task join error: {error}"))?
    }
}

pub async fn build_index(
    directory: impl Into<PathBuf>,
    embedder: &EmbeddingClient,
    config: &IndexConfig,
) -> Result<KnowledgeBase> {
    let directory = directory.into();
    let config_clone = config.clone();
    let documents =
        tokio::task::spawn_blocking(move || collect_documents(&directory, &config_clone))
            .await
            .map_err(|error| anyhow!("indexing task join error: {error}"))??;

    // RAG step 1: chunking. We split each text file into smaller semantic units so the
    // retriever can return focused snippets instead of entire files.
    let chunk_inputs = documents
        .into_iter()
        .flat_map(|document| {
            chunk_document(
                &document.path,
                &document.content,
                config.chunk_line_limit,
                config.chunk_char_limit,
            )
        })
        .collect::<Vec<_>>();

    if chunk_inputs.is_empty() {
        return Ok(KnowledgeBase::default());
    }

    // RAG step 2: embedding. Each chunk is projected into a dense vector space using a
    // local embedding model so later similarity search can compare meaning, not just text.
    let embeddings = embedder
        .embed_texts(
            chunk_inputs
                .iter()
                .map(|chunk| format!("passage: {}", chunk.content))
                .collect(),
        )
        .await?;

    let chunks = chunk_inputs
        .into_iter()
        .zip(embeddings)
        .map(|(chunk, embedding)| Chunk {
            file_path: chunk.file_path,
            content: chunk.content,
            embedding,
        })
        .collect();

    Ok(KnowledgeBase::new(chunks))
}

#[derive(Debug, Clone)]
struct SourceDocument {
    path: PathBuf,
    content: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ChunkInput {
    file_path: String,
    content: String,
}

fn collect_documents(root: &Path, config: &IndexConfig) -> Result<Vec<SourceDocument>> {
    let mut documents = Vec::new();
    walk_directory(root, &mut documents, config)?;
    Ok(documents)
}

fn walk_directory(
    path: &Path,
    documents: &mut Vec<SourceDocument>,
    config: &IndexConfig,
) -> Result<()> {
    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry = entry.with_context(|| format!("failed to inspect `{}`", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect `{}`", entry_path.display()))?;

        if file_type.is_dir() {
            walk_directory(&entry_path, documents, config)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let extension = entry_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        if !config.extensions.contains(&extension) {
            continue;
        }

        let bytes = fs::read(&entry_path)
            .with_context(|| format!("failed to read file `{}`", entry_path.display()))?;

        if bytes.contains(&0) {
            continue;
        }

        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(_) => continue,
        };

        if content.trim().is_empty() {
            continue;
        }

        documents.push(SourceDocument {
            path: entry_path,
            content,
        });
    }

    Ok(())
}

fn chunk_document(
    path: &Path,
    content: &str,
    line_limit: usize,
    char_limit: usize,
) -> Vec<ChunkInput> {
    let file_path = path.display().to_string();
    let mut chunks = Vec::new();

    for section in content.split("\n\n") {
        let trimmed = section.trim();
        if trimmed.is_empty() {
            continue;
        }

        let lines = trimmed.lines().collect::<Vec<_>>();
        let needs_line_split = lines.len() > line_limit || trimmed.len() > char_limit;

        if !needs_line_split {
            chunks.push(ChunkInput {
                file_path: file_path.clone(),
                content: trimmed.to_string(),
            });
            continue;
        }

        for line_group in lines.chunks(line_limit) {
            let grouped = line_group.join("\n");
            let grouped = grouped.trim();
            if grouped.is_empty() {
                continue;
            }

            chunks.push(ChunkInput {
                file_path: file_path.clone(),
                content: grouped.to_string(),
            });
        }
    }

    chunks
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let (dot, left_norm, right_norm) = left.iter().zip(right.iter()).fold(
        (0.0_f32, 0.0_f32, 0.0_f32),
        |(dot, left_norm, right_norm), (left_value, right_value)| {
            (
                dot + (left_value * right_value),
                left_norm + (left_value * left_value),
                right_norm + (right_value * right_value),
            )
        },
    );

    let denominator = left_norm.sqrt() * right_norm.sqrt();
    if denominator == 0.0 {
        return None;
    }

    // RAG step 3: cosine similarity. The query embedding is compared with each stored chunk
    // embedding, and the highest-scoring snippets become the retrieval result set.
    Some(dot / denominator)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::Result;

    use super::{
        Chunk, EmbeddingBackend, EmbeddingClient, IndexConfig, KnowledgeBase, build_index,
        chunk_document,
    };

    struct FakeBackend;

    impl EmbeddingBackend for FakeBackend {
        fn embed(&mut self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>> {
            Ok(inputs
                .into_iter()
                .map(|text| {
                    vec![
                        text.matches("auth").count() as f32,
                        text.matches("db").count() as f32,
                        text.len() as f32,
                    ]
                })
                .collect())
        }
    }

    #[test]
    fn chunk_document_splits_by_blank_lines_and_line_limit() {
        let content = "fn a() {}\n\nfn b() {}\n\n".to_string()
            + &(0..45)
                .map(|idx| format!("let line_{idx} = {idx};"))
                .collect::<Vec<_>>()
                .join("\n");

        let chunks = chunk_document(Path::new("src/lib.rs"), &content, 40, 2000);

        assert_eq!(chunks[0].content, "fn a() {}");
        assert_eq!(chunks[1].content, "fn b() {}");
        assert_eq!(chunks.len(), 4);
    }

    #[test]
    fn knowledge_base_ranks_chunks_by_cosine_similarity() {
        let knowledge_base = KnowledgeBase::new(vec![
            Chunk {
                file_path: "src/auth.rs".into(),
                content: "auth login flow".into(),
                embedding: vec![1.0, 0.0],
            },
            Chunk {
                file_path: "src/db.rs".into(),
                content: "db pool config".into(),
                embedding: vec![0.0, 1.0],
            },
        ]);

        let results = knowledge_base.search(&[1.0, 0.0], 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_path, "src/auth.rs");
        assert!(results[0].similarity_score > 0.99);
    }

    #[tokio::test]
    async fn build_index_recursively_collects_text_files() -> Result<()> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!("mask-indexer-{unique}"));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested)?;
        std::fs::write(root.join("auth.rs"), "fn auth() {}\n\nlet token = 1;")?;
        std::fs::write(nested.join("db.rs"), "fn db() {}")?;
        std::fs::write(root.join("blob.bin"), [0, 159, 146, 150])?;

        let embedder = EmbeddingClient::from_backend(FakeBackend);
        let config = IndexConfig::default();
        let knowledge_base = build_index(&root, &embedder, &config).await?;

        assert!(knowledge_base.len() >= 2);

        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
