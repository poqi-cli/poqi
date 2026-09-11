#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use poqi_config::AppConfig;
use std::{io, path::PathBuf, sync::OnceLock};
use tracing::Level;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

const LOG_FILE_NAME: &str = "poqi-semantic.log";
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Initialize tracing subscribers with an environment filter.
///
/// # Errors
/// Returns an error if the tracing subscriber cannot be configured or initialized.
pub fn init_tracing() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;

    let (writer, guard, log_path) = build_writer().unwrap_or_else(|err| {
        eprintln!("poqi: failed to open log file ({err}); falling back to in-memory logs");
        let (fallback_writer, fallback_guard) = tracing_appender::non_blocking(io::sink());
        (fallback_writer, fallback_guard, None)
    });
    let _ = LOG_GUARD.set(guard);

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_timer(fmt::time::uptime())
        // Keep log output out of the TUI while still persisting to disk.
        .with_writer(writer);

    Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .try_init()
        .or_else(|_| {
            // If tracing was already initialized (e.g., in tests), ignore the error.
            tracing::event!(Level::DEBUG, "tracing already initialized");
            Ok::<(), anyhow::Error>(())
        })?;

    if let Some(path) = log_path {
        tracing::event!(
            Level::INFO,
            log_path = %path.display(),
            "file logging enabled"
        );
    }
    Ok(())
}

fn build_writer() -> Result<(NonBlocking, WorkerGuard, Option<PathBuf>)> {
    let log_path = resolve_log_path()?;
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create log directory {}", dir.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open log file {}", log_path.display()))?;
    let (writer, guard) = tracing_appender::non_blocking(file);
    Ok((writer, guard, Some(log_path)))
}

fn resolve_log_path() -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?.join("logs").join(LOG_FILE_NAME))
}

#[cfg(test)]
mod tests;
