use std::path::PathBuf;

use poqi_store::SemanticRuntimePreference;
use serde::{Deserialize, Serialize};

const DEFAULT_SEMANTIC_DIM: usize = 384;
const DEFAULT_ANN_ENGINE: &str = "usearch";
const DEFAULT_MODEL_DIR: &str = "models/granite-embedding-97m-multilingual-r2";
const DEFAULT_SEMANTIC_BATCH_SIZE: usize = 96;
const DEFAULT_SEMANTIC_TOP_K: usize = 100;
const DEFAULT_SEMANTIC_SCORE_THRESHOLD: Option<f32> = Some(0.2);

fn default_semantic_dim() -> usize {
    DEFAULT_SEMANTIC_DIM
}

fn default_ann_engine() -> String {
    DEFAULT_ANN_ENGINE.to_string()
}

fn default_model_dir() -> PathBuf {
    PathBuf::from(DEFAULT_MODEL_DIR)
}

fn default_semantic_batch_size() -> usize {
    DEFAULT_SEMANTIC_BATCH_SIZE
}

fn default_semantic_top_k() -> usize {
    DEFAULT_SEMANTIC_TOP_K
}

fn default_semantic_score_threshold() -> Option<f32> {
    DEFAULT_SEMANTIC_SCORE_THRESHOLD
}

fn default_semantic_runtime_preference() -> SemanticRuntimePreference {
    SemanticRuntimePreference::Off
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    #[serde(default = "default_semantic_dim")]
    pub semantic_dim: usize,
    #[serde(default = "default_ann_engine")]
    pub ann_engine: String,
    pub rebuild_on_start: bool,
    #[serde(default = "default_model_dir")]
    pub model_dir: PathBuf,
    #[serde(default = "default_semantic_batch_size")]
    pub semantic_batch_size: usize,
    #[serde(default = "default_semantic_top_k")]
    pub semantic_top_k: usize,
    #[serde(default = "default_semantic_score_threshold")]
    pub semantic_score_threshold: Option<f32>,
    pub semantic_title_column: Option<String>,
    #[serde(default = "default_semantic_runtime_preference")]
    pub semantic_runtime_preference: SemanticRuntimePreference,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            semantic_dim: DEFAULT_SEMANTIC_DIM,
            ann_engine: DEFAULT_ANN_ENGINE.to_string(),
            rebuild_on_start: false,
            model_dir: PathBuf::from(DEFAULT_MODEL_DIR),
            semantic_batch_size: DEFAULT_SEMANTIC_BATCH_SIZE,
            semantic_top_k: DEFAULT_SEMANTIC_TOP_K,
            semantic_score_threshold: DEFAULT_SEMANTIC_SCORE_THRESHOLD,
            semantic_title_column: None,
            semantic_runtime_preference: SemanticRuntimePreference::Off,
        }
    }
}
