use super::ExecutionStrategy;
use crate::{inputs::select_embedding_value, ranking::l2_normalize};
use anyhow::{anyhow, Context, Result};
use ort::{
    execution_providers::directml::DirectMLExecutionProvider,
    session::{builder::GraphOptimizationLevel, Session},
    value::DynValue,
};
use parking_lot::Mutex;
use std::path::Path;
use tracing::error;

const GRANITE_EMBEDDING_DIM: usize = 384;

pub(crate) fn build_session(model_path: &Path, strategy: ExecutionStrategy) -> Result<Session> {
    let opt_level = match strategy {
        // DirectML Level3/1 have produced NaNs on some drivers; disable graph opts for stability.
        ExecutionStrategy::DirectMl { .. } => GraphOptimizationLevel::Disable,
        ExecutionStrategy::Cpu => GraphOptimizationLevel::Level3,
    };
    let builder = Session::builder()?
        .with_optimization_level(opt_level)?
        .with_intra_threads(num_cpus::get())?;
    let builder = match strategy {
        ExecutionStrategy::Cpu => builder,
        ExecutionStrategy::DirectMl { fail_on_error } => {
            // DirectML is the only GPU backend we support, so surface failures loudly
            // when the user explicitly asked for hardware acceleration. DirectML requires
            // sequential execution and does not support ONNX Runtime memory patterns.
            let dispatch = DirectMLExecutionProvider::default().build();
            let dispatch = if fail_on_error {
                dispatch.error_on_failure()
            } else {
                dispatch
            };
            builder
                .with_parallel_execution(false)?
                .with_memory_pattern(false)?
                .with_execution_providers([dispatch])?
        }
    };
    builder
        .commit_from_file(model_path)
        .with_context(|| format!("failed to load ONNX model at {}", model_path.display()))
}

pub(crate) fn run_with_session(
    session: &Mutex<Session>,
    feeds: Vec<(&str, ort::value::Tensor<i64>)>,
    preferred_output: Option<&str>,
) -> Result<DynValue> {
    let embedding_value = {
        let mut session = session.lock();
        let outputs = session.run(feeds)?;
        select_embedding_value(outputs, preferred_output)?
    };
    Ok(embedding_value)
}

pub(crate) fn extract_embeddings(
    embedding_value: &DynValue,
    backend: &str,
    expected_batch: usize,
) -> Result<Vec<Vec<f32>>> {
    let (shape, buffer) = embedding_value
        .try_extract_tensor::<f32>()
        .map_err(|err| anyhow!("failed to decode embedding tensor: {err}"))?;
    extract_embedding_buffer(shape.as_ref(), buffer, backend, expected_batch)
}

pub(crate) fn extract_embedding_buffer(
    dims: &[i64],
    buffer: &[f32],
    backend: &str,
    expected_batch: usize,
) -> Result<Vec<Vec<f32>>> {
    let (batch, sequence_len, dim) = validate_embedding_shape(dims, buffer, expected_batch)?;
    let invalid_values = buffer.iter().filter(|value| !value.is_finite()).count();
    if invalid_values > 0 {
        let mut finite_min = f32::INFINITY;
        let mut finite_max = f32::NEG_INFINITY;
        let mut finite_values = 0_usize;
        for value in buffer.iter().copied() {
            if value.is_finite() {
                finite_min = finite_min.min(value);
                finite_max = finite_max.max(value);
                finite_values += 1;
            }
        }
        let finite_min = if finite_values > 0 { finite_min } else { 0.0 };
        let finite_max = if finite_values > 0 { finite_max } else { 0.0 };
        error!(
            invalid_values,
            total_values = buffer.len(),
            finite_values,
            dim,
            finite_min,
            finite_max,
            backend,
            "semantic model returned non-finite values"
        );
        return Err(NonFiniteValuesError {
            invalid_values,
            total_values: buffer.len(),
            dim,
            finite_min,
            finite_max,
            backend: backend.to_string(),
        }
        .into());
    }

    let row_stride = sequence_len
        .checked_mul(dim)
        .ok_or_else(|| anyhow!("Granite output row stride exceeds usize::MAX"))?;
    let mut vectors = Vec::with_capacity(batch);
    for batch_idx in 0..batch {
        let start = batch_idx
            .checked_mul(row_stride)
            .ok_or_else(|| anyhow!("Granite output offset exceeds usize::MAX"))?;
        let end = start + dim;
        let mut row = buffer[start..end].to_vec();
        l2_normalize(&mut row);
        vectors.push(row);
    }
    Ok(vectors)
}

fn validate_embedding_shape(
    dims: &[i64],
    buffer: &[f32],
    expected_batch: usize,
) -> Result<(usize, usize, usize)> {
    let (batch, sequence_len, dim) = match dims {
        [batch, dim] => (
            positive_dim(*batch, "batch")?,
            1,
            positive_dim(*dim, "embedding")?,
        ),
        [batch, sequence_len, dim] => (
            positive_dim(*batch, "batch")?,
            positive_dim(*sequence_len, "sequence")?,
            positive_dim(*dim, "embedding")?,
        ),
        _ => {
            return Err(anyhow!(
                "expected Granite output shape [batch, 384] or [batch, sequence, 384], got {dims:?}"
            ));
        }
    };
    if batch != expected_batch {
        return Err(anyhow!(
            "Granite output batch {batch} does not match input batch {expected_batch}"
        ));
    }
    if dim != GRANITE_EMBEDDING_DIM {
        return Err(anyhow!(
            "Granite output dimension {dim} does not match expected {GRANITE_EMBEDDING_DIM}"
        ));
    }
    let expected_len = batch
        .checked_mul(sequence_len)
        .and_then(|value| value.checked_mul(dim))
        .ok_or_else(|| anyhow!("Granite output shape exceeds usize::MAX"))?;
    if buffer.len() != expected_len {
        return Err(anyhow!(
            "Granite output buffer length {} does not match shape {dims:?} ({expected_len} values)",
            buffer.len()
        ));
    }
    Ok((batch, sequence_len, dim))
}

fn positive_dim(value: i64, label: &str) -> Result<usize> {
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow!("Granite output {label} dimension must be positive, got {value}"))
}

#[cfg(test)]
mod tests {
    use super::extract_embedding_buffer;

    #[test]
    fn cls_pooling_selects_first_token_per_batch_and_normalizes() {
        let mut buffer = Vec::new();
        for batch in 0..2 {
            let mut cls = vec![0.0_f32; 384];
            cls[batch] = 3.0;
            cls[batch + 2] = 4.0;
            buffer.extend(cls);
            buffer.extend(vec![99.0_f32; 384]);
        }

        let vectors = extract_embedding_buffer(&[2, 2, 384], &buffer, "test", 2).unwrap();
        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), 384);
        assert!((vectors[0][0] - 0.6).abs() < 1e-6);
        assert!((vectors[0][2] - 0.8).abs() < 1e-6);
        assert!(vectors[0][1].abs() < f32::EPSILON);
        assert!((vectors[1][1] - 0.6).abs() < 1e-6);
        assert!((vectors[1][3] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn rejects_rank_shape_batch_and_dimension_mismatches() {
        let rank_error = extract_embedding_buffer(&[384], &[0.0; 384], "test", 1)
            .unwrap_err()
            .to_string();
        assert!(rank_error.contains("expected Granite output shape"));

        let batch_error = extract_embedding_buffer(&[2, 384], &[0.0; 768], "test", 1)
            .unwrap_err()
            .to_string();
        assert!(batch_error.contains("does not match input batch"));

        let dim_error = extract_embedding_buffer(&[1, 768], &[0.0; 768], "test", 1)
            .unwrap_err()
            .to_string();
        assert!(dim_error.contains("does not match expected 384"));
    }

    #[test]
    fn rejects_non_finite_values_before_pooling() {
        let mut buffer = vec![0.0_f32; 2 * 384];
        buffer[384] = f32::NAN;
        let error = extract_embedding_buffer(&[1, 2, 384], &buffer, "test", 1).unwrap_err();
        assert!(error.to_string().contains("non-finite values"));
    }
}

#[derive(Debug)]
pub(crate) struct NonFiniteValuesError {
    pub(crate) invalid_values: usize,
    pub(crate) total_values: usize,
    pub(crate) dim: usize,
    pub(crate) finite_min: f32,
    pub(crate) finite_max: f32,
    pub(crate) backend: String,
}

impl std::fmt::Display for NonFiniteValuesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "semantic model returned non-finite values (backend={}, {} of {}, dim={}, finite_min={}, finite_max={})",
            self.backend,
            self.invalid_values,
            self.total_values,
            self.dim,
            self.finite_min,
            self.finite_max
        )
    }
}

impl std::error::Error for NonFiniteValuesError {}
