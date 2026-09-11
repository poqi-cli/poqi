use anyhow::{anyhow, Result};
use poqi_config::{AppConfig, SemanticRuntimePreference};
use std::path::{Path, PathBuf};

pub(crate) const ORT_VERSION: &str = "1.23.0";
pub(crate) const CACHE_SUBDIR: &str = "runtimes/onxxruntime";
pub(crate) const DOWNLOAD_STEM: &str = "onnxruntime.download";
pub(crate) const DIRECTML_DOWNLOAD_STEM: &str = "directml.download";

#[derive(Debug, Clone, Copy)]
pub(crate) enum ArchiveType {
    Zip,
    Tgz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeBackend {
    Cpu,
    GpuDirectMl,
}

impl RuntimeBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::GpuDirectMl => "DirectML",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeArtifact {
    pub inner_path: &'static str,
    pub output_path: &'static str,
    pub output_sha256: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeDependency {
    pub(crate) download_stem: &'static str,
    pub(crate) download_urls: &'static [&'static str],
    pub(crate) archive_type: ArchiveType,
    pub(crate) label: &'static str,
    pub(crate) artifacts: &'static [RuntimeArtifact],
    pub(crate) sha256: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeSpec {
    pub platform_dir: &'static str,
    pub download_urls: &'static [&'static str],
    pub archive_type: ArchiveType,
    pub output_name: &'static str,
    pub artifacts: &'static [RuntimeArtifact],
    pub sha256: Option<&'static str>,
    pub backend: RuntimeBackend,
    pub dependencies: &'static [RuntimeDependency],
    pub retain_paths: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSelection {
    pub backend: RuntimeBackend,
    pub lib_path: PathBuf,
}

const DIRECTML_ARTIFACT: RuntimeArtifact = RuntimeArtifact {
    inner_path: "bin/x64-win/DirectML.dll",
    output_path: "DirectML.dll",
    output_sha256: "9c9e6d822561c6c41b90e6994b3e8857cf1d66dbfb1e0c4c799c7c89b4e92da1",
};

const DIRECTML_REDIST_SPEC: RuntimeDependency = RuntimeDependency {
    download_stem: DIRECTML_DOWNLOAD_STEM,
    download_urls: &["https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/1.15.4"],
    archive_type: ArchiveType::Zip,
    label: "DirectML.dll",
    artifacts: &[DIRECTML_ARTIFACT],
    sha256: Some("4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9"),
};

const WINDOWS_RUNTIME_ARTIFACTS: &[RuntimeArtifact] = &[
    RuntimeArtifact {
        inner_path: "runtimes/win-x64/native/onnxruntime.dll",
        output_path: "onnxruntime.dll",
        output_sha256: "f5131591edac6b0a8090d0e329040a49319d7a689cb5b465235fbf7030fa8027",
    },
    RuntimeArtifact {
        inner_path: "runtimes/win-x64/native/onnxruntime_providers_shared.dll",
        output_path: "onnxruntime_providers_shared.dll",
        output_sha256: "3b27e1417d12b73a6a34d80414c083e359e092d2f0ce572d7e67be8cdbe9e825",
    },
];

const WINDOWS_RETAIN_PATHS: &[&str] = &[
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "DirectML.dll",
];

const MAC_RUNTIME_ARTIFACTS: &[RuntimeArtifact] = &[RuntimeArtifact {
    inner_path: "onnxruntime-osx-universal2-1.23.0/lib/libonnxruntime.dylib",
    output_path: "lib/libonnxruntime.dylib",
    output_sha256: "10b6793bc75d4c3c675c529b3f2a9896cdd3a1f9fe50c600fe75036fb171a610",
}];

const LINUX_RUNTIME_ARTIFACTS: &[RuntimeArtifact] = &[RuntimeArtifact {
    inner_path: "onnxruntime-linux-x64-1.23.0/lib/libonnxruntime.so",
    output_path: "lib/libonnxruntime.so",
    output_sha256: "98b0253652d36c706cd9b873f3e8dc74e107c26cf9694672fb4d88da1c00f250",
}];

const WINDOWS_CPU_SPEC: RuntimeSpec = RuntimeSpec {
    platform_dir: "windows-x86_64",
    download_urls: &[
        "https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.23.0",
    ],
    archive_type: ArchiveType::Zip,
    output_name: "onnxruntime.dll",
    artifacts: WINDOWS_RUNTIME_ARTIFACTS,
    sha256: Some("a33ec2382b3c440bab74042a135733bb6e5085f293b908d3997688a58fe307e7"),
    backend: RuntimeBackend::Cpu,
    dependencies: &[],
    retain_paths: WINDOWS_RETAIN_PATHS,
};

const WINDOWS_GPU_SPEC: RuntimeSpec = RuntimeSpec {
    platform_dir: "windows-x86_64",
    download_urls: &[
        "https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.23.0",
    ],
    archive_type: ArchiveType::Zip,
    output_name: "onnxruntime.dll",
    artifacts: WINDOWS_RUNTIME_ARTIFACTS,
    sha256: Some("a33ec2382b3c440bab74042a135733bb6e5085f293b908d3997688a58fe307e7"),
    backend: RuntimeBackend::GpuDirectMl,
    dependencies: &[DIRECTML_REDIST_SPEC],
    retain_paths: WINDOWS_RETAIN_PATHS,
};

const MAC_CPU_SPEC: RuntimeSpec = RuntimeSpec {
    platform_dir: "macos-universal/cpu",
    download_urls: &["https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-osx-universal2-1.23.0.tgz"],
    archive_type: ArchiveType::Tgz,
    output_name: "lib/libonnxruntime.dylib",
    artifacts: MAC_RUNTIME_ARTIFACTS,
    sha256: Some("5e4365fb4a05aef353f6232b9a1848f37e608c421c9227e9224572205c0cfc08"),
    backend: RuntimeBackend::Cpu,
    dependencies: &[],
    retain_paths: &[],
};

const LINUX_CPU_SPEC: RuntimeSpec = RuntimeSpec {
    platform_dir: "linux-x86_64/cpu",
    download_urls: &["https://github.com/microsoft/onnxruntime/releases/download/v1.23.0/onnxruntime-linux-x64-1.23.0.tgz"],
    archive_type: ArchiveType::Tgz,
    output_name: "lib/libonnxruntime.so",
    artifacts: LINUX_RUNTIME_ARTIFACTS,
    sha256: Some("b6deea7f2e22c10c043019f294a0ea4d2a6c0ae52a009c34847640db75ec5580"),
    backend: RuntimeBackend::Cpu,
    dependencies: &[],
    retain_paths: &[],
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlatformSpecs {
    cpu: &'static RuntimeSpec,
    gpu: &'static [&'static RuntimeSpec],
}

pub(crate) fn detect_platform_specs() -> Result<PlatformSpecs> {
    if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        Ok(PlatformSpecs {
            cpu: &WINDOWS_CPU_SPEC,
            gpu: &[&WINDOWS_GPU_SPEC],
        })
    } else if cfg!(all(
        target_os = "macos",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        Ok(PlatformSpecs {
            cpu: &MAC_CPU_SPEC,
            gpu: &[],
        })
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Ok(PlatformSpecs {
            cpu: &LINUX_CPU_SPEC,
            gpu: &[],
        })
    } else {
        Err(anyhow!(
            "ONNX Runtime bootstrap is unsupported on this platform"
        ))
    }
}

pub(crate) fn candidate_specs(
    specs: PlatformSpecs,
    preference: SemanticRuntimePreference,
) -> Result<Vec<&'static RuntimeSpec>> {
    match preference {
        SemanticRuntimePreference::Off => Err(anyhow!("Semantic search is disabled")),
        SemanticRuntimePreference::Cpu => Ok(vec![specs.cpu]),
        SemanticRuntimePreference::Gpu => {
            if specs.gpu.is_empty() {
                Err(anyhow!(
                    "GPU semantic runtime is unavailable on this platform"
                ))
            } else {
                Ok(specs.gpu.to_vec())
            }
        }
        SemanticRuntimePreference::Auto => {
            let mut ordered = Vec::with_capacity(specs.gpu.len() + 1);
            ordered.extend_from_slice(specs.gpu);
            ordered.push(specs.cpu);
            Ok(ordered)
        }
    }
}

fn primary_runtime_spec(preference: SemanticRuntimePreference) -> Result<&'static RuntimeSpec> {
    let specs = detect_platform_specs()?;
    let candidates = candidate_specs(specs, preference)?;
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no ONNX Runtime candidates are available"))
}

pub(crate) fn initial_runtime_label(preference: SemanticRuntimePreference) -> Result<String> {
    if super::env::user_runtime_override()?.is_some() {
        return Ok("Preparing custom ONNX Runtime and model assets.".to_string());
    }
    let spec = primary_runtime_spec(preference)?;
    Ok(format!(
        "Preparing semantic runtime and model assets ({}).",
        spec.backend.label()
    ))
}

pub(crate) fn runtime_dir(_config: &AppConfig, spec: &RuntimeSpec) -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?
        .join(CACHE_SUBDIR)
        .join(ORT_VERSION)
        .join(spec.platform_dir))
}

pub(crate) fn join_relative(dir: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|segment| !segment.is_empty())
        .fold(dir.to_path_buf(), |acc, segment| acc.join(segment))
}
