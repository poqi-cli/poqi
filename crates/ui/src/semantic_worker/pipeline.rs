mod cache;
mod dataset;
mod documents;
mod embedder;
mod ranking;
use std::sync::Arc;

// This module keeps the semantic worker pieces small and composable.
pub(super) use cache::EmbeddingCache;
pub(super) use embedder::EmbeddingProvider;
pub(super) use ranking::{rank_rows, RankingError};
pub(super) type SharedEmbedder = Arc<dyn EmbeddingProvider + Send + Sync>;
