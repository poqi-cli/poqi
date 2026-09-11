mod bootstrap;
mod embedder;

pub(crate) use bootstrap::{
    init_semantic_runtime, init_semantic_runtime_with_handles, SemanticBootstrap,
};
pub(crate) use embedder::load_embedder_from_root;
