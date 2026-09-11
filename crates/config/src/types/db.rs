use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbDefaultsConfig {
    #[serde(rename = "statement_timeout_ms")]
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    #[serde(default = "default_statement_timeout")]
    pub statement_timeout: Duration,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConnectionConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub uri: String,
    #[serde(default)]
    pub max_pool_size: Option<usize>,
    #[serde(rename = "connect_timeout_ms")]
    #[serde(default)]
    #[serde_as(as = "Option<DurationMilliSeconds<u64>>")]
    pub connect_timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DbDefaultsWrapper {
    #[serde(default)]
    pub defaults: DbDefaultsConfig,
    #[serde(default)]
    pub primary: Option<ConnectionConfig>,
}

fn default_statement_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_page_size() -> u32 {
    5_000
}

impl Default for DbDefaultsConfig {
    fn default() -> Self {
        Self {
            statement_timeout: default_statement_timeout(),
            page_size: default_page_size(),
        }
    }
}
