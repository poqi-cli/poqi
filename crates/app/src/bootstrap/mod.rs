use std::{cmp::Ordering, collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use poqi_catalog::{schema_table_map, Catalog, CatalogSnapshot};
use poqi_db::{ConnectionProfile, Database};
use poqi_store::Store;

mod connection_selection;
mod loading_screen;
mod start_menu;
mod test_data;

pub use connection_selection::{
    database_url_from_lookup, resolve_connection_profile, ConnectionChoice,
};

#[derive(Debug)]
pub struct UserExit;

impl std::fmt::Display for UserExit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "user requested exit")
    }
}

impl std::error::Error for UserExit {}

#[derive(Debug)]
struct SelectionCancelled;

impl std::fmt::Display for SelectionCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selection cancelled")
    }
}

impl std::error::Error for SelectionCancelled {}

#[derive(Debug)]
struct SelectionQuit;

impl std::fmt::Display for SelectionQuit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selection quit")
    }
}

impl std::error::Error for SelectionQuit {}

pub fn init_store() -> Result<Store> {
    let store = Store::default();
    store.init()?;
    Ok(store)
}

pub fn handle_connection_failure(
    message: &str,
    choice: &ConnectionChoice,
    store: &Store,
    menu_tick_rate: Duration,
) -> Result<ConnectionChoice> {
    select_or_create_profile_with_recovery(
        store,
        Some(&choice.profile),
        true,
        menu_tick_rate,
        Some(start_menu::ProfileRecovery {
            profile: &choice.profile,
            message,
            editing: choice.editing,
            draft: choice.draft.as_deref(),
        }),
    )
}

pub async fn connect_and_hydrate_with_loading(
    profile: &ConnectionProfile,
    spinner_tick_rate: Duration,
    timeout: Duration,
    source: &str,
) -> Result<(Database, HashMap<String, Vec<String>>, CatalogSnapshot)> {
    let mut config = loading_screen::LoadingScreenConfig::new(
        format!("Connecting to {} ({source})", profile.name),
        "Dialing PostgreSQL...",
    )
    .with_success_detail("Launching poqi UI...".to_string());
    config.failure_detail = Some("Connection failed. Opening connection settings...".into());

    let profile_owned = profile.clone();

    loading_screen::run_loading_screen(config, spinner_tick_rate, move |handle| {
        let profile = profile_owned;
        async move {
            tokio::time::timeout(timeout, async {
                handle.set_detail(connection_detail(&profile));
                let database = Database::new();
                database
                    .connect(&profile)
                    .await
                    .with_context(|| format!("failed to connect using profile {}", profile.name))?;
                tracing::info!(profile = %profile.name, "connected to database");

                handle.set_detail("Hydrating catalog...");
                let (schemas, catalog_snapshot) = hydrate_catalog(&database).await?;

                Ok((database, schemas, catalog_snapshot))
            })
            .await
            .context("connection setup timed out")?
        }
    })
    .await
}

fn connection_detail(profile: &ConnectionProfile) -> String {
    format!(
        "Dialing {}...",
        crate::diagnostics::redact_uri(&profile.uri)
    )
}

async fn hydrate_catalog(
    database: &Database,
) -> Result<(HashMap<String, Vec<String>>, CatalogSnapshot)> {
    let mut catalog = Catalog::new(database.clone());
    catalog
        .refresh()
        .await
        .context("failed to load schema catalog")?;
    let snapshot = catalog.snapshot();

    let schemas = schema_table_map(&snapshot);
    Ok((schemas, snapshot))
}

pub fn select_or_create_profile(
    store: &Store,
    current: Option<&ConnectionProfile>,
    from_main_ui: bool,
    menu_tick_rate: Duration,
) -> Result<ConnectionChoice> {
    select_or_create_profile_with_recovery(store, current, from_main_ui, menu_tick_rate, None)
}

fn select_or_create_profile_with_recovery(
    store: &Store,
    current: Option<&ConnectionProfile>,
    from_main_ui: bool,
    menu_tick_rate: Duration,
    recovery: Option<start_menu::ProfileRecovery<'_>>,
) -> Result<ConnectionChoice> {
    let repo = store.connection_profiles();
    let mut profiles = match repo.list() {
        Ok(profiles) => profiles,
        Err(_) if recovery.is_some() => Vec::new(),
        Err(error) => return Err(error),
    };

    profiles.sort_by(|a, b| match b.updated_at.cmp(&a.updated_at) {
        Ordering::Equal => a.name.cmp(&b.name),
        ordering => ordering,
    });

    let initial = current
        .and_then(|profile| {
            profiles
                .iter()
                .position(|stored| stored.name == profile.name)
        })
        .unwrap_or(0);

    let selection = match start_menu::select_profile_stylish(
        store.clone(),
        profiles,
        initial,
        from_main_ui,
        menu_tick_rate,
        recovery,
    ) {
        Ok(sel) => sel,
        Err(err) => {
            if err.is::<SelectionQuit>() {
                return Err(UserExit.into());
            }
            if err.is::<SelectionCancelled>() {
                if let Some(profile) = current {
                    return Ok(ConnectionChoice::new(
                        profile.clone(),
                        false,
                        "current connection",
                    ));
                }
                return Err(UserExit.into());
            }
            return Err(err);
        }
    };

    match selection {
        start_menu::ProfileSelectionResult::CreateNew { name, uri, draft } => {
            let mut choice =
                ConnectionChoice::new(ConnectionProfile::new(name, uri), true, "connection form");
            choice.editing = draft.is_editing();
            choice.draft = Some(draft);
            Ok(choice)
        }
        start_menu::ProfileSelectionResult::Selected { profile: chosen } => {
            let profile = ConnectionProfile::new(chosen.name, chosen.uri);
            Ok(ConnectionChoice::saved(profile, "saved profile"))
        }
        start_menu::ProfileSelectionResult::GenerateTestData => {
            test_data::generate_and_save_test_database(store)
                .map(|profile| ConnectionChoice::saved(profile, "Docker demo"))
        }
    }
}

#[cfg(test)]
mod tests;
