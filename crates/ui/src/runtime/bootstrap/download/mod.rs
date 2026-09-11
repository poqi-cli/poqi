mod archive;
pub(crate) mod policy;
mod transfer;

use super::{
    env::{configure_runtime_env, user_runtime_override},
    specs::{
        candidate_specs, detect_platform_specs, join_relative, runtime_dir, RuntimeBackend,
        RuntimeDependency, RuntimeSelection, RuntimeSpec,
    },
};
use crate::semantic_bootstrap::SemanticBootstrapEvent;
use anyhow::{anyhow, Context, Result};
use poqi_config::{AppConfig, SemanticRuntimePreference};
use poqi_search_semantic::{directml_is_available, initialize_runtime};
use std::path::PathBuf;
use tokio::{fs, sync::mpsc::UnboundedSender};

use archive::{download_archive_entry, prune_runtime_dir, ArchiveRequest};

/// Download or reuse the ONNX runtime that best matches the user's preference.
pub(crate) async fn ensure_runtime_lib_with_preference(
    config: &AppConfig,
    preference: SemanticRuntimePreference,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<RuntimeSelection> {
    if let Some(lib_path) = user_runtime_override()? {
        return prepare_user_runtime(lib_path, preference, event_tx);
    }

    let specs = detect_platform_specs()?;
    let candidates = candidate_specs(specs, preference)?;
    let mut last_err: Option<anyhow::Error> = None;

    for (idx, spec) in candidates.iter().enumerate() {
        let label = spec.backend.label();
        let phase = if idx == 0 {
            format!("Preparing ONNX Runtime ({label})…")
        } else {
            format!("Falling back to ONNX Runtime ({label})…")
        };
        send_status(event_tx, phase);

        match ensure_runtime_for_spec(config, spec, event_tx).await {
            Ok(lib_path) => {
                configure_runtime_env(&lib_path);
                initialize_runtime(&lib_path).with_context(|| {
                    format!(
                        "failed to load verified {label} ONNX Runtime at {}",
                        lib_path.display()
                    )
                })?;
                return Ok(RuntimeSelection {
                    backend: spec.backend,
                    lib_path,
                });
            }
            Err(err) => {
                send_status(
                    event_tx,
                    format!("Failed to initialize {label} runtime: {err}"),
                );
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("no ONNX Runtime candidates were available")))
}

fn prepare_user_runtime(
    lib_path: PathBuf,
    preference: SemanticRuntimePreference,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<RuntimeSelection> {
    send_status(
        event_tx,
        format!("Validating custom ONNX Runtime at {}…", lib_path.display()),
    );
    configure_runtime_env(&lib_path);
    initialize_runtime(&lib_path).with_context(|| {
        format!(
            "failed to load custom ONNX Runtime from ORT_DYLIB_PATH ({})",
            lib_path.display()
        )
    })?;

    let directml_available = if cfg!(target_os = "windows")
        && matches!(
            preference,
            SemanticRuntimePreference::Auto | SemanticRuntimePreference::Gpu
        ) {
        directml_is_available().context(
            "custom ONNX Runtime loaded, but its DirectML capability could not be determined",
        )?
    } else {
        false
    };
    let backend =
        custom_runtime_backend(preference, cfg!(target_os = "windows"), directml_available)?;
    send_status(
        event_tx,
        format!(
            "Using custom ONNX Runtime from ORT_DYLIB_PATH ({}).",
            backend.label()
        ),
    );
    Ok(RuntimeSelection { backend, lib_path })
}

fn custom_runtime_backend(
    preference: SemanticRuntimePreference,
    is_windows: bool,
    directml_available: bool,
) -> Result<RuntimeBackend> {
    match preference {
        SemanticRuntimePreference::Off => Err(anyhow!("Semantic search is disabled")),
        SemanticRuntimePreference::Auto if is_windows && directml_available => {
            Ok(RuntimeBackend::GpuDirectMl)
        }
        SemanticRuntimePreference::Cpu | SemanticRuntimePreference::Auto => Ok(RuntimeBackend::Cpu),
        SemanticRuntimePreference::Gpu if !is_windows => Err(anyhow!(
            "GPU semantic runtime is unavailable on this platform; poqi supports DirectML on Windows"
        )),
        SemanticRuntimePreference::Gpu if directml_available => Ok(RuntimeBackend::GpuDirectMl),
        SemanticRuntimePreference::Gpu => Err(anyhow!(
            "ORT_DYLIB_PATH points to a valid ONNX Runtime, but it does not advertise the DirectML execution provider required by GPU preference"
        )),
    }
}

async fn ensure_runtime_for_spec(
    config: &AppConfig,
    spec: &'static RuntimeSpec,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<PathBuf> {
    let dir = runtime_dir(config, spec)?;
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("failed to create runtime dir {}", dir.display()))?;

    let lib_path = join_relative(&dir, spec.output_name);
    if fs::try_exists(&lib_path).await? {
        if let Err(err) = archive::verify_artifacts(&dir, spec.artifacts, event_tx).await {
            send_status(
                event_tx,
                format!(
                    "Cached {} runtime failed checksum verification: {err}",
                    spec.backend.label()
                ),
            );
            download_and_extract(spec, &dir, event_tx).await?;
        }
    } else {
        download_and_extract(spec, &dir, event_tx).await?;
    }

    for dependency in spec.dependencies {
        ensure_dependency(&dir, dependency, event_tx).await?;
    }

    if !spec.retain_paths.is_empty() {
        prune_runtime_dir(&dir, spec.retain_paths).await?;
    }

    Ok(lib_path)
}

async fn download_and_extract(
    spec: &'static RuntimeSpec,
    dir: &std::path::Path,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<()> {
    let request = ArchiveRequest::runtime(spec);
    download_archive_entry(request, dir, event_tx).await?;
    let lib_path = join_relative(dir, spec.output_name);
    archive::set_unix_permissions(&lib_path)?;
    Ok(())
}

async fn ensure_dependency(
    dir: &std::path::Path,
    dependency: &'static RuntimeDependency,
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
) -> Result<()> {
    let mut missing = false;
    for artifact in dependency.artifacts {
        let target = join_relative(dir, artifact.output_path);
        if !fs::try_exists(&target).await? {
            missing = true;
            break;
        }
    }
    if !missing
        && archive::verify_artifacts(dir, dependency.artifacts, event_tx)
            .await
            .is_ok()
    {
        return Ok(());
    }

    let request = ArchiveRequest::dependency(dependency);
    download_archive_entry(request, dir, event_tx).await
}

pub(super) fn send_status(
    event_tx: Option<&UnboundedSender<SemanticBootstrapEvent>>,
    message: impl Into<String>,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(SemanticBootstrapEvent::Status(message.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::{custom_runtime_backend, ensure_runtime_lib_with_preference};
    use crate::runtime::RuntimeBackend;
    use poqi_config::AppConfig;
    use poqi_config::SemanticRuntimePreference;

    #[test]
    fn custom_runtime_backend_honors_cpu_and_auto_capabilities() {
        assert_eq!(
            custom_runtime_backend(SemanticRuntimePreference::Cpu, true, true).unwrap(),
            RuntimeBackend::Cpu
        );
        assert_eq!(
            custom_runtime_backend(SemanticRuntimePreference::Auto, true, true).unwrap(),
            RuntimeBackend::GpuDirectMl
        );
        assert_eq!(
            custom_runtime_backend(SemanticRuntimePreference::Auto, true, false).unwrap(),
            RuntimeBackend::Cpu
        );
        assert_eq!(
            custom_runtime_backend(SemanticRuntimePreference::Auto, false, true).unwrap(),
            RuntimeBackend::Cpu
        );
    }

    #[test]
    fn custom_runtime_backend_rejects_unsupported_explicit_gpu() {
        let missing_provider = custom_runtime_backend(SemanticRuntimePreference::Gpu, true, false)
            .unwrap_err()
            .to_string();
        assert!(missing_provider.contains("does not advertise the DirectML"));

        let wrong_platform = custom_runtime_backend(SemanticRuntimePreference::Gpu, false, true)
            .unwrap_err()
            .to_string();
        assert!(wrong_platform.contains("DirectML on Windows"));
    }

    #[tokio::test]
    #[ignore = "manual isolated probe; requires ORT_DYLIB_PATH to reference a real runtime"]
    async fn production_runtime_override_probe() {
        let preference = match std::env::var("POQI_RUNTIME_PROBE_BACKEND")
            .unwrap_or_else(|_| "cpu".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "auto" => SemanticRuntimePreference::Auto,
            "cpu" => SemanticRuntimePreference::Cpu,
            "gpu" => SemanticRuntimePreference::Gpu,
            other => panic!("POQI_RUNTIME_PROBE_BACKEND must be auto, cpu, or gpu; got {other}"),
        };
        assert!(
            std::env::var_os("ORT_DYLIB_PATH").is_some(),
            "ORT_DYLIB_PATH must be set by the isolated probe process"
        );

        let selection = ensure_runtime_lib_with_preference(&AppConfig::default(), preference, None)
            .await
            .expect("custom ONNX Runtime should load through the production resolver");

        eprintln!(
            "custom runtime probe: backend={}, path={}",
            selection.backend.label(),
            selection.lib_path.display()
        );
    }
}
