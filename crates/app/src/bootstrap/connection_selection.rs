use std::{io::IsTerminal, time::Duration};

use anyhow::{bail, Result};
use poqi_config::AppConfig;
use poqi_db::ConnectionProfile;
use poqi_store::{NewConnectionProfile, Store};

use crate::cli::Cli;

pub struct ConnectionChoice {
    pub profile: ConnectionProfile,
    pub save: bool,
    pub source: &'static str,
    pub editing: bool,
    pub draft: Option<Box<super::start_menu::ProfileForm>>,
}

impl ConnectionChoice {
    pub fn new(profile: ConnectionProfile, save: bool, source: &'static str) -> Self {
        Self {
            profile,
            save,
            source,
            editing: false,
            draft: None,
        }
    }

    pub fn saved(profile: ConnectionProfile, source: &'static str) -> Self {
        Self {
            editing: true,
            ..Self::new(profile, false, source)
        }
    }

    pub fn persist_after_success(&mut self, store: &Store) -> Result<()> {
        if self.save {
            let profile = NewConnectionProfile::new(&self.profile.name, &self.profile.uri);
            let repo = store.connection_profiles();
            if self.editing {
                repo.upsert(&profile)?;
            } else {
                repo.insert(&profile)?;
            }
            self.save = false;
            self.editing = true;
        }
        Ok(())
    }
}

pub fn resolve_connection_profile(
    cli: &Cli,
    config: &AppConfig,
    store: &Store,
    menu_tick_rate: Duration,
) -> Result<ConnectionChoice> {
    if let Some(choice) = resolve_direct_connection(cli, config, store, std::env::var)? {
        return Ok(choice);
    }
    if cli.connection.check_connection {
        bail!("No connection selected. Use --url URL, --profile NAME, or POQI_DATABASE_URL. Run --list-profiles to see saved names.");
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("The connection form needs an interactive terminal. Use --check-connection with --url URL or --profile NAME for a headless login test.");
    }
    super::select_or_create_profile(store, None, false, menu_tick_rate)
}

fn resolve_direct_connection(
    cli: &Cli,
    config: &AppConfig,
    store: &Store,
    lookup: impl FnMut(&'static str) -> std::result::Result<String, std::env::VarError>,
) -> Result<Option<ConnectionChoice>> {
    if !cli.connection.check_connection {
        return Ok(None);
    }
    if let Some(uri) = cli
        .connection
        .url
        .as_ref()
        .or(cli.connection.database_url.as_ref())
    {
        return choice_from_url(uri, "command line").map(Some);
    }
    if let Some(name) = &cli.connection.profile {
        let Some(profile) = store.connection_profiles().get(name)? else {
            bail!("Saved profile not found. Use --list-profiles to see available names, or run poqi to create one.");
        };
        return Ok(Some(ConnectionChoice::saved(
            ConnectionProfile::new(profile.name, profile.uri),
            "--profile",
        )));
    }
    if let Some((name, value)) = database_url_from_lookup(lookup)? {
        return choice_from_url(&value, name).map(Some);
    }
    Ok(config.db.primary.as_ref().map(|primary| {
        let mut profile = ConnectionProfile::new(
            primary.name.as_deref().unwrap_or("primary"),
            primary.uri.trim(),
        );
        profile.max_pool_size = primary.max_pool_size;
        profile.connect_timeout = primary.connect_timeout;
        ConnectionChoice::new(profile, false, "saved primary")
    }))
}

fn choice_from_url(uri: &str, source: &'static str) -> Result<ConnectionChoice> {
    if uri.trim().is_empty() {
        bail!("{source} contains an empty database URL. Supply a URL or remove the override to use saved profiles.");
    }
    Ok(ConnectionChoice::new(
        ConnectionProfile::new("primary", uri.trim()),
        false,
        source,
    ))
}

pub fn database_url_from_lookup(
    mut lookup: impl FnMut(&'static str) -> std::result::Result<String, std::env::VarError>,
) -> Result<Option<(&'static str, String)>> {
    let name = "POQI_DATABASE_URL";
    match lookup(name) {
        Ok(value) => Ok(Some((name, value))),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => bail!("failed to read {name}: value is not valid Unicode"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use poqi_config::ConnectionConfig;

    #[test]
    fn explicit_url_and_environment_override_saved_primary() {
        let mut config = AppConfig::default();
        config.db.primary = Some(ConnectionConfig {
            uri: "postgres://saved".into(),
            ..Default::default()
        });
        let store = Store::default();
        for (args, expected, source) in [
            (
                vec!["poqi", "--check-connection", "--url", "postgres://explicit"],
                "postgres://explicit",
                "command line",
            ),
            (
                vec!["poqi", "--check-connection"],
                "postgres://environment",
                "POQI_DATABASE_URL",
            ),
        ] {
            let choice = resolve_direct_connection(&Cli::parse_from(args), &config, &store, |_| {
                Ok("postgres://environment".into())
            })
            .unwrap()
            .unwrap();
            assert_eq!(choice.profile.uri, expected);
            assert_eq!(choice.source, source);
            assert!(!choice.save);
        }
    }

    #[test]
    fn primary_preserves_pool_options_when_environment_absent() {
        let mut config = AppConfig::default();
        config.db.primary = Some(ConnectionConfig {
            name: Some("custom".into()),
            uri: "postgres://saved".into(),
            max_pool_size: Some(42),
            connect_timeout: Some(Duration::from_secs(5)),
        });
        let choice = resolve_direct_connection(
            &Cli::parse_from(["poqi", "--check-connection"]),
            &config,
            &Store::default(),
            |_| Err(std::env::VarError::NotPresent),
        )
        .unwrap()
        .unwrap();
        assert_eq!(choice.profile.name, "custom");
        assert_eq!(choice.profile.max_pool_size, Some(42));
        assert_eq!(choice.profile.connect_timeout, Some(Duration::from_secs(5)));
    }

    #[test]
    fn normal_startup_skips_environment_and_primary() {
        let mut config = AppConfig::default();
        config.db.primary = Some(ConnectionConfig {
            uri: "postgres://saved".into(),
            ..Default::default()
        });
        let choice = resolve_direct_connection(
            &Cli::parse_from(["poqi"]),
            &config,
            &Store::default(),
            |_| panic!("menu must not read environment"),
        )
        .unwrap();
        assert!(choice.is_none());
    }

    #[test]
    fn empty_environment_does_not_fall_back() {
        let result = resolve_direct_connection(
            &Cli::parse_from(["poqi", "--check-connection"]),
            &AppConfig::default(),
            &Store::default(),
            |_| Ok(" ".into()),
        );
        assert!(result.is_err());
    }
}
