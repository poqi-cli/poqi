use std::cmp::Ordering;

use anyhow::{anyhow, Result};

use crate::embedder::SemanticEmbedder;

pub type ProgressCallback<'a> = &'a mut dyn FnMut(usize, usize, &str);

/// Options that control how semantic ranking behaves.
#[derive(Debug, Clone, Copy)]
pub struct SearchOptions {
    pub batch_size: usize,
    pub top_k: usize,
    pub threshold: Option<f32>,
    pub dim: usize,
}

impl SearchOptions {
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            batch_size: self.batch_size.max(1),
            top_k: self.top_k.max(1),
            threshold: self.threshold.map(|t| t.clamp(-1.0, 1.0)),
            dim: self.dim.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RankedRow {
    pub index: usize,
    pub score: f32,
}

/// Execute semantic ranking by embedding both the query and documents.
///
/// # Errors
/// Returns an error when embedding fails or no embeddings are produced.
pub fn embed_and_rank(
    embedder: &SemanticEmbedder,
    query_prompt: &str,
    docs: &[String],
    options: SearchOptions,
    mut progress: Option<ProgressCallback<'_>>,
) -> Result<Vec<RankedRow>> {
    if docs.is_empty() {
        return Ok(Vec::new());
    }

    let opts = options.sanitized();
    let mut query_embedding = embedder
        .encode(&[query_prompt.to_string()])?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("model did not return a query embedding"))?;
    l2_normalize(&mut query_embedding);

    let mut scored = Vec::with_capacity(docs.len());
    let mut base_index = 0;
    let total_chunks = docs.chunks(opts.batch_size).len().max(1);
    if let Some(cb) = progress.as_deref_mut() {
        cb(0, total_chunks, embedder.backend_label());
    }
    for (chunk_idx, chunk) in docs.chunks(opts.batch_size).enumerate() {
        let embeddings = embedder.encode(chunk)?;
        if embeddings.len() != chunk.len() {
            return Err(anyhow!(
                "embedding count {} did not match chunk size {}",
                embeddings.len(),
                chunk.len()
            ));
        }
        for (offset, embedding) in embeddings.into_iter().enumerate() {
            let index = base_index + offset;
            let score = cosine_similarity(&query_embedding, &embedding);
            scored.push(RankedRow { index, score });
        }
        base_index += chunk.len();
        if let Some(cb) = progress.as_deref_mut() {
            cb(chunk_idx + 1, total_chunks, embedder.backend_label());
        }
    }

    Ok(finalize_ranking(scored, opts))
}

/// Rank pre-embedded documents against a fresh query embedding.
///
/// # Errors
/// Returns an error when embedding the query fails or no embeddings are produced.
pub fn rank_with_embeddings(
    embedder: &SemanticEmbedder,
    query_prompt: &str,
    doc_embeddings: &[Vec<f32>],
    options: SearchOptions,
) -> Result<Vec<RankedRow>> {
    if doc_embeddings.is_empty() {
        return Ok(Vec::new());
    }

    let opts = options.sanitized();
    let mut query_embedding = embedder
        .encode(&[query_prompt.to_string()])?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("model did not return a query embedding"))?;
    l2_normalize(&mut query_embedding);

    let mut scored = Vec::with_capacity(doc_embeddings.len());
    for (index, embedding) in doc_embeddings.iter().enumerate() {
        let score = cosine_similarity(&query_embedding, embedding);
        scored.push(RankedRow { index, score });
    }

    Ok(finalize_ranking(scored, opts))
}

pub(crate) fn finalize_ranking(mut rows: Vec<RankedRow>, options: SearchOptions) -> Vec<RankedRow> {
    if rows.is_empty() {
        return rows;
    }

    rows.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

    if let Some(threshold) = options.threshold {
        return rows
            .into_iter()
            .filter(|row| row.score >= threshold)
            .take(options.top_k)
            .collect();
    }

    rows.into_iter().take(options.top_k).collect()
}

/// L2-normalize a vector in-place.
pub fn l2_normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
}

/// Cosine similarity between two equal-length vectors.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum::<f32>()
}
