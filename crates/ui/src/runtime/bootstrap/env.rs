use anyhow::{anyhow, Context, Result};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::OnceLock,
};

static USER_DEFINED_ORT_PATH: OnceLock<Option<OsString>> = OnceLock::new();

pub(crate) fn user_runtime_override() -> Result<Option<PathBuf>> {
    let raw = USER_DEFINED_ORT_PATH
        .get_or_init(|| std::env::var_os("ORT_DYLIB_PATH"))
        .clone();
    resolve_runtime_override(raw.as_deref())
}

fn resolve_runtime_override(raw: Option<&OsStr>) -> Result<Option<PathBuf>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let path_text = raw
        .to_str()
        .ok_or_else(|| anyhow!("ORT_DYLIB_PATH must be valid Unicode"))?;
    if path_text.trim().is_empty() {
        return Err(anyhow!("ORT_DYLIB_PATH is set but empty"));
    }

    let path = PathBuf::from(raw);
    let metadata = std::fs::metadata(&path).with_context(|| {
        format!(
            "ORT_DYLIB_PATH does not point to a readable file: {}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "ORT_DYLIB_PATH must point to an ONNX Runtime library file: {}",
            path.display()
        ));
    }
    let canonical = path.canonicalize().with_context(|| {
        format!(
            "failed to resolve the ONNX Runtime library configured by ORT_DYLIB_PATH: {}",
            path.display()
        )
    })?;
    Ok(Some(canonical))
}

pub(crate) fn configure_runtime_env(lib_path: &Path) {
    if let Some(parent) = lib_path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            tracing::warn!(?err, "failed to ensure runtime dir before setting env");
        }
        if let Err(err) = prepend_library_search_path(parent) {
            tracing::warn!(
                ?err,
                path = %parent.display(),
                "failed to extend library search path"
            );
        }
    }
    std::env::set_var("ORT_DYLIB_PATH", lib_path);
}

fn prepend_library_search_path(dir: &Path) -> Result<()> {
    let Some(var) = library_search_env_var() else {
        return Ok(());
    };
    let current = std::env::var_os(var);
    let mut entries: Vec<std::path::PathBuf> = current
        .as_ref()
        .map(|paths| std::env::split_paths(paths).collect())
        .unwrap_or_default();
    entries.retain(|existing| existing != dir);
    entries.insert(0, dir.to_path_buf());
    let joined = std::env::join_paths(entries).context("failed to rebuild library search path")?;
    std::env::set_var(var, joined);
    Ok(())
}

fn library_search_env_var() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        Some("PATH")
    } else if cfg!(target_os = "macos") {
        Some("DYLD_LIBRARY_PATH")
    } else if cfg!(unix) {
        Some("LD_LIBRARY_PATH")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_runtime_override;
    use std::ffi::OsStr;

    #[test]
    fn runtime_override_absence_skips_custom_runtime() {
        assert_eq!(resolve_runtime_override(None).unwrap(), None);
    }

    #[test]
    fn runtime_override_rejects_empty_value() {
        let error = resolve_runtime_override(Some(OsStr::new("  ")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("ORT_DYLIB_PATH is set but empty"));
    }

    #[test]
    fn runtime_override_rejects_missing_file() {
        let missing = std::env::temp_dir().join("poqi-definitely-missing-onnxruntime-library");
        let error = resolve_runtime_override(Some(missing.as_os_str()))
            .unwrap_err()
            .to_string();
        assert!(error.contains("ORT_DYLIB_PATH"));
        assert!(error.contains("readable file"));
    }

    #[test]
    fn runtime_override_accepts_existing_file_without_reading_environment() {
        let executable = std::env::current_exe().unwrap();
        let resolved = resolve_runtime_override(Some(executable.as_os_str())).unwrap();
        assert_eq!(resolved, Some(executable.canonicalize().unwrap()));
        assert!(resolved.is_some_and(|path| path.is_absolute()));
    }
}
