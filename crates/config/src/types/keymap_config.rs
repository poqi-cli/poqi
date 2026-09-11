use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeymapConfig {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(flatten)]
    #[serde(default = "default_profiles")]
    #[serde(deserialize_with = "deserialize_profiles")]
    pub profiles: HashMap<String, KeymapProfile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeymapProfile {
    #[serde(flatten)]
    #[serde(default)]
    pub contexts: HashMap<String, HashMap<String, String>>,
}

fn default_profile() -> String {
    "default".to_string()
}

fn default_profiles() -> HashMap<String, KeymapProfile> {
    HashMap::from([("default".to_string(), default_profile_bindings())])
}

fn deserialize_profiles<'de, D>(deserializer: D) -> Result<HashMap<String, KeymapProfile>, D::Error>
where
    D: Deserializer<'de>,
{
    let provided = HashMap::<String, KeymapProfile>::deserialize(deserializer)?;
    let mut merged = default_profiles();
    merged.extend(provided);
    Ok(merged)
}

type ContextBindings = &'static [(&'static str, &'static str)];

/// Bindings that are available no matter which surface the user is focused in.
const GLOBAL_CONTEXT: ContextBindings = &[
    ("back", "esc"),
    ("quit", "shift+esc"),
    ("focus_next_panel", "tab"),
    ("focus_prev_panel", "shift+tab"),
];

/// Bindings for the schema browser so navigation mirrors WASD-driven movement.
const SCHEMA_CONTEXT: ContextBindings = &[
    ("move_up", "w | arrow_up | shift+w | shift+arrow_up"),
    ("move_down", "s | arrow_down | shift+s | shift+arrow_down"),
    ("move_left", "a | arrow_left | shift+a | shift+arrow_left"),
    (
        "move_right",
        "d | arrow_right | shift+d | shift+arrow_right",
    ),
    ("table_detail", "f"),
    ("select_top", "shift+f"),
    ("refresh", "r"),
    ("open", "enter"),
];

/// Bindings for tabular grids, pairing movement and selection extensions.
const GRID_CONTEXT: ContextBindings = &[
    ("move_up", "w | arrow_up | shift+w | shift+arrow_up"),
    ("move_down", "s | arrow_down | shift+s | shift+arrow_down"),
    ("move_left", "a | arrow_left | shift+a | shift+arrow_left"),
    (
        "move_right",
        "d | arrow_right | shift+d | shift+arrow_right",
    ),
    ("refresh", "f | r"),
];

/// Bindings scoped to the SQL editor, covering execution and ide-like helpers.
const EDITOR_CONTEXT: ContextBindings = &[("run", "alt+enter | shift+enter | f5")];

/// Bindings for the semantic search panel.
const SEMANTIC_CONTEXT: ContextBindings = &[("semantic_search", "enter")];

fn default_profile_bindings() -> KeymapProfile {
    let contexts = HashMap::from([
        ("global".to_string(), context_map(GLOBAL_CONTEXT)),
        ("schema".to_string(), context_map(SCHEMA_CONTEXT)),
        ("grid".to_string(), context_map(GRID_CONTEXT)),
        ("editor".to_string(), context_map(EDITOR_CONTEXT)),
        ("semantic".to_string(), context_map(SEMANTIC_CONTEXT)),
    ]);
    KeymapProfile { contexts }
}

fn context_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(action, spec)| ((*action).to_string(), (*spec).to_string()))
        .collect()
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            profile: default_profile(),
            profiles: default_profiles(),
        }
    }
}
