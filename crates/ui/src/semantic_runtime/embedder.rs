use crate::{
    runtime::{self, RuntimeBackend, RuntimeSelection},
    semantic_bootstrap,
};
use anyhow::{anyhow, Context, Result};
use poqi_config::{AppConfig, SemanticRuntimePreference};
use poqi_search_semantic::{ExecutionStrategy, SemanticEmbedder};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::{info, warn};

const ERROR_SUMMARY_MAX_LEN: usize = 180;

pub(crate) async fn load_embedder_from_root(
    config: &AppConfig,
    preference: SemanticRuntimePreference,
    cached_runtime: Option<RuntimeSelection>,
    root: &Path,
) -> Result<(Arc<SemanticEmbedder>, RuntimeBackend, Option<String>)> {
    let selection = if let Some(selection) = cached_runtime {
        selection
    } else {
        runtime::ensure_runtime_lib_with_preference(config, preference, None).await?
    };
    let (model_path, tokenizer_path) = ensure_model_paths(root, selection.backend)?;
    instantiate_with_optional_fallback(config, preference, selection, &model_path, &tokenizer_path)
        .await
}

fn ensure_model_paths(root: &Path, backend: RuntimeBackend) -> Result<(PathBuf, PathBuf)> {
    let model_filename = match backend {
        RuntimeBackend::Cpu => semantic_bootstrap::CPU_MODEL_FILE,
        RuntimeBackend::GpuDirectMl => semantic_bootstrap::GPU_MODEL_FILE,
    };
    let model_path = root.join(model_filename);
    if !model_path.exists() {
        return Err(anyhow!(
            "semantic model not found at {}",
            model_path.display()
        ));
    }
    let tokenizer_path = root.join(semantic_bootstrap::TOKENIZER_FILE);
    if !tokenizer_path.exists() {
        return Err(anyhow!(
            "tokenizer file not found at {}",
            tokenizer_path.display()
        ));
    }
    Ok((model_path, tokenizer_path))
}

fn should_consider_gpu_fallback(
    preference: SemanticRuntimePreference,
    backend: RuntimeBackend,
) -> bool {
    preference == SemanticRuntimePreference::Auto && matches!(backend, RuntimeBackend::GpuDirectMl)
}

fn instantiate_embedder(
    selection: &RuntimeSelection,
    model_path: &Path,
    tokenizer_path: &Path,
) -> Result<SemanticEmbedder> {
    runtime::configure_runtime_env(&selection.lib_path);
    info!(
        backend = selection.backend.label(),
        runtime = %selection.lib_path.display(),
        model = %model_path.display(),
        tokenizer = %tokenizer_path.display(),
        "Building semantic embedder"
    );
    let strategy = match selection.backend {
        RuntimeBackend::Cpu => ExecutionStrategy::Cpu,
        RuntimeBackend::GpuDirectMl => ExecutionStrategy::DirectMl {
            fail_on_error: true,
        },
    };
    SemanticEmbedder::from_files(model_path, tokenizer_path, strategy)
}

async fn instantiate_with_optional_fallback(
    config: &AppConfig,
    preference: SemanticRuntimePreference,
    selection: RuntimeSelection,
    model_path: &Path,
    tokenizer_path: &Path,
) -> Result<(Arc<SemanticEmbedder>, RuntimeBackend, Option<String>)> {
    match instantiate_embedder(&selection, model_path, tokenizer_path) {
        Ok(embedder) => {
            info!(
                backend = selection.backend.label(),
                "Semantic embedder ready"
            );
            Ok((Arc::new(embedder), selection.backend, None))
        }
        Err(err) if should_consider_gpu_fallback(preference, selection.backend) => {
            let summary = summarize_error(&err);
            warn!(
                backend = selection.backend.label(),
                summary = %summary,
                "DirectML semantic runtime failed"
            );
            fallback_to_cpu(config, model_path, tokenizer_path, err).await
        }
        Err(err) => Err(err),
    }
}

async fn fallback_to_cpu(
    config: &AppConfig,
    model_path: &Path,
    tokenizer_path: &Path,
    previous_error: anyhow::Error,
) -> Result<(Arc<SemanticEmbedder>, RuntimeBackend, Option<String>)> {
    let summary = summarize_error(&previous_error);
    warn!(summary = %summary, "Falling back to CPU semantic runtime");
    let cpu_selection =
        runtime::ensure_runtime_lib_with_preference(config, SemanticRuntimePreference::Cpu, None)
            .await?;
    let embedder = instantiate_embedder(&cpu_selection, model_path, tokenizer_path)
        .with_context(|| format!("CPU fallback failed after GPU error: {previous_error}"))?;
    info!(
        summary = %summary,
        backend = cpu_selection.backend.label(),
        "DirectML failed; using CPU for semantic search"
    );
    Ok((Arc::new(embedder), cpu_selection.backend, Some(summary)))
}

fn summarize_error(err: &anyhow::Error) -> String {
    let mut text = err.to_string();
    if let Some((first_line, _)) = text.split_once('\n') {
        text = first_line.to_string();
    }
    if text.chars().count() > ERROR_SUMMARY_MAX_LEN {
        let mut truncated = text.chars().take(ERROR_SUMMARY_MAX_LEN).collect::<String>();
        truncated.push('.');
        truncated
    } else {
        text
    }
}
