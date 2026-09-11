use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Mutex as StdMutex,
};

use anyhow::{anyhow, Result};
use ort::{
    execution_providers::{directml::DirectMLExecutionProvider, ExecutionProvider},
    session::Session,
    value::{DynValue, Tensor},
};
use parking_lot::Mutex;
use tokenizers::{Tokenizer, TruncationParams};
use tracing::{info, warn};

mod runtime;
mod tokens;

use crate::inputs::{build_model_inputs, select_embedding_value, ModelEncoding, ModelInputSpec};
use runtime::{build_session, extract_embeddings, run_with_session, NonFiniteValuesError};
use tokens::SpecialTokens;

const DEFAULT_MAX_SEQ_LEN: usize = 2048;

static ACTIVE_RUNTIME_PATH: StdMutex<Option<PathBuf>> = StdMutex::new(None);

/// Load and initialize the process-global ONNX Runtime from `lib_path`.
///
/// # Errors
/// Returns an error when the library cannot be loaded, has an incompatible API,
/// or a different runtime library was already initialized by this process.
pub fn initialize_runtime(lib_path: &Path) -> Result<()> {
    let path_text = lib_path.to_str().ok_or_else(|| {
        anyhow!(
            "ONNX Runtime path is not valid Unicode: {}",
            lib_path.display()
        )
    })?;
    let mut active = ACTIVE_RUNTIME_PATH
        .lock()
        .map_err(|_| anyhow!("ONNX Runtime initialization lock was poisoned"))?;
    if let Some(active_path) = active.as_ref() {
        if active_path == lib_path {
            return Ok(());
        }
        return Err(anyhow!(
            "ONNX Runtime is already initialized from {}; cannot switch to {} in the same process",
            active_path.display(),
            lib_path.display()
        ));
    }

    let initialized = catch_unwind(AssertUnwindSafe(|| ort::init_from(path_text).commit()))
        .map_err(|panic| {
            anyhow!(
                "ONNX Runtime load panicked: {}",
                panic_message(panic.as_ref())
            )
        })?
        .map_err(|err| anyhow!("ONNX Runtime initialization failed: {err}"))?;
    if !initialized {
        return Err(anyhow!(
            "ONNX Runtime was initialized before poqi could validate {}",
            lib_path.display()
        ));
    }
    *active = Some(lib_path.to_path_buf());
    Ok(())
}

/// Report whether the loaded ONNX Runtime advertises the `DirectML` provider.
///
/// # Errors
/// Returns an error when ONNX Runtime cannot enumerate its providers.
pub fn directml_is_available() -> Result<bool> {
    catch_unwind(AssertUnwindSafe(|| {
        DirectMLExecutionProvider::default().is_available()
    }))
    .map_err(|panic| {
        anyhow!(
            "DirectML capability query panicked: {}",
            panic_message(panic.as_ref())
        )
    })?
    .map_err(|err| anyhow!("failed to query DirectML capability: {err}"))
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied())
        .unwrap_or("unknown panic")
}

/// Wraps the ONNX runtime session and tokenizer used for semantic embeddings.
#[derive(Debug)]
pub struct SemanticEmbedder {
    tokenizer: Tokenizer,
    special_tokens: SpecialTokens,
    session: Mutex<Session>,
    max_tokens: usize,
    strategy: ExecutionStrategy,
    model_path: PathBuf,
    cpu_fallback: Mutex<Option<Session>>,
    dml_disabled: AtomicBool,
    fallback_note: Mutex<Option<String>>,
    has_attention_mask: bool,
    has_token_type_ids: bool,
    has_position_ids: bool,
    preferred_output: Option<String>,
}

/// Controls how the ONNX session is wired; Windows can opt into `DirectML` while
/// every other platform runs on CPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStrategy {
    /// Only register the built-in CPU execution provider.
    Cpu,
    /// Register the `DirectML` provider (Windows) and optionally fail if it cannot initialize.
    DirectMl { fail_on_error: bool },
}

impl SemanticEmbedder {
    /// Build an ONNX runtime session and tokenizer from disk.
    ///
    /// # Errors
    /// Returns an error if the ONNX model or tokenizer cannot be loaded.
    pub fn from_files(
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        strategy: ExecutionStrategy,
    ) -> Result<Self> {
        let model_path = model_path.as_ref().to_path_buf();
        let mut tokenizer = Tokenizer::from_file(tokenizer_path.as_ref())
            .map_err(|err| anyhow!("failed to load tokenizer: {err}"))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: DEFAULT_MAX_SEQ_LEN,
                ..TruncationParams::default()
            }))
            .map_err(|err| anyhow!("failed to configure tokenizer truncation: {err}"))?;
        let special_tokens = SpecialTokens::from_tokenizer(&tokenizer)?;

        let session = build_session(&model_path, strategy)?;

        let has_attention_mask = session
            .inputs
            .iter()
            .any(|input| input.name == "attention_mask");
        let has_token_type_ids = session
            .inputs
            .iter()
            .any(|input| input.name == "token_type_ids");
        let has_position_ids = session
            .inputs
            .iter()
            .any(|input| input.name == "position_ids");
        let preferred_output = session
            .outputs
            .iter()
            .find(|output| {
                matches!(
                    output.name.as_str(),
                    "sentence_embedding" | "last_hidden_state"
                )
            })
            .map(|output| output.name.clone());
        if preferred_output.is_none() && session.outputs.len() != 1 {
            let names = session
                .outputs
                .iter()
                .map(|output| output.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "Granite model has no recognized embedding output; available outputs: {names}"
            ));
        }

        Ok(Self {
            tokenizer,
            special_tokens,
            session: Mutex::new(session),
            max_tokens: DEFAULT_MAX_SEQ_LEN,
            strategy,
            model_path,
            cpu_fallback: Mutex::new(None),
            dml_disabled: AtomicBool::new(false),
            fallback_note: Mutex::new(None),
            has_attention_mask,
            has_token_type_ids,
            has_position_ids,
            preferred_output,
        })
    }

    /// Maximum number of tokens allowed per sequence.
    #[must_use]
    pub fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    /// Absolute path to the loaded model weights.
    #[must_use]
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Encode the provided texts into normalized embedding vectors.
    ///
    /// # Errors
    /// Returns an error if tokenization or model inference fails.
    pub fn encode(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if matches!(self.strategy, ExecutionStrategy::DirectMl { .. })
            && self.dml_disabled.load(Ordering::Relaxed)
        {
            return self.run_cpu_fallback_forced(texts);
        }

        let encodings = self.tokenize(texts)?;
        match self.run_primary(&encodings) {
            Ok(vectors) => Ok(vectors),
            Err(err) if self.is_directml_non_finite(&err) => {
                self.disable_directml_once();
                warn!("DirectML produced non-finite embeddings; retrying on CPU fallback");
                self.run_cpu_fallback(&encodings).or(Err(err))
            }
            Err(err) => Err(err),
        }
    }

    fn tokenize(&self, texts: &[String]) -> Result<Vec<ModelEncoding>> {
        texts.iter().map(|text| self.encode_text(text)).collect()
    }

    fn encode_text(&self, text: &str) -> Result<ModelEncoding> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|err| anyhow!("tokenization failed: {err}"))?;
        let ids = encoding
            .get_ids()
            .iter()
            .copied()
            .map(i64::from)
            .collect::<Vec<_>>();
        self.special_tokens.validate_sequence(&ids)?;
        let attention_mask = encoding
            .get_attention_mask()
            .iter()
            .copied()
            .map(i64::from)
            .collect();
        Ok(ModelEncoding {
            ids,
            attention_mask,
        })
    }

    fn max_sequence_len(&self, encodings: &[ModelEncoding]) -> usize {
        encodings
            .iter()
            .map(|encoding| encoding.ids.len())
            .max()
            .unwrap_or(1)
            .min(self.max_tokens)
            .max(1)
    }

    fn run_primary(&self, encodings: &[ModelEncoding]) -> Result<Vec<Vec<f32>>> {
        let feeds = self.build_feeds(encodings)?;
        let embedding_value = self.run_model(feeds)?;
        extract_embeddings(&embedding_value, self.backend_label(), encodings.len())
    }

    fn run_cpu_fallback(&self, encodings: &[ModelEncoding]) -> Result<Vec<Vec<f32>>> {
        let feeds = self.build_feeds(encodings)?;
        let embedding_value = {
            let mut guard = self.cpu_fallback.lock();
            if guard.is_none() {
                *guard = Some(build_session(&self.model_path, ExecutionStrategy::Cpu)?);
            }
            let session = guard.as_mut().expect("fallback session initialized");
            let outputs = session.run(feeds)?;
            select_embedding_value(outputs, self.preferred_output.as_deref())?
        };
        info!("Using CPU fallback for semantic embeddings");
        extract_embeddings(&embedding_value, "CPU", encodings.len())
    }

    fn run_cpu_fallback_forced(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let encodings = self.tokenize(texts)?;
        let feeds = self.build_feeds(&encodings)?;
        let embedding_value = {
            let mut guard = self.cpu_fallback.lock();
            if guard.is_none() {
                *guard = Some(build_session(&self.model_path, ExecutionStrategy::Cpu)?);
            }
            let session = guard.as_mut().expect("fallback session initialized");
            let outputs = session.run(feeds)?;
            select_embedding_value(outputs, self.preferred_output.as_deref())?
        };
        info!("DirectML disabled; using CPU for semantic embeddings");
        extract_embeddings(&embedding_value, "CPU", encodings.len())
    }

    fn build_feeds(&self, encodings: &[ModelEncoding]) -> Result<Vec<(&'static str, Tensor<i64>)>> {
        let batch = encodings.len();
        let max_len = self.max_sequence_len(encodings);
        build_model_inputs(
            encodings,
            batch,
            max_len,
            ModelInputSpec {
                pad_token_id: self.special_tokens.pad,
                has_attention_mask: self.has_attention_mask,
                has_token_type_ids: self.has_token_type_ids,
                has_position_ids: self.has_position_ids,
            },
        )
    }

    fn is_directml_non_finite(&self, err: &anyhow::Error) -> bool {
        matches!(self.strategy, ExecutionStrategy::DirectMl { .. })
            && err.is::<NonFiniteValuesError>()
    }

    fn disable_directml_once(&self) {
        if self
            .dml_disabled
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            let mut note = self.fallback_note.lock();
            *note = Some(
                "DirectML returned NaN/Inf embeddings; using CPU for the rest of this session"
                    .to_string(),
            );
        }
    }

    pub fn take_fallback_note(&self) -> Option<String> {
        let mut note = self.fallback_note.lock();
        note.take()
    }

    pub fn backend_label(&self) -> &'static str {
        if self.dml_disabled.load(Ordering::Relaxed) {
            "CPU (fallback)"
        } else {
            match self.strategy {
                ExecutionStrategy::DirectMl { .. } => "DirectML",
                ExecutionStrategy::Cpu => "CPU",
            }
        }
    }

    fn run_model(&self, feeds: Vec<(&str, Tensor<i64>)>) -> Result<DynValue> {
        run_with_session(&self.session, feeds, self.preferred_output.as_deref())
    }
}
