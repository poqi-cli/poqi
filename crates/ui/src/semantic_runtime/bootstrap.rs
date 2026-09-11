use crate::{app::SemanticRuntime, runtime, semantic_bootstrap::SemanticBootstrapCoordinator};
use anyhow::Result;
use parking_lot::Mutex;
use poqi_config::{AppConfig, SemanticRuntimePreference};
use poqi_search_semantic::SearchOptions;
use std::{path::PathBuf, sync::Arc};
use tokio::{sync::mpsc, task::JoinHandle};

/// Bundles the semantic worker wiring plus any background handles that need to be awaited.
pub(crate) struct SemanticBootstrap {
    pub(crate) runtime: SemanticRuntime,
    pub(crate) handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

pub(crate) fn init_semantic_runtime(
    config: &AppConfig,
    coordinator: &SemanticBootstrapCoordinator,
) -> SemanticBootstrap {
    init_semantic_runtime_with_handles(config, coordinator, Arc::new(Mutex::new(Vec::new())))
}

pub(crate) fn init_semantic_runtime_with_handles(
    config: &AppConfig,
    coordinator: &SemanticBootstrapCoordinator,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
) -> SemanticBootstrap {
    let options = SearchOptions {
        batch_size: config.search.semantic_batch_size,
        top_k: config.search.semantic_top_k,
        threshold: config.search.semantic_score_threshold,
        dim: config.search.semantic_dim,
    };
    let title_column = config.search.semantic_title_column.clone();
    let preference = config.search.semantic_runtime_preference;

    // If semantic search is disabled, return immediately without any downloads
    if matches!(preference, SemanticRuntimePreference::Off) {
        return SemanticBootstrap {
            runtime: disabled_semantic_runtime(
                options,
                title_column.as_deref(),
                "Semantic search is off. Enable it in Settings → Semantic runtime.",
            ),
            handles,
        };
    }

    if coordinator.has_different_enabled_preference(preference) {
        return SemanticBootstrap {
            runtime: disabled_semantic_runtime(
                options,
                title_column.as_deref(),
                "Restart poqi to switch the semantic backend.",
            ),
            handles,
        };
    }

    let root = match resolve_model_root(config) {
        Ok(path) => path,
        Err(err) => {
            return SemanticBootstrap {
                runtime: disabled_semantic_runtime(
                    options,
                    title_column.as_deref(),
                    err.to_string(),
                ),
                handles,
            };
        }
    };

    let label = match runtime::initial_runtime_label(preference) {
        Ok(label) => label,
        Err(err) => {
            return SemanticBootstrap {
                runtime: disabled_semantic_runtime(
                    options,
                    title_column.as_deref(),
                    err.to_string(),
                ),
                handles,
            };
        }
    };

    let (event_tx, event_rx) = mpsc::unbounded_channel();
    coordinator.attach(
        config.clone(),
        root,
        preference,
        label.clone(),
        Arc::clone(&handles),
        event_tx,
    );
    let runtime = SemanticRuntime {
        tx: None,
        rx: None,
        options,
        title_column,
        disabled_reason: None,
        events: Some(event_rx),
        initial_loading_label: Some(label),
        initial_ready_summary: None,
    };
    SemanticBootstrap { runtime, handles }
}

fn disabled_semantic_runtime(
    options: SearchOptions,
    title_column: Option<&str>,
    reason: impl Into<String>,
) -> SemanticRuntime {
    SemanticRuntime {
        tx: None,
        rx: None,
        options,
        title_column: title_column.map(ToOwned::to_owned),
        disabled_reason: Some(reason.into()),
        events: None,
        initial_loading_label: None,
        initial_ready_summary: None,
    }
}

fn resolve_model_root(config: &AppConfig) -> Result<PathBuf> {
    if config.search.model_dir.is_absolute() {
        Ok(config.search.model_dir.clone())
    } else {
        Ok(AppConfig::config_dir()?.join(&config.search.model_dir))
    }
}

#[cfg(test)]
mod tests {
    use super::{init_semantic_runtime, init_semantic_runtime_with_handles};
    use crate::semantic_bootstrap::SemanticBootstrapCoordinator;
    use parking_lot::Mutex;
    use poqi_config::{AppConfig, SemanticRuntimePreference};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::{sync::mpsc, task::JoinHandle};

    static NEXT_OFF_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn off_creates_no_bootstrap_entry_handle_or_download_directory() {
        let coordinator = SemanticBootstrapCoordinator::new();
        let mut config = AppConfig::default();
        let root = std::env::temp_dir().join(format!(
            "poqi-off-semantic-test-{}-{}",
            std::process::id(),
            NEXT_OFF_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        config.search.model_dir = root.clone();

        let bootstrap = init_semantic_runtime(&config, &coordinator);

        assert_eq!(coordinator.entry_count(), 0);
        assert!(bootstrap.handles.lock().is_empty());
        assert!(bootstrap.runtime.events.is_none());
        assert!(bootstrap.runtime.disabled_reason.is_some());
        assert!(!root.exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_coordinator_blocks_backend_switch_during_next_initialization() {
        let coordinator = SemanticBootstrapCoordinator::new();
        let mut config = AppConfig::default();
        config.search.semantic_runtime_preference = SemanticRuntimePreference::Auto;
        config.search.model_dir = std::env::temp_dir().join("poqi-semantic-auto-test");
        coordinator.attach(
            config.clone(),
            config.search.model_dir.clone(),
            SemanticRuntimePreference::Auto,
            "test bootstrap".to_string(),
            empty_handles(),
            mpsc::unbounded_channel().0,
        );

        config.search.semantic_runtime_preference = SemanticRuntimePreference::Cpu;
        let bootstrap = init_semantic_runtime_with_handles(&config, &coordinator, empty_handles());

        assert_eq!(coordinator.entry_count(), 1);
        assert!(bootstrap.runtime.events.is_none());
        assert_eq!(
            bootstrap.runtime.disabled_reason.as_deref(),
            Some("Restart poqi to switch the semantic backend.")
        );
    }

    fn empty_handles() -> Arc<Mutex<Vec<JoinHandle<()>>>> {
        Arc::new(Mutex::new(Vec::new()))
    }
}
