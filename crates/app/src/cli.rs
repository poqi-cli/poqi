use clap::{Args, Parser, ValueEnum};
use poqi_config::AppConfig;

/// Command-line surface for the poqi bootstrap binary.
#[derive(Parser, Debug)]
#[command(
    name = "poqi",
    version,
    about = "PostgreSQL Query Interface — browse and query PostgreSQL in your terminal",
    after_help = "Run poqi to choose or create a connection. Semantic search is off until enabled in Settings.\n\nLogin checks:\n  poqi --check-connection --profile local\n  poqi --check-connection --url 'postgresql://user@localhost:5432/app'\n\nLogin checks use explicit URL/profile > POQI_DATABASE_URL > saved primary.\nURLs passed as arguments may appear in shell history/process listings."
)]
pub struct Cli {
    #[command(flatten)]
    pub connection: ConnectionArgs,
    /// Print the resolved configuration with connection passwords hidden, then exit
    #[arg(long, conflicts_with_all = ["doctor", "list_profiles", "profile", "url", "database_url"])]
    pub show_config: bool,
    /// Print local install/config readiness (does not test database login)
    #[arg(long, conflicts_with_all = ["list_profiles", "profile", "url", "database_url"])]
    pub doctor: bool,
    /// Override the active keymap profile
    #[arg(long, value_enum)]
    pub keymap: Option<KeymapCliProfile>,
}

#[derive(Args, Debug)]
pub struct ConnectionArgs {
    /// `PostgreSQL` URL for --check-connection (alternative to --url)
    #[arg(value_name = "URL", requires = "check_connection", conflicts_with_all = ["url", "profile", "list_profiles"])]
    pub database_url: Option<String>,
    /// Test this URL with --check-connection
    #[arg(long, value_name = "URL", requires = "check_connection", conflicts_with_all = ["profile", "list_profiles"])]
    pub url: Option<String>,
    /// Test a saved profile with --check-connection
    #[arg(
        long,
        value_name = "NAME",
        requires = "check_connection",
        conflicts_with = "list_profiles"
    )]
    pub profile: Option<String>,
    /// List saved profile names and redacted URLs, then exit
    #[arg(long, conflicts_with = "check_connection")]
    pub list_profiles: bool,
    /// Test login and SELECT 1 without opening the TUI; exits nonzero on failure
    #[arg(long, conflicts_with_all = ["doctor", "show_config"])]
    pub check_connection: bool,
    /// Maximum seconds for connection setup (including catalog loading in the TUI)
    #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..=300))]
    pub connect_timeout: u64,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum KeymapCliProfile {
    Default,
}

/// Apply all CLI-provided overrides onto the mutable config.
pub fn apply_cli_overrides(cli: &Cli, config: &mut AppConfig) {
    if let Some(profile) = cli.keymap {
        config.keymap.profile = match profile {
            KeymapCliProfile::Default => "default".to_string(),
        };
    }
}
