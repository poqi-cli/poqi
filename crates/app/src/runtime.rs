use std::{collections::HashMap, io::IsTerminal, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;
use poqi_catalog::CatalogSnapshot;
use poqi_config::AppConfig;
use poqi_db::{Database, DatabaseError};
use poqi_engine::{DatabaseEngine, Engine};
use poqi_observability::init_tracing;

use crate::{
    bootstrap,
    cli::{apply_cli_overrides, Cli},
    diagnostics, settings,
};

type ConnectionBundle = (Database, HashMap<String, Vec<String>>, CatalogSnapshot);

async fn check_connection(choice: &bootstrap::ConnectionChoice, timeout: Duration) -> Result<()> {
    println!("Connection source: {}", choice.source);
    println!("Testing {}", diagnostics::redact_uri(&choice.profile.uri));
    let database = Database::new();
    match tokio::time::timeout(timeout, database.connect(&choice.profile)).await {
        Ok(Ok(())) => {
            println!("Connection OK (login and SELECT 1 succeeded). No profile was saved.");
            Ok(())
        }
        Ok(Err(error)) => anyhow::bail!("{}", error.connection_hint()),
        Err(_) => anyhow::bail!("Connection timed out. Check host, port and network access, or increase --connect-timeout."),
    }
}

fn connection_failure_hint(error: &anyhow::Error) -> &'static str {
    if let Some(error) = error.downcast_ref::<DatabaseError>() {
        return error.connection_hint();
    }
    if error
        .downcast_ref::<tokio::time::error::Elapsed>()
        .is_some()
    {
        return "Connection setup timed out. Check host, port and network access, or increase --connect-timeout.";
    }
    "Could not load the database browser. Check database/catalog permissions and try again."
}

/// Bootstrap tracing, load configuration, and drive the profile/connection loop.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    init_tracing()?;
    let store = bootstrap::init_store()?;
    let mut config = load_configuration(&cli, &store)?;
    let mut ui_settings = settings::load_ui_settings(&store)?;
    let semantic_coordinator = poqi_ui::SemanticBootstrapCoordinator::new();

    if print_requested_info(&cli, &config, &store)? {
        return Ok(());
    }

    let Some(mut choice) = resolve_profile(&cli, &config, &store, ui_settings.menu_tick_rate())?
    else {
        return Ok(());
    };

    if cli.connection.check_connection {
        return check_connection(&choice, Duration::from_secs(cli.connection.connect_timeout))
            .await;
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        anyhow::bail!("The database browser needs an interactive terminal. Use --check-connection to test login without the TUI.");
    }

    loop {
        let Some((database, schemas, catalog_snapshot)) = connect_with_profile_recovery(
            &mut choice,
            &store,
            ui_settings.menu_tick_rate(),
            Duration::from_secs(cli.connection.connect_timeout),
        )
        .await?
        else {
            break;
        };

        match launch_ui(
            &config,
            ui_settings,
            store.clone(),
            database,
            schemas,
            catalog_snapshot,
            &semantic_coordinator,
        )
        .await?
        {
            poqi_ui::ExitReason::Quit => break,
            poqi_ui::ExitReason::BackToProfiles => {
                config = load_configuration(&cli, &store)?;
                ui_settings = settings::load_ui_settings(&store)?;
                choice = match bootstrap::select_or_create_profile(
                    &store,
                    Some(&choice.profile),
                    true,
                    ui_settings.menu_tick_rate(),
                ) {
                    Ok(p) => p,
                    Err(err) => {
                        if err.downcast_ref::<bootstrap::UserExit>().is_some() {
                            break;
                        }
                        return Err(err);
                    }
                };
            }
        }
    }

    Ok(())
}

fn print_requested_info(cli: &Cli, config: &AppConfig, store: &poqi_store::Store) -> Result<bool> {
    if cli.doctor {
        println!("{}", diagnostics::render_doctor(config, store)?);
    } else if cli.show_config {
        let mut redacted = config.clone();
        if let Some(primary) = &mut redacted.db.primary {
            primary.uri = diagnostics::redact_uri(&primary.uri);
        }
        println!("{redacted:#?}");
    } else if cli.connection.list_profiles {
        let profiles = store.connection_profiles().list()?;
        if profiles.is_empty() {
            println!("No saved profiles. Run poqi to create one.");
        }
        for profile in profiles {
            println!(
                "{}\t{}",
                profile.name,
                diagnostics::redact_uri(&profile.uri)
            );
        }
    } else {
        return Ok(false);
    }
    Ok(true)
}

async fn connect_with_profile_recovery(
    choice: &mut bootstrap::ConnectionChoice,
    store: &poqi_store::Store,
    menu_tick_rate: std::time::Duration,
    timeout: Duration,
) -> Result<Option<ConnectionBundle>> {
    loop {
        let message = match bootstrap::connect_and_hydrate_with_loading(
            &choice.profile,
            menu_tick_rate,
            timeout,
            choice.source,
        )
        .await
        {
            Ok(bundle) => match choice.persist_after_success(store) {
                Ok(()) => return Ok(Some(bundle)),
                Err(_) => "Connected, but secure saving failed. Unlock your system credential store and check local storage permissions, then retry.",
            },
            Err(error) => connection_failure_hint(&error),
        };
        tracing::warn!(message, source = choice.source, "connection setup failed");
        match bootstrap::handle_connection_failure(message, choice, store, menu_tick_rate) {
            Ok(updated) => *choice = updated,
            Err(selection_error)
                if selection_error
                    .downcast_ref::<bootstrap::UserExit>()
                    .is_some() =>
            {
                return Ok(None);
            }
            Err(selection_error) => return Err(selection_error),
        }
    }
}

fn load_configuration(cli: &Cli, store: &poqi_store::Store) -> Result<AppConfig> {
    let mut config = AppConfig::load_or_default_with_store(store)?;
    apply_cli_overrides(cli, &mut config);
    Ok(config)
}

/// Resolve a connection profile, returning `None` when the user explicitly exits.
fn resolve_profile(
    cli: &Cli,
    config: &AppConfig,
    store: &poqi_store::Store,
    menu_tick_rate: std::time::Duration,
) -> Result<Option<bootstrap::ConnectionChoice>> {
    match bootstrap::resolve_connection_profile(cli, config, store, menu_tick_rate) {
        Ok(profile) => Ok(Some(profile)),
        Err(err) => {
            if err.downcast_ref::<bootstrap::UserExit>().is_some() {
                return Ok(None);
            }
            Err(err)
        }
    }
}

/// Spin up the UI for an established database session.
async fn launch_ui(
    config: &AppConfig,
    ui_settings: poqi_ui::UiRuntimeSettings,
    store: poqi_store::Store,
    database: Database,
    schemas: HashMap<String, Vec<String>>,
    catalog_snapshot: CatalogSnapshot,
    semantic_coordinator: &poqi_ui::SemanticBootstrapCoordinator,
) -> Result<poqi_ui::ExitReason> {
    let engine: Arc<dyn Engine> = Arc::new(DatabaseEngine::new(
        database,
        config.db.defaults.statement_timeout,
        config.db.defaults.page_size,
    ));

    poqi_ui::run_app(
        config,
        ui_settings,
        store,
        engine,
        schemas,
        catalog_snapshot,
        semantic_coordinator,
    )
    .await
    .inspect_err(|err| tracing::error!(?err, "failed to run UI"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn headless_check_times_out_when_server_never_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
            drop(socket);
        });
        let choice = bootstrap::ConnectionChoice::new(
            poqi_db::ConnectionProfile::new(
                "test",
                format!("postgres://user:timeout-secret@127.0.0.1:{port}/app?sslmode=disable"),
            ),
            false,
            "test",
        );
        let error = check_connection(&choice, Duration::from_millis(100))
            .await
            .unwrap_err();
        server.abort();
        let hint = error.to_string();
        assert!(hint.contains("timed out"));
        assert!(!hint.contains("timeout-secret"));
    }

    #[tokio::test]
    async fn headless_check_reports_invalid_url_without_disclosing_credentials() {
        let choice = bootstrap::ConnectionChoice::new(
            poqi_db::ConnectionProfile::new(
                "test",
                "postgres://user:hidden-secret@localhost:bad/app",
            ),
            false,
            "test",
        );
        let error = check_connection(&choice, Duration::from_secs(1))
            .await
            .unwrap_err();
        let hint = error.to_string();
        assert!(hint.contains("valid PostgreSQL"));
        assert!(!hint.contains("hidden-secret"));
    }
}
