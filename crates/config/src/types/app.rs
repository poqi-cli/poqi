use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{DbDefaultsWrapper, KeymapConfig, SearchConfig, UiConfig};

/// Root configuration container that aggregates every workspace feature toggle.
///
/// The defaults mirror the build guide:
///
/// ```
/// use poqi_config::AppConfig;
///
/// let config = AppConfig::default();
/// assert_eq!(config.ui.theme, "dark");
/// assert_eq!(config.keymap.profile, "default");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub keymap: KeymapConfig,
    #[serde(default)]
    pub db: DbDefaultsWrapper,
    #[serde(default)]
    pub search: SearchConfig,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not determine configuration directory")]
    NoConfigDir,
}
