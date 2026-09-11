mod pipeline;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use poqi_engine::QueryResult;
use poqi_search_semantic::SearchOptions;
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use self::pipeline::{rank_rows, EmbeddingCache, RankingError, SharedEmbedder};

/// Messages processed by the semantic worker task.
#[derive(Debug)]
pub(super) enum SemanticCommand {
    RankRows {
        request_id: u64,
        query: String,
        rows: QueryResult,
        title_column: Option<String>,
        options: SearchOptions,
        cancel: Arc<AtomicBool>,
    },
}

/// Replies produced by the semantic worker.
#[derive(Debug)]
pub(super) enum SemanticResponse {
    Success {
        request_id: u64,
        result: Box<QueryResult>,
        stats: SemanticStats,
    },
    Progress {
        request_id: u64,
        message: String,
    },
    Canceled {
        request_id: u64,
    },
    Error {
        request_id: u64,
        message: String,
    },
}

/// Lightweight stats reported alongside each ranking run.
#[derive(Debug, Clone)]
pub(super) struct SemanticStats {
    pub matched_rows: usize,
    pub top_score: f32,
    pub fallback_notice: Option<String>,
    pub backend_label: String,
    pub elapsed_ms: u128,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Spawn a background task that receives rows, ranks them, and streams results back.
pub(super) fn spawn_semantic_worker(
    embedder: Option<SharedEmbedder>,
    unavailable_reason: Option<String>,
    mut commands: UnboundedReceiver<SemanticCommand>,
    responses: UnboundedSender<SemanticResponse>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut cache = EmbeddingCache::default();
        while let Some(command) = commands.recv().await {
            let SemanticCommand::RankRows { request_id, .. } = &command;
            let request_id = *request_id;
            let Some(embedder) = embedder.clone() else {
                let message = unavailable_reason
                    .clone()
                    .unwrap_or_else(|| "semantic model unavailable".to_string());
                let _ = responses.send(SemanticResponse::Error {
                    request_id,
                    message,
                });
                continue;
            };
            let progress_tx = responses.clone();
            let job = RankingJob {
                command,
                embedder,
                cache,
                progress_tx,
            };
            match tokio::task::spawn_blocking(move || run_ranking_job(job)).await {
                Ok((returned_cache, response)) => {
                    cache = returned_cache;
                    let _ = responses.send(response);
                }
                Err(err) => {
                    cache = EmbeddingCache::default();
                    let message = if err.is_panic() {
                        "Semantic worker panicked while ranking rows".to_string()
                    } else {
                        format!("Semantic worker stopped unexpectedly: {err}")
                    };
                    let _ = responses.send(SemanticResponse::Error {
                        request_id,
                        message,
                    });
                }
            }
        }
    })
}

struct RankingJob {
    command: SemanticCommand,
    embedder: SharedEmbedder,
    cache: EmbeddingCache,
    progress_tx: UnboundedSender<SemanticResponse>,
}

fn run_ranking_job(job: RankingJob) -> (EmbeddingCache, SemanticResponse) {
    let RankingJob {
        command,
        embedder,
        mut cache,
        progress_tx,
    } = job;
    let SemanticCommand::RankRows {
        request_id,
        query,
        rows,
        title_column,
        options,
        cancel,
    } = command;
    let started = Instant::now();
    let mut progress = |done: usize, total: usize, backend: &str| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let _ = progress_tx.send(SemanticResponse::Progress {
            request_id,
            message: format!("Embedding batch {done}/{total} via {backend}"),
        });
    };
    let response = match rank_rows(
        embedder.as_ref(),
        &query,
        rows,
        title_column.as_deref(),
        options,
        &mut cache,
        Some(&mut progress),
        &cancel,
    ) {
        Ok((result, ranking)) => SemanticResponse::Success {
            request_id,
            result: Box::new(result),
            stats: SemanticStats {
                matched_rows: ranking.kept_rows,
                top_score: ranking.top_score,
                fallback_notice: ranking.fallback_notice,
                backend_label: ranking.backend_label,
                elapsed_ms: started.elapsed().as_millis(),
                cache_hits: ranking.cache_hits,
                cache_misses: ranking.cache_misses,
            },
        },
        Err(RankingError::Canceled) => SemanticResponse::Canceled { request_id },
        Err(RankingError::Failed(message)) => SemanticResponse::Error {
            request_id,
            message,
        },
    };
    (cache, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use poqi_engine::{QueryResult, ResultMetadata};
    use std::path::Path;
    use std::time::Duration;

    struct PanicEmbedder;

    impl pipeline::EmbeddingProvider for PanicEmbedder {
        fn encode(&self, _texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            panic!("embedding panic")
        }

        fn backend_label(&self) -> &'static str {
            "panic-test"
        }

        fn take_fallback_note(&self) -> Option<String> {
            None
        }

        fn model_path(&self) -> &Path {
            Path::new("panic-test.onnx")
        }
    }

    struct BlockingEmbedder {
        started: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }

    impl pipeline::EmbeddingProvider for BlockingEmbedder {
        fn encode(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
            self.started.store(true, Ordering::Release);
            while !self.release.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            Ok(texts.iter().map(|_| vec![1.0]).collect())
        }

        fn backend_label(&self) -> &'static str {
            "blocking-test"
        }

        fn take_fallback_note(&self) -> Option<String> {
            None
        }

        fn model_path(&self) -> &Path {
            Path::new("blocking-test.onnx")
        }
    }

    fn sample_command(request_id: u64) -> SemanticCommand {
        SemanticCommand::RankRows {
            request_id,
            query: "alpha".to_string(),
            rows: QueryResult {
                columns: vec!["name".to_string()],
                rows: vec![vec!["alpha".to_string()]],
                metadata: ResultMetadata::default(),
            },
            title_column: None,
            options: SearchOptions {
                batch_size: 1,
                top_k: 1,
                threshold: None,
                dim: 768,
            },
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    #[tokio::test]
    async fn semantic_panic_is_returned_as_request_error() {
        let (commands, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (responses, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let handle =
            spawn_semantic_worker(Some(Arc::new(PanicEmbedder)), None, command_rx, responses);
        commands.send(sample_command(31)).expect("semantic command");

        loop {
            match response_rx.recv().await.expect("semantic response") {
                SemanticResponse::Error {
                    request_id,
                    message,
                } => {
                    assert_eq!(request_id, 31);
                    assert!(message.contains("panicked"));
                    break;
                }
                SemanticResponse::Progress { .. } => {}
                other => panic!("unexpected semantic response: {other:?}"),
            }
        }
        drop(commands);
        handle.await.expect("semantic worker shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn aborting_semantic_coordinator_does_not_wait_for_blocking_inference() {
        let started = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let embedder = BlockingEmbedder {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        };
        let (commands, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (responses, _response_rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = spawn_semantic_worker(Some(Arc::new(embedder)), None, command_rx, responses);
        commands.send(sample_command(37)).expect("semantic command");
        while !started.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }

        handle.abort();
        let outcome = tokio::time::timeout(Duration::from_millis(250), handle)
            .await
            .expect("async coordinator abort should not wait for inference");
        assert!(outcome
            .expect_err("worker should be aborted")
            .is_cancelled());
        release.store(true, Ordering::Release);
    }
}
