#![warn(clippy::all, clippy::pedantic)]

mod embedder;
mod inputs;
mod prompts;
mod ranking;

pub use embedder::{
    directml_is_available, initialize_runtime, ExecutionStrategy, SemanticEmbedder,
};
pub use prompts::NULL_SENTINEL;
pub use prompts::{format_query_prompt, format_row_prompt};
#[cfg(test)]
pub(crate) use ranking::finalize_ranking;
pub use ranking::{
    cosine_similarity, embed_and_rank, l2_normalize, rank_with_embeddings, ProgressCallback,
    RankedRow, SearchOptions,
};

#[cfg(test)]
mod tests;
