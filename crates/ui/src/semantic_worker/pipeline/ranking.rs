use std::sync::atomic::{AtomicBool, Ordering};

use poqi_engine::QueryResult;
use poqi_search_semantic::{
    cosine_similarity, format_query_prompt, l2_normalize, ProgressCallback, RankedRow,
    SearchOptions,
};
use tracing::error;

use super::{
    cache::{EmbeddingCache, EmbeddingComputationError},
    dataset::{build_dataset_id, row_keys},
    documents::build_documents,
    EmbeddingProvider,
};

/// Outcome summary describing how many rows survived ranking and the top score.
#[derive(Debug, Clone)]
pub(crate) struct RankingOutcome {
    pub(crate) kept_rows: usize,
    pub(crate) top_score: f32,
    pub(crate) fallback_notice: Option<String>,
    pub(crate) backend_label: String,
    pub(crate) cache_hits: usize,
    pub(crate) cache_misses: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RankingError {
    Canceled,
    Failed(String),
}

/// Run semantic ranking over a result set and return the reduced table plus stats.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rank_rows(
    embedder: &dyn EmbeddingProvider,
    query: &str,
    mut result: QueryResult,
    title_column: Option<&str>,
    options: SearchOptions,
    cache: &mut EmbeddingCache,
    progress: Option<ProgressCallback<'_>>,
    cancel: &AtomicBool,
) -> Result<(QueryResult, RankingOutcome), RankingError> {
    let options = options.sanitized();
    let docs = build_documents(&result, title_column);
    let row_keys = row_keys(&result);
    if docs.is_empty() {
        result.rows.clear();
        result.metadata.row_identities.clear();
        result.metadata.null_cells.clear();
        return Ok((
            result,
            RankingOutcome {
                kept_rows: 0,
                top_score: 0.0,
                fallback_notice: None,
                backend_label: embedder.backend_label().to_string(),
                cache_hits: 0,
                cache_misses: 0,
            },
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(RankingError::Canceled);
    }
    let dataset_id = build_dataset_id(&result, embedder.model_path(), title_column);
    let (doc_embeddings, cache_hits) = match cache.embeddings_for(
        &dataset_id,
        &row_keys,
        &docs,
        embedder,
        options,
        progress,
        cancel,
    ) {
        Ok(embeddings) => embeddings,
        Err(EmbeddingComputationError::Canceled) => return Err(RankingError::Canceled),
        Err(EmbeddingComputationError::Failed(err)) => {
            error!(error = %err, "semantic ranking failed");
            return Err(RankingError::Failed(err.to_string()));
        }
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(RankingError::Canceled);
    }
    let mut query_embedding = embedder
        .encode(&[format_query_prompt(query)])
        .map_err(|err| {
            error!(error = %err, "semantic ranking failed");
            RankingError::Failed(err.to_string())
        })?
        .into_iter()
        .next()
        .ok_or_else(|| {
            RankingError::Failed("model did not return a query embedding".to_string())
        })?;
    l2_normalize(&mut query_embedding);
    if cancel.load(Ordering::Relaxed) {
        return Err(RankingError::Canceled);
    }
    let mut ranking = Vec::with_capacity(doc_embeddings.len());
    for (index, embedding) in doc_embeddings.iter().enumerate() {
        let score = cosine_similarity(&query_embedding, embedding);
        ranking.push(RankedRow { index, score });
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(RankingError::Canceled);
    }
    let ranking = finalize_local_ranking(ranking, options);
    if cancel.load(Ordering::Relaxed) {
        return Err(RankingError::Canceled);
    }
    let fallback_notice = embedder.take_fallback_note();
    let backend_label = embedder.backend_label().to_string();
    let subset = subset_result(result, &ranking);
    let outcome = RankingOutcome {
        kept_rows: subset.rows.len(),
        top_score: ranking.first().map_or(0.0, |row| row.score),
        fallback_notice,
        backend_label,
        cache_hits: cache_hits.hits,
        cache_misses: cache_hits.misses,
    };
    Ok((subset, outcome))
}

fn finalize_local_ranking(mut rows: Vec<RankedRow>, options: SearchOptions) -> Vec<RankedRow> {
    if rows.is_empty() {
        return rows;
    }

    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if let Some(threshold) = options.threshold {
        return rows
            .into_iter()
            .filter(|row| row.score >= threshold)
            .take(options.top_k)
            .collect();
    }

    rows.into_iter().take(options.top_k).collect()
}

/// Reorder the result set so only ranked rows remain.
fn subset_result(mut result: QueryResult, ranking: &[RankedRow]) -> QueryResult {
    if ranking.is_empty() {
        result.rows.clear();
        result.metadata.row_identities.clear();
        result.metadata.null_cells.clear();
        return result;
    }

    let mut new_rows = Vec::with_capacity(ranking.len());
    let mut new_identities = Vec::with_capacity(ranking.len());
    let has_null_metadata = !result.metadata.null_cells.is_empty();
    let mut new_null_cells = Vec::with_capacity(ranking.len());
    for ranked in ranking {
        if let Some(row) = result.rows.get(ranked.index) {
            new_rows.push(row.clone());
            let identity = result
                .metadata
                .row_identities
                .get(ranked.index)
                .cloned()
                .unwrap_or(None);
            new_identities.push(identity);
            if has_null_metadata {
                let nulls = result
                    .metadata
                    .null_cells
                    .get(ranked.index)
                    .cloned()
                    .unwrap_or_else(|| vec![false; result.columns.len()]);
                new_null_cells.push(nulls);
            }
        }
    }
    result.rows = new_rows;
    result.metadata.row_identities = new_identities;
    result.metadata.null_cells = new_null_cells;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use crate::semantic_worker::pipeline::cache::EmbeddingCache;
    use poqi_catalog::QualifiedRelation;
    use poqi_engine::{QueryResult, ResultMetadata, ResultOrigin, RowIdentity};
    use poqi_search_semantic::SearchOptions;

    #[derive(Debug)]
    struct MockEmbedder {
        calls: AtomicUsize,
        model_path: &'static str,
    }

    #[derive(Debug)]
    struct NoMatchEmbedder;

    impl EmbeddingProvider for NoMatchEmbedder {
        fn encode(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|text| {
                    if text == "unrelated query" {
                        vec![1.0, 0.0]
                    } else {
                        vec![0.0, 1.0]
                    }
                })
                .collect())
        }

        fn backend_label(&self) -> &'static str {
            "mock"
        }

        fn take_fallback_note(&self) -> Option<String> {
            None
        }

        fn model_path(&self) -> &Path {
            Path::new("/tmp/no-match-model")
        }
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
            rows: vec![vec!["1".into()], vec!["2".into()]],
            metadata: ResultMetadata {
                source_table: Some(QualifiedRelation::in_schema("public", "demo")),
                row_identities: vec![
                    Some(RowIdentity {
                        relation_oid: 1,
                        xmin: "1".into(),
                        primary_key: Vec::new(),
                        table_oid: 1,
                        ctid: "(0,1)".into(),
                    }),
                    Some(RowIdentity {
                        relation_oid: 1,
                        xmin: "1".into(),
                        primary_key: Vec::new(),
                        table_oid: 1,
                        ctid: "(0,2)".into(),
                    }),
                ],
                source_columns: vec![Some("id".into())],
                column_types: Vec::new(),
                null_cells: vec![vec![true], vec![false]],
                origin: ResultOrigin::SelectTop {
                    table: QualifiedRelation::in_schema("public", "demo"),
                    limit: 2,
                },
            },
        }
    }

    #[test]
    fn rank_rows_aborts_when_cancelled_up_front() {
        let embedder = MockEmbedder::new();
        let mut cache = EmbeddingCache::default();
        let options = SearchOptions {
            batch_size: 2,
            top_k: 1,
            threshold: None,
            dim: 4,
        };
        let cancel = AtomicBool::new(true);
        let result = sample_result();
        let outcome = rank_rows(
            &embedder, "anything", result, None, options, &mut cache, None, &cancel,
        );

        assert!(matches!(outcome, Err(RankingError::Canceled)));
        assert_eq!(embedder.calls(), 0);
    }

    #[test]
    fn rank_rows_returns_empty_result_when_no_row_meets_threshold() {
        let embedder = NoMatchEmbedder;
        let mut cache = EmbeddingCache::default();
        let options = SearchOptions {
            batch_size: 2,
            top_k: 2,
            threshold: Some(0.8),
            dim: 2,
        };
        let cancel = AtomicBool::new(false);
        let (result, outcome) = rank_rows(
            &embedder,
            "unrelated query",
            sample_result(),
            None,
            options,
            &mut cache,
            None,
            &cancel,
        )
        .unwrap();

        assert!(result.rows.is_empty());
        assert!(result.metadata.row_identities.is_empty());
        assert!(result.metadata.null_cells.is_empty());
        assert_eq!(outcome.kept_rows, 0);
        assert!(outcome.top_score.abs() < f32::EPSILON);
    }

    #[test]
    fn subset_result_reorders_null_metadata_with_ranked_rows() {
        let result = subset_result(
            sample_result(),
            &[
                RankedRow {
                    index: 1,
                    score: 0.9,
                },
                RankedRow {
                    index: 0,
                    score: 0.8,
                },
            ],
        );

        assert_eq!(
            result.rows,
            vec![vec!["2".to_string()], vec!["1".to_string()]]
        );
        assert_eq!(result.metadata.null_cells, vec![vec![false], vec![true]]);
        assert_eq!(
            result.metadata.row_identities[0]
                .as_ref()
                .map(|identity| identity.ctid.as_str()),
            Some("(0,2)")
        );
    }
}
