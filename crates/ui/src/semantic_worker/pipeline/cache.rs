use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Error;
use poqi_search_semantic::{ProgressCallback, SearchOptions};

use super::{
    dataset::{DatasetId, RowKey},
    EmbeddingProvider,
};

#[derive(Debug)]
struct CacheEntry {
    id: DatasetId,
    embeddings: HashMap<RowKey, Vec<f32>>,
}

impl CacheEntry {
    fn new(id: DatasetId) -> Self {
        Self {
            id,
            embeddings: HashMap::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct CacheHitStats {
    pub(super) hits: usize,
    pub(super) misses: usize,
}

#[derive(Debug, Default)]
pub(crate) struct EmbeddingCache {
    entry: Option<CacheEntry>,
}

#[derive(Debug)]
pub(crate) enum EmbeddingComputationError {
    Canceled,
    Failed(Error),
}

impl From<Error> for EmbeddingComputationError {
    fn from(value: Error) -> Self {
        Self::Failed(value)
    }
}

impl EmbeddingCache {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn embeddings_for(
        &mut self,
        id: &DatasetId,
        row_keys: &[RowKey],
        docs: &[String],
        embedder: &dyn EmbeddingProvider,
        options: SearchOptions,
        mut progress: Option<ProgressCallback<'_>>,
        cancel: &AtomicBool,
    ) -> Result<(Vec<Vec<f32>>, CacheHitStats), EmbeddingComputationError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(EmbeddingComputationError::Canceled);
        }

        if docs.len() != row_keys.len() {
            return Err(EmbeddingComputationError::Failed(anyhow::anyhow!(
                "document count {} did not match row keys {}",
                docs.len(),
                row_keys.len()
            )));
        }

        let mut entry = match &self.entry {
            Some(existing) if existing.id == *id => self.entry.take().unwrap(),
            _ => CacheEntry::new(id.clone()),
        };
        let current_keys = row_keys.iter().collect::<HashSet<_>>();
        entry.embeddings.retain(|key, _| current_keys.contains(key));
        let mut embeddings: Vec<Option<Vec<f32>>> = vec![None; docs.len()];
        let mut hits = 0_usize;
        let mut missing = Vec::new();

        for (idx, key) in row_keys.iter().enumerate() {
            if let Some(vector) = entry.embeddings.get(key) {
                embeddings[idx] = Some(vector.clone());
                hits += 1;
            } else {
                missing.push((idx, key.clone(), docs[idx].clone()));
            }
        }

        let misses = missing.len();
        if misses > 0 {
            let batch_size = options.batch_size;
            let total_chunks = (misses.saturating_sub(1)) / batch_size + 1;
            if let Some(cb) = progress.as_deref_mut() {
                if cancel.load(Ordering::Relaxed) {
                    self.entry = Some(entry);
                    return Err(EmbeddingComputationError::Canceled);
                }
                cb(0, total_chunks, embedder.backend_label());
            }
            for (chunk_idx, chunk) in missing.chunks(batch_size).enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    self.entry = Some(entry);
                    return Err(EmbeddingComputationError::Canceled);
                }
                let mut batch_indices = Vec::with_capacity(chunk.len());
                let mut batch_keys = Vec::with_capacity(chunk.len());
                let mut batch_docs = Vec::with_capacity(chunk.len());
                for (idx, key, doc) in chunk {
                    batch_indices.push(*idx);
                    batch_keys.push(key.clone());
                    batch_docs.push(doc.clone());
                }
                let batch_embeddings = embedder.encode(&batch_docs)?;
                if batch_embeddings.len() != batch_docs.len() {
                    return Err(EmbeddingComputationError::Failed(anyhow::anyhow!(
                        "embedding count {} did not match batch size {}",
                        batch_embeddings.len(),
                        batch_docs.len()
                    )));
                }
                for ((target_idx, key), embedding) in batch_indices
                    .into_iter()
                    .zip(batch_keys)
                    .zip(batch_embeddings)
                {
                    entry.embeddings.insert(key.clone(), embedding.clone());
                    embeddings[target_idx] = Some(embedding);
                }
                if let Some(cb) = progress.as_deref_mut() {
                    if cancel.load(Ordering::Relaxed) {
                        self.entry = Some(entry);
                        return Err(EmbeddingComputationError::Canceled);
                    }
                    cb(chunk_idx + 1, total_chunks, embedder.backend_label());
                }
            }
        }

        let mut finalized = Vec::with_capacity(embeddings.len());
        for (idx, slot) in embeddings.into_iter().enumerate() {
            let Some(vector) = slot else {
                return Err(EmbeddingComputationError::Failed(anyhow::anyhow!(
                    "missing embedding for row {idx} after caching step"
                )));
            };
            finalized.push(vector);
        }
        self.entry = Some(entry);
        Ok((finalized, CacheHitStats { hits, misses }))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::super::dataset::{build_dataset_id, row_keys};
    use super::*;
    use poqi_catalog::QualifiedRelation;
    use poqi_engine::{QueryResult, ResultMetadata, ResultOrigin, RowIdentity};
    use poqi_search_semantic::SearchOptions;

    #[derive(Debug)]
    struct MockEmbedder {
        calls: AtomicUsize,
        model_path: &'static str,
    }

    impl MockEmbedder {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                model_path: "/tmp/mock-model",
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl EmbeddingProvider for MockEmbedder {
        #[allow(clippy::cast_precision_loss)]
        fn encode(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(texts.iter().map(|text| vec![text.len() as f32]).collect())
        }

        fn backend_label(&self) -> &'static str {
            "mock"
        }

        fn take_fallback_note(&self) -> Option<String> {
            None
        }

        fn model_path(&self) -> &Path {
            Path::new(self.model_path)
        }
    }

    fn sample_result() -> QueryResult {
        QueryResult {
            columns: vec!["id".into()],
            rows: vec![
                vec!["1".into()],
                vec!["2".into()],
                vec!["3".into()],
                vec!["4".into()],
                vec!["5".into()],
            ],
            metadata: ResultMetadata {
                source_table: Some(QualifiedRelation::in_schema("public", "demo")),
                row_identities: (1..=5)
                    .map(|n| {
                        Some(RowIdentity {
                            relation_oid: 1,
                            xmin: "1".into(),
                            primary_key: Vec::new(),
                            table_oid: 1,
                            ctid: format!("(0,{n})"),
                        })
                    })
                    .collect(),
                source_columns: vec![Some("id".to_string())],
                column_types: Vec::new(),
                null_cells: vec![vec![false]; 5],
                origin: ResultOrigin::SelectTop {
                    table: QualifiedRelation::in_schema("public", "demo"),
                    limit: 5,
                },
            },
        }
    }

    #[test]
    fn cancellation_short_circuits_and_preserves_cache() {
        let mut cache = EmbeddingCache::default();
        let embedder = MockEmbedder::new();
        let options = SearchOptions {
            batch_size: 2,
            top_k: 3,
            threshold: None,
            dim: 4,
        };
        let result = sample_result();
        let docs = vec!["a", "bb", "ccc", "dddd", "eeeee"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let keys = row_keys(&result);
        let dataset_id = build_dataset_id(&result, Path::new(embedder.model_path), None);
        let cancel = AtomicBool::new(false);
        let mut progress = |done: usize, _total: usize, _backend: &str| {
            if done == 1 {
                cancel.store(true, Ordering::Relaxed);
            }
        };

        let first = cache.embeddings_for(
            &dataset_id,
            &keys,
            &docs,
            &embedder,
            options,
            Some(&mut progress),
            &cancel,
        );
        assert!(matches!(first, Err(EmbeddingComputationError::Canceled)));
        assert_eq!(embedder.calls(), 1);

        cancel.store(false, Ordering::Relaxed);
        let mut noop_progress = |_done: usize, _total: usize, _backend: &str| {};
        let second = cache
            .embeddings_for(
                &dataset_id,
                &keys,
                &docs,
                &embedder,
                options,
                Some(&mut noop_progress),
                &cancel,
            )
            .expect("second run should complete");
        assert_eq!(second.0.len(), docs.len());
        assert_eq!(embedder.calls(), 3);
    }

    #[test]
    fn changed_row_reembeds_only_changed_content_despite_reused_ctid() {
        let mut cache = EmbeddingCache::default();
        let embedder = MockEmbedder::new();
        let options = SearchOptions {
            batch_size: 8,
            top_k: 3,
            threshold: None,
            dim: 768,
        };
        let mut result = sample_result();
        let docs = result
            .rows
            .iter()
            .map(|row| row.join(" "))
            .collect::<Vec<_>>();
        let keys = row_keys(&result);
        let id = build_dataset_id(&result, Path::new(embedder.model_path), None);
        let cancel = AtomicBool::new(false);
        let (_, cold) = cache
            .embeddings_for(&id, &keys, &docs, &embedder, options, None, &cancel)
            .expect("cold cache fill");
        assert_eq!((cold.hits, cold.misses), (0, 5));

        result.rows[0][0] = "replacement".to_string();
        let changed_docs = result
            .rows
            .iter()
            .map(|row| row.join(" "))
            .collect::<Vec<_>>();
        let changed_keys = row_keys(&result);
        let changed_id = build_dataset_id(&result, Path::new(embedder.model_path), None);
        assert_eq!(id, changed_id);
        let (_, warm) = cache
            .embeddings_for(
                &changed_id,
                &changed_keys,
                &changed_docs,
                &embedder,
                options,
                None,
                &cancel,
            )
            .expect("incremental cache refresh");
        assert_eq!((warm.hits, warm.misses), (4, 1));
        assert_eq!(
            cache.entry.as_ref().map(|entry| entry.embeddings.len()),
            Some(5)
        );

        let future_dim_options = SearchOptions {
            dim: 256,
            ..options
        };
        let (_, unchanged) = cache
            .embeddings_for(
                &changed_id,
                &changed_keys,
                &changed_docs,
                &embedder,
                future_dim_options,
                None,
                &cancel,
            )
            .expect("future dimension must not invalidate embeddings");
        assert_eq!((unchanged.hits, unchanged.misses), (5, 0));
    }
}
