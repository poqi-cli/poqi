use crate::{
    download_progress::{
        resume_action, send_status, temp_download_path, ProgressReporter, ResumeAction,
    },
    runtime::{
        self,
        bootstrap::download::policy::{download_client, write_http_response, DownloadLimit},
        RuntimeBackend,
    },
    semantic_runtime::load_embedder_from_root,
    semantic_worker::{spawn_semantic_worker, SemanticCommand, SemanticResponse},
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use poqi_config::{AppConfig, SemanticRuntimePreference};
use poqi_search_semantic::SemanticEmbedder;
use reqwest::{header::RANGE, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs,
    io::AsyncReadExt,
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::{yield_now, JoinHandle},
};
use tracing::{error, warn};

pub(crate) const CPU_MODEL_FILE: &str = "model-quint8-avx2.onnx";
pub(crate) const GPU_MODEL_FILE: &str = "model.onnx";
pub(crate) const TOKENIZER_FILE: &str = "tokenizer.json";

const MODEL_REVISION: &str = "c61e626a6255c490879d0af885078b61929d51f6";
const MODEL_DOWNLOAD_USER_AGENT: &str = "poqi-semantic-bootstrap/0.1";
const CPU_MODEL_URL: &str = "https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2/resolve/c61e626a6255c490879d0af885078b61929d51f6/onnx/model_quint8_avx2.onnx";
const CPU_MODEL_SHA256: &str = "a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e";
const GPU_MODEL_URL: &str = "https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2/resolve/c61e626a6255c490879d0af885078b61929d51f6/onnx/model.onnx";
const GPU_MODEL_SHA256: &str = "68e592b160673d30250824c1116bc6ab33f70efb22b97c9e1d7ce1e69c1c9d70";
const TOKENIZER_URL: &str = "https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2/resolve/c61e626a6255c490879d0af885078b61929d51f6/tokenizer.json";
const TOKENIZER_SHA256: &str = "4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f";

#[derive(Debug)]
struct AssetSpec {
    filename: &'static str,
    url: &'static str,
    sha256: &'static str,
    size_bytes: u64,
}

const CPU_MODEL_ASSET: AssetSpec = AssetSpec {
    filename: CPU_MODEL_FILE,
    url: CPU_MODEL_URL,
    sha256: CPU_MODEL_SHA256,
    size_bytes: 98_247_878,
};
const GPU_MODEL_ASSET: AssetSpec = AssetSpec {
    filename: GPU_MODEL_FILE,
    url: GPU_MODEL_URL,
    sha256: GPU_MODEL_SHA256,
    size_bytes: 390_004_608,
};
const TOKENIZER_ASSET: AssetSpec = AssetSpec {
    filename: TOKENIZER_FILE,
    url: TOKENIZER_URL,
    sha256: TOKENIZER_SHA256,
    size_bytes: 25_301_672,
};

#[derive(Debug)]
pub(crate) enum SemanticBootstrapEvent {
    Status(String),
    Ready {
        tx: UnboundedSender<SemanticCommand>,
        rx: UnboundedReceiver<SemanticResponse>,
        backend: RuntimeBackend,
    },
    Failed(String),
}

#[derive(Clone)]
struct PreparedSemanticRuntime {
    embedder: Arc<SemanticEmbedder>,
    backend: RuntimeBackend,
}

enum SharedBootstrapState {
    Loading { latest_status: String },
    Ready(PreparedSemanticRuntime),
    Failed(String),
}

struct SharedBootstrapSubscriber {
    tx: UnboundedSender<SemanticBootstrapEvent>,
    session_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

struct SharedBootstrapEntry {
    state: SharedBootstrapState,
    subscribers: Vec<SharedBootstrapSubscriber>,
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SemanticBootstrapKey {
    preference: &'static str,
    root: PathBuf,
}

/// Coordinates semantic runtime/model preparation across multiple UI sessions.
#[derive(Clone, Default)]
pub struct SemanticBootstrapCoordinator {
    inner: Arc<Mutex<HashMap<SemanticBootstrapKey, Arc<Mutex<SharedBootstrapEntry>>>>>,
}

impl SemanticBootstrapCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn attach(
        &self,
        config: AppConfig,
        root: PathBuf,
        preference: SemanticRuntimePreference,
        initial_label: String,
        session_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
        event_tx: UnboundedSender<SemanticBootstrapEvent>,
    ) {
        let key = SemanticBootstrapKey::new(preference, root.clone());
        let entry = {
            let mut inner = self.inner.lock();
            let existing = inner.get(&key).cloned();
            if let Some(entry) = existing
                .filter(|entry| !matches!(entry.lock().state, SharedBootstrapState::Failed(_)))
            {
                entry
            } else {
                let entry = Arc::new(Mutex::new(new_shared_bootstrap_entry(initial_label)));
                attach_shared_bootstrap_task(&entry, config, root, preference);
                inner.insert(key, Arc::clone(&entry));
                entry
            }
        };
        attach_to_entry(&entry, event_tx, session_handles);
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.inner.lock().len()
    }

    pub(crate) fn has_different_enabled_preference(
        &self,
        preference: SemanticRuntimePreference,
    ) -> bool {
        let preference = preference_key(preference);
        self.inner
            .lock()
            .keys()
            .any(|key| key.preference != "off" && key.preference != preference)
    }
}

impl Drop for SemanticBootstrapCoordinator {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) > 1 {
            return;
        }
        for entry in self.inner.lock().values() {
            entry.lock().handle.abort();
        }
    }
}

impl SemanticBootstrapKey {
    fn new(preference: SemanticRuntimePreference, root: PathBuf) -> Self {
        Self {
            preference: preference_key(preference),
            root,
        }
    }
}

fn preference_key(preference: SemanticRuntimePreference) -> &'static str {
    match preference {
        SemanticRuntimePreference::Off => "off",
        SemanticRuntimePreference::Auto => "auto",
        SemanticRuntimePreference::Gpu => "gpu",
        SemanticRuntimePreference::Cpu => "cpu",
    }
}

fn new_shared_bootstrap_entry(initial_label: String) -> SharedBootstrapEntry {
    SharedBootstrapEntry {
        state: SharedBootstrapState::Loading {
            latest_status: initial_label,
        },
        subscribers: Vec::new(),
        handle: tokio::spawn(async {}),
    }
}

fn attach_shared_bootstrap_task(
    entry: &Arc<Mutex<SharedBootstrapEntry>>,
    config: AppConfig,
    root: PathBuf,
    preference: SemanticRuntimePreference,
) {
    let entry = Arc::clone(entry);
    let task_entry = Arc::clone(&entry);
    let handle = tokio::spawn(async move {
        let (status_tx, mut status_rx) = mpsc::unbounded_channel();
        let status_entry = Arc::clone(&task_entry);
        let status_forwarder = tokio::spawn(async move {
            while let Some(event) = status_rx.recv().await {
                if let SemanticBootstrapEvent::Status(message) = event {
                    broadcast_status(&status_entry, &message);
                }
            }
        });
        let result = prepare_semantic_runtime(root, config, preference, status_tx.clone()).await;
        drop(status_tx);
        let _ = status_forwarder.await;
        match result {
            Ok(prepared) => broadcast_ready(&task_entry, &prepared),
            Err(err) => {
                error!(?err, "semantic bootstrap failed");
                broadcast_failed(&task_entry, &format!("Semantic bootstrap failed: {err}"));
            }
        }
    });
    entry.lock().handle = handle;
}

async fn prepare_semantic_runtime(
    root: PathBuf,
    config: AppConfig,
    preference: SemanticRuntimePreference,
    event_tx: UnboundedSender<SemanticBootstrapEvent>,
) -> Result<PreparedSemanticRuntime> {
    let runtime_selection =
        runtime::ensure_runtime_lib_with_preference(&config, preference, Some(&event_tx))
            .await
            .context("failed to prepare ONNX Runtime")?;

    fs::create_dir_all(&root)
        .await
        .with_context(|| format!("failed to create semantic model dir {}", root.display()))?;

    let client = download_client(MODEL_DOWNLOAD_USER_AGENT)?;

    let assets = assets_for_backend(runtime_selection.backend);
    send_status(
        Some(&event_tx),
        format!(
            "Verifying Granite Embedding assets ({} files, revision {}).",
            assets.len(),
            &MODEL_REVISION[..7]
        ),
    );

    for (idx, asset) in assets.iter().enumerate() {
        let dest = root.join(asset.filename);
        if asset_present_and_valid(asset, &dest, &event_tx).await? {
            continue;
        }
        let ordinal = idx + 1;
        send_status(
            Some(&event_tx),
            format!(
                "Downloading Granite Embedding asset {ordinal}/{}: {}.",
                assets.len(),
                asset.filename
            ),
        );
        download_asset(&client, asset, &dest, &event_tx).await?;
    }

    send_status(Some(&event_tx), "Loading semantic model…");
    let (embedder, backend, fallback_note) =
        load_embedder_from_root(&config, preference, Some(runtime_selection), &root)
            .await
            .context("failed to load semantic model after download")?;
    if let Some(note) = fallback_note {
        send_status(
            Some(&event_tx),
            format!("DirectML runtime unavailable — fell back to CPU ({note})"),
        );
    }
    Ok(PreparedSemanticRuntime { embedder, backend })
}

fn assets_for_backend(backend: RuntimeBackend) -> [&'static AssetSpec; 2] {
    let model = match backend {
        RuntimeBackend::Cpu => &CPU_MODEL_ASSET,
        RuntimeBackend::GpuDirectMl => &GPU_MODEL_ASSET,
    };
    [model, &TOKENIZER_ASSET]
}

fn attach_to_entry(
    entry: &Arc<Mutex<SharedBootstrapEntry>>,
    event_tx: UnboundedSender<SemanticBootstrapEvent>,
    session_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
) {
    let mut guard = entry.lock();
    match &guard.state {
        SharedBootstrapState::Loading { latest_status } => {
            let _ = event_tx.send(SemanticBootstrapEvent::Status(latest_status.clone()));
            guard.subscribers.push(SharedBootstrapSubscriber {
                tx: event_tx,
                session_handles,
            });
        }
        SharedBootstrapState::Ready(prepared) => {
            send_ready_to_session(&event_tx, &session_handles, prepared.clone());
        }
        SharedBootstrapState::Failed(message) => {
            let _ = event_tx.send(SemanticBootstrapEvent::Failed(message.clone()));
        }
    }
}

fn broadcast_status(entry: &Arc<Mutex<SharedBootstrapEntry>>, message: &str) {
    let mut guard = entry.lock();
    if let SharedBootstrapState::Loading { latest_status } = &mut guard.state {
        latest_status.clear();
        latest_status.push_str(message);
    }
    guard.subscribers.retain(|subscriber| {
        subscriber
            .tx
            .send(SemanticBootstrapEvent::Status(message.to_string()))
            .is_ok()
    });
}

fn broadcast_ready(entry: &Arc<Mutex<SharedBootstrapEntry>>, prepared: &PreparedSemanticRuntime) {
    let subscribers = {
        let mut guard = entry.lock();
        guard.state = SharedBootstrapState::Ready(prepared.clone());
        std::mem::take(&mut guard.subscribers)
    };
    for subscriber in subscribers {
        send_ready_to_session(
            &subscriber.tx,
            &subscriber.session_handles,
            prepared.clone(),
        );
    }
}

fn broadcast_failed(entry: &Arc<Mutex<SharedBootstrapEntry>>, message: &str) {
    let subscribers = {
        let mut guard = entry.lock();
        guard.state = SharedBootstrapState::Failed(message.to_string());
        std::mem::take(&mut guard.subscribers)
    };
    for subscriber in subscribers {
        let _ = subscriber
            .tx
            .send(SemanticBootstrapEvent::Failed(message.to_string()));
    }
}

fn send_ready_to_session(
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
    session_handles: &Arc<Mutex<Vec<JoinHandle<()>>>>,
    prepared: PreparedSemanticRuntime,
) {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = mpsc::unbounded_channel();
    let worker_handle =
        spawn_semantic_worker(Some(prepared.embedder), None, command_rx, response_tx);
    if event_tx
        .send(SemanticBootstrapEvent::Ready {
            tx: command_tx,
            rx: response_rx,
            backend: prepared.backend,
        })
        .is_ok()
    {
        session_handles.lock().push(worker_handle);
    } else {
        worker_handle.abort();
    }
}

async fn download_asset(
    client: &reqwest::Client,
    asset: &AssetSpec,
    dest: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<()> {
    let tmp_path = temp_download_path(dest);
    let limit = DownloadLimit::exact(asset.size_bytes);
    let mut restarted_after_integrity_mismatch = false;
    let mut retried_after_range_not_satisfiable = false;
    loop {
        let Some(resume_from) = prepare_temp_for_request(asset, &tmp_path, dest, event_tx).await?
        else {
            return Ok(());
        };

        let mut request = client.get(asset.url);
        if resume_from > 0 {
            send_status(
                Some(event_tx),
                format!(
                    "Resuming {} from {}.",
                    asset.filename,
                    crate::download_progress::format_mib(resume_from)
                ),
            );
            request = request.header(RANGE, format!("bytes={resume_from}-"));
        }

        let response = request
            .send()
            .await
            .with_context(|| format!("failed to request {}", asset.url))?;
        let status = response.status();
        if status == StatusCode::RANGE_NOT_SATISFIABLE {
            if fs::try_exists(&tmp_path).await? {
                let Some(mismatch) =
                    verify_and_finalize_temp(asset, &tmp_path, dest, event_tx).await?
                else {
                    return Ok(());
                };
                log_integrity_mismatch(&tmp_path, &mismatch);
                remove_invalid_temp(&tmp_path, &mismatch).await?;
                send_status(
                    Some(event_tx),
                    format!("{mismatch}; restarting clean download."),
                );
            }
            if retried_after_range_not_satisfiable {
                anyhow::bail!(
                    "server repeatedly rejected a clean download range for {}",
                    asset.filename
                );
            }
            retried_after_range_not_satisfiable = true;
            continue;
        }
        let response = response
            .error_for_status()
            .with_context(|| format!("download failed for {}", asset.url))?;

        write_http_response(
            response,
            &tmp_path,
            resume_from,
            limit,
            asset.filename,
            Some(event_tx),
        )
        .await?;

        let Some(mismatch) = verify_and_finalize_temp(asset, &tmp_path, dest, event_tx).await?
        else {
            return Ok(());
        };
        log_integrity_mismatch(&tmp_path, &mismatch);
        remove_invalid_temp(&tmp_path, &mismatch).await?;
        if !restarted_after_integrity_mismatch {
            restarted_after_integrity_mismatch = true;
            send_status(
                Some(event_tx),
                format!("{mismatch}; restarting clean download."),
            );
            continue;
        }
        anyhow::bail!("{mismatch} after clean download; invalid temporary file removed");
    }
}

async fn prepare_temp_for_request(
    asset: &AssetSpec,
    tmp_path: &Path,
    dest: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<Option<u64>> {
    let existing_bytes = match fs::metadata(tmp_path).await {
        Ok(metadata) => Some(metadata.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", tmp_path.display()));
        }
    };
    match resume_action(existing_bytes, Some(asset.size_bytes)) {
        ResumeAction::StartFresh => Ok(Some(0)),
        ResumeAction::Resume(bytes) => Ok(Some(bytes)),
        ResumeAction::VerifyComplete | ResumeAction::DeleteAndRestart => {
            let Some(mismatch) = verify_and_finalize_temp(asset, tmp_path, dest, event_tx).await?
            else {
                return Ok(None);
            };
            log_integrity_mismatch(tmp_path, &mismatch);
            remove_invalid_temp(tmp_path, &mismatch).await?;
            send_status(
                Some(event_tx),
                format!("{mismatch}; restarting clean download."),
            );
            Ok(Some(0))
        }
    }
}

fn log_integrity_mismatch(path: &Path, mismatch: &str) {
    warn!(path = %path.display(), mismatch, "semantic asset integrity mismatch");
}

async fn verify_and_finalize_temp(
    asset: &AssetSpec,
    tmp_path: &Path,
    dest: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<Option<String>> {
    let mismatch = asset_integrity_mismatch(asset, tmp_path, event_tx).await?;
    if mismatch.is_none() {
        finalize_temp_asset(tmp_path, dest).await?;
    }
    Ok(mismatch)
}

async fn remove_invalid_temp(tmp_path: &Path, mismatch: &str) -> Result<()> {
    fs::remove_file(tmp_path).await.with_context(|| {
        format!(
            "{mismatch}; failed to remove invalid temp {}",
            tmp_path.display()
        )
    })
}

async fn finalize_temp_asset(tmp_path: &Path, dest: &Path) -> Result<()> {
    fs::rename(tmp_path, dest)
        .await
        .with_context(|| format!("failed to finalize {}", dest.display()))?;
    Ok(())
}

async fn asset_present_and_valid(
    asset: &AssetSpec,
    dest: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<bool> {
    if !fs::try_exists(dest).await? {
        return Ok(false);
    }

    let Some(mismatch) = asset_integrity_mismatch(asset, dest, event_tx).await? else {
        return Ok(true);
    };

    log_integrity_mismatch(dest, &mismatch);
    send_status(
        Some(event_tx),
        format!("Cached {mismatch}; downloading pinned copy."),
    );
    fs::remove_file(dest)
        .await
        .with_context(|| format!("{mismatch}; failed to remove stale {}", dest.display()))?;
    Ok(false)
}

async fn asset_integrity_mismatch(
    asset: &AssetSpec,
    path: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<Option<String>> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.is_file() {
        return Ok(Some(format!(
            "invalid asset {} (expected a regular file of {} bytes)",
            asset.filename, asset.size_bytes
        )));
    }
    let actual_size = metadata.len();
    if actual_size != asset.size_bytes {
        return Ok(Some(format!(
            "size mismatch for {} (expected {} bytes, got {actual_size})",
            asset.filename, asset.size_bytes
        )));
    }

    let digest = file_sha256(asset, path, event_tx)
        .await
        .with_context(|| format!("failed to verify {}", path.display()))?;
    if digest != asset.sha256 {
        return Ok(Some(format!(
            "checksum mismatch for {} (expected {}, got {digest})",
            asset.filename, asset.sha256
        )));
    }
    Ok(None)
}

async fn file_sha256(
    asset: &AssetSpec,
    path: &Path,
    event_tx: &UnboundedSender<SemanticBootstrapEvent>,
) -> Result<String> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut read_total = 0_u64;
    let mut progress = ProgressReporter::new(
        "Verifying",
        asset.filename,
        Some(asset.size_bytes),
        Some(event_tx),
    );
    progress.emit(0);
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        read_total += read as u64;
        hasher.update(&buffer[..read]);
        progress.maybe_emit(read_total);
        yield_now().await;
    }
    progress.finish(read_total);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::{
        assets_for_backend, AssetSpec, SemanticBootstrapCoordinator, SemanticBootstrapKey,
        SharedBootstrapEntry, SharedBootstrapState, CPU_MODEL_FILE, GPU_MODEL_FILE, TOKENIZER_FILE,
    };
    use crate::runtime::RuntimeBackend;
    use parking_lot::Mutex;
    use poqi_config::{AppConfig, SemanticRuntimePreference};
    use sha2::{Digest, Sha256};
    use std::{path::PathBuf, sync::Arc};
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
        task::JoinHandle,
    };

    #[tokio::test]
    #[ignore = "manual isolated probe; downloads the pinned model from its publisher"]
    async fn pinned_model_download_probe() {
        let root = std::env::var_os("POQI_MODEL_DOWNLOAD_PROBE_DIR")
            .map(PathBuf::from)
            .expect("set POQI_MODEL_DOWNLOAD_PROBE_DIR to an isolated writable directory");
        tokio::fs::create_dir_all(&root)
            .await
            .expect("create probe directory");
        let client = super::download_client(super::MODEL_DOWNLOAD_USER_AGENT)
            .expect("create model download client");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress = tokio::spawn(async move {
            while let Some(super::SemanticBootstrapEvent::Status(message)) = rx.recv().await {
                eprintln!("{message}");
            }
        });
        let asset = &super::GPU_MODEL_ASSET;
        let dest = root.join(asset.filename);
        let result = super::download_asset(&client, asset, &dest, &tx).await;
        drop(tx);
        progress.await.expect("progress task");
        result.expect("download and verify pinned model");
    }

    #[tokio::test]
    async fn corrupt_same_size_download_retries_clean_and_finishes_valid() {
        let expected = b"correct";
        let corrupt = b"wrong!!";
        let (url, server) = serve_model_bodies(vec![corrupt.to_vec(), expected.to_vec()]).await;
        let asset = test_asset(url, expected);
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join(asset.filename);
        let (tx, mut rx) = mpsc::unbounded_channel();

        super::download_asset(&test_client(), &asset, &dest, &tx)
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(&dest).await.unwrap(), expected);
        assert!(!tokio::fs::try_exists(super::temp_download_path(&dest))
            .await
            .unwrap());
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("\r\nrange:")));
        let statuses = received_statuses(&mut rx);
        assert!(statuses.iter().any(|message| {
            message.contains(asset.sha256)
                && message.contains(&sha256(corrupt))
                && message.contains("restarting clean download")
        }));
    }

    #[tokio::test]
    async fn twice_corrupt_download_reports_digests_and_removes_invalid_file() {
        let expected = b"correct";
        let first_corrupt = b"wrong!!";
        let final_corrupt = b"broken?";
        let (url, server) =
            serve_model_bodies(vec![first_corrupt.to_vec(), final_corrupt.to_vec()]).await;
        let asset = test_asset(url, expected);
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join(asset.filename);
        let (tx, _rx) = mpsc::unbounded_channel();

        let error = super::download_asset(&test_client(), &asset, &dest, &tx)
            .await
            .unwrap_err();

        let message = format!("{error:#}");
        assert!(message.contains("checksum mismatch"));
        assert!(message.contains(asset.sha256));
        assert!(message.contains(&sha256(final_corrupt)));
        assert!(message.contains("invalid temporary file removed"));
        assert!(!tokio::fs::try_exists(&dest).await.unwrap());
        assert!(!tokio::fs::try_exists(super::temp_download_path(&dest))
            .await
            .unwrap());
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("\r\nrange:")));
    }

    #[tokio::test]
    async fn wrong_size_download_reports_size_without_checksum_claim() {
        let expected = b"correct";
        let (url, server) = serve_model_bodies(vec![b"short".to_vec()]).await;
        let asset = test_asset(url, expected);
        let temp = TempDir::new().unwrap();
        let dest = temp.path().join(asset.filename);
        let (tx, _rx) = mpsc::unbounded_channel();

        let error = super::download_asset(&test_client(), &asset, &dest, &tx)
            .await
            .unwrap_err();

        let message = format!("{error:#}");
        assert!(message.contains("invalid size for test-model.onnx"));
        assert!(message.contains("expected 7 bytes, got 5"));
        assert!(!message.contains("checksum mismatch"));
        assert!(!tokio::fs::try_exists(&dest).await.unwrap());
        assert!(!tokio::fs::try_exists(super::temp_download_path(&dest))
            .await
            .unwrap());
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn coordinator_reuses_entry_for_same_key() {
        let coordinator = SemanticBootstrapCoordinator::new();
        let config = AppConfig::default();
        let root = PathBuf::from("test-model-root");

        coordinator.attach(
            config.clone(),
            root.clone(),
            SemanticRuntimePreference::Off,
            "Preparing semantic runtime and model assets (test).".to_string(),
            empty_session_handles(),
            mpsc::unbounded_channel().0,
        );
        coordinator.attach(
            config,
            root,
            SemanticRuntimePreference::Off,
            "Preparing semantic runtime and model assets (test).".to_string(),
            empty_session_handles(),
            mpsc::unbounded_channel().0,
        );

        assert_eq!(coordinator.entry_count(), 1);
    }

    #[tokio::test]
    async fn coordinator_separates_different_keys() {
        let coordinator = SemanticBootstrapCoordinator::new();
        let config = AppConfig::default();

        coordinator.attach(
            config.clone(),
            PathBuf::from("test-model-root-a"),
            SemanticRuntimePreference::Off,
            "Preparing semantic runtime and model assets (test).".to_string(),
            empty_session_handles(),
            mpsc::unbounded_channel().0,
        );
        coordinator.attach(
            config,
            PathBuf::from("test-model-root-b"),
            SemanticRuntimePreference::Off,
            "Preparing semantic runtime and model assets (test).".to_string(),
            empty_session_handles(),
            mpsc::unbounded_channel().0,
        );

        assert_eq!(coordinator.entry_count(), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_same_key_is_replaced_with_fresh_loading_attempt() {
        let coordinator = SemanticBootstrapCoordinator::new();
        let mut config = AppConfig::default();
        config.search.semantic_runtime_preference = SemanticRuntimePreference::Auto;
        let root = PathBuf::from("retry-model-root");
        let key = SemanticBootstrapKey::new(SemanticRuntimePreference::Auto, root.clone());
        let failed_entry = Arc::new(Mutex::new(SharedBootstrapEntry {
            state: SharedBootstrapState::Failed("transient failure".to_string()),
            subscribers: Vec::new(),
            handle: tokio::spawn(async {}),
        }));
        let failed_handle_id = failed_entry.lock().handle.id();
        coordinator
            .inner
            .lock()
            .insert(key.clone(), Arc::clone(&failed_entry));
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        coordinator.attach(
            config,
            root,
            SemanticRuntimePreference::Auto,
            "Retrying semantic bootstrap".to_string(),
            empty_session_handles(),
            event_tx,
        );

        let fresh_entry = coordinator
            .inner
            .lock()
            .get(&key)
            .cloned()
            .expect("replacement entry");
        assert!(!Arc::ptr_eq(&failed_entry, &fresh_entry));
        let fresh_guard = fresh_entry.lock();
        assert!(matches!(
            fresh_guard.state,
            SharedBootstrapState::Loading { .. }
        ));
        assert_ne!(fresh_guard.handle.id(), failed_handle_id);
        drop(fresh_guard);
        assert!(matches!(
            event_rx.try_recv(),
            Ok(super::SemanticBootstrapEvent::Status(message))
                if message == "Retrying semantic bootstrap"
        ));
        assert_eq!(coordinator.entry_count(), 1);
        assert!(!coordinator.has_different_enabled_preference(SemanticRuntimePreference::Auto));
    }

    fn test_client() -> reqwest::Client {
        super::download_client("poqi-semantic-bootstrap-test").unwrap()
    }

    fn test_asset(url: String, expected: &[u8]) -> AssetSpec {
        AssetSpec {
            filename: "test-model.onnx",
            url: Box::leak(url.into_boxed_str()),
            sha256: Box::leak(sha256(expected).into_boxed_str()),
            size_bytes: expected.len() as u64,
        }
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    async fn serve_model_bodies(bodies: Vec<Vec<u8>>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::with_capacity(bodies.len());
            for body in bodies {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(read_http_request(&mut socket).await);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
                socket.flush().await.unwrap();
            }
            requests
        });
        (format!("http://{address}/model"), server)
    }

    async fn read_http_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 512];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let read = socket.read(&mut buffer).await.unwrap();
            assert!(read > 0, "request ended before headers completed");
            request.extend_from_slice(&buffer[..read]);
            assert!(request.len() <= 8 * 1024, "request headers too large");
        }
        String::from_utf8(request).unwrap()
    }

    fn received_statuses(
        rx: &mut mpsc::UnboundedReceiver<super::SemanticBootstrapEvent>,
    ) -> Vec<String> {
        let mut statuses = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let super::SemanticBootstrapEvent::Status(message) = event {
                statuses.push(message);
            }
        }
        statuses
    }

    fn empty_session_handles() -> Arc<Mutex<Vec<JoinHandle<()>>>> {
        Arc::new(Mutex::new(Vec::new()))
    }

    #[test]
    fn backend_assets_keep_compact_cpu_and_directml_models_separate() {
        let cpu = assets_for_backend(RuntimeBackend::Cpu);
        assert_eq!(cpu[0].filename, CPU_MODEL_FILE);
        assert_eq!(cpu[1].filename, TOKENIZER_FILE);

        let gpu = assets_for_backend(RuntimeBackend::GpuDirectMl);
        assert_eq!(gpu[0].filename, GPU_MODEL_FILE);
        assert_eq!(gpu[1].filename, TOKENIZER_FILE);
        assert!(cpu[0].size_bytes < gpu[0].size_bytes);
    }
}
