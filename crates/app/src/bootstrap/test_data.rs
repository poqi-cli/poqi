use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::style::Stylize;
use poqi_db::{ConnectionProfile, Database};
use poqi_store::{NewConnectionProfile, Store};
use poqi_testdata::{
    check_docker_available, generate_test_database, remove_test_container,
    start_test_container_with_progress, TestContainer, TestDataConfig,
};

/// Spawns a background thread that builds a Docker-backed test database and persists the profile.
pub(super) fn generate_and_save_test_database(store: &Store) -> Result<ConnectionProfile> {
    let handle = spawn_generation_thread(store.path.clone());
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("Test data generation thread panicked"))?
}

#[derive(Clone, Copy)]
enum Tone {
    Header,
    Container,
    Success,
    Error,
}

fn spawn_generation_thread(
    store_path: Option<PathBuf>,
) -> std::thread::JoinHandle<Result<ConnectionProfile>> {
    std::thread::spawn(move || run_generation_job(store_path))
}

fn run_generation_job(store_path: Option<PathBuf>) -> Result<ConnectionProfile> {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { generate_database(store_path).await })
}

async fn generate_database(store_path: Option<PathBuf>) -> Result<ConnectionProfile> {
    log_to_user("Starting test database generation...", Tone::Header);

    if let Err(e) = check_docker_available().await {
        let message = format!("Docker not available: {e}");
        log_to_user(&message, Tone::Error);
        anyhow::bail!("{message}");
    }

    let container = start_test_container_with_progress(|message| {
        let display = format!("{} {message}", "[docker]".cyan());
        log_to_user(&display, Tone::Container);
    })
    .await
    .context("failed to start PostgreSQL container")?;

    match populate_and_persist_test_database(store_path, &container).await {
        Ok(profile) => Ok(profile),
        Err(error) => {
            log_to_user(
                "Cleaning up the unsuccessful test container...",
                Tone::Container,
            );
            if let Err(cleanup_error) = remove_test_container(&container.container_id).await {
                return Err(error.context(format!(
                    "cleanup of test data container {} also failed: {cleanup_error}",
                    container.container_id
                )));
            }
            Err(error)
        }
    }
}

async fn populate_and_persist_test_database(
    store_path: Option<PathBuf>,
    container: &TestContainer,
) -> Result<ConnectionProfile> {
    let profile = ConnectionProfile::new("temp".to_string(), container.connection_uri.clone());
    let database = Database::new();
    database
        .connect(&profile)
        .await
        .context("failed to connect to test database")?;

    let config = TestDataConfig::default_config();

    log_to_user("Generating test data...", Tone::Header);
    generate_test_database(&database, config, |progress: String| {
        tracing::info!("{progress}");
        eprintln!("{}", format_progress_for_display(&progress));
    })
    .await
    .context("failed to generate test data")?;

    let profile_name = format!("Test Database (Docker:{}) ", container.port);

    let temp_store = Store::new(store_path);
    temp_store.init().context("failed to initialize store")?;
    upsert_profile(&temp_store, &profile_name, &container.connection_uri, None)?;

    log_to_user(
        &format!("Test database generated and saved as '{profile_name}'"),
        Tone::Success,
    );

    let mut profile = ConnectionProfile::new(profile_name, container.connection_uri.clone());
    profile.max_pool_size = None;
    profile.connect_timeout = None;

    Ok(profile)
}

fn log_to_user(message: &str, tone: Tone) {
    tracing::info!("{message}");
    let colored = match tone {
        Tone::Header | Tone::Success => message.green().bold().to_string(),
        Tone::Container => message.to_string(),
        Tone::Error => message.red().bold().to_string(),
    };
    eprintln!("{colored}");
}

/// Writes or updates a connection profile in the store.
pub(crate) fn upsert_profile(
    store: &Store,
    name: &str,
    uri: &str,
    _secret_handle: Option<()>,
) -> Result<()> {
    let profile = NewConnectionProfile::new(name.to_string(), uri.to_string());
    store
        .connection_profiles()
        .upsert(&profile)
        .context("failed to persist connection profile")?;
    Ok(())
}

/// Pretty-prints table progress lines emitted by the generator.
pub(super) fn format_table_progress(message: &str) -> Option<String> {
    let marker = ": table ";
    let idx = message.find(marker)?;
    let schema_prefix = &message[..idx];
    let mut parts = message[idx + marker.len()..].split('/');
    let count = parts.next()?;
    let total = parts.next()?;
    let width = total.len().max(count.len());
    let padded_count = format!("{count:>width$}");
    let colored_schema = format!("{schema_prefix}:").cyan();
    let colored_tail = format!(" table {}/{}", padded_count.yellow(), total);
    Some(format!("{colored_schema}{colored_tail}"))
}

/// Formats generator progress for terminal output.
pub(super) fn format_progress_for_display(message: &str) -> String {
    if let Some(formatted) = format_table_progress(message) {
        return formatted;
    }

    if message.starts_with("Creating schema ") {
        return message.bold().to_string();
    }

    message.to_string()
}
