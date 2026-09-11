use std::path::Path;

use poqi_search_semantic::SemanticEmbedder;

/// Abstraction over the embedder so tests can supply a lightweight stand-in.
pub(crate) trait EmbeddingProvider {
    fn encode(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
    fn backend_label(&self) -> &'static str;
    fn take_fallback_note(&self) -> Option<String>;
    fn model_path(&self) -> &Path;
}

impl EmbeddingProvider for SemanticEmbedder {
    fn encode(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        SemanticEmbedder::encode(self, texts)
    }

    fn backend_label(&self) -> &'static str {
        SemanticEmbedder::backend_label(self)
    }

    fn take_fallback_note(&self) -> Option<String> {
        SemanticEmbedder::take_fallback_note(self)
    }

    fn model_path(&self) -> &Path {
        SemanticEmbedder::model_path(self)
    }
}
