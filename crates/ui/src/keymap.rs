use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use poqi_config::{
    AppConfig, KeyCodeSpec, KeyCombination, KeyModifiers as ConfigModifiers, ResolvedKeymap,
};
use tracing::warn;

use crate::{
    input::{Action, FocusPanel},
    UiRuntimeSettings,
};

#[derive(Debug)]
pub struct KeymapEngine {
    global: Vec<Binding>,
    schema: Vec<Binding>,
    grid: Vec<Binding>,
    editor: Vec<Binding>,
    semantic: Vec<Binding>,
    fast_scroll_step: usize,
}

#[derive(Debug, Clone)]
struct Binding {
    action: Action,
    step: KeyMatchStep,
}

#[derive(Debug, Clone, Copy)]
struct KeyMatchStep {
    code: KeyCode,
    modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyContext {
    Global,
    Schema,
    Grid,
    Editor,
    Semantic,
}

impl KeymapEngine {
    pub fn from_config(config: &AppConfig, fast_scroll_step: usize) -> Result<Self> {
        let resolved = config.keymap.active_profile().map_err(|err| anyhow!(err))?;
        Ok(Self::from_resolved(&resolved, fast_scroll_step))
    }

    fn from_resolved(resolved: &ResolvedKeymap, fast_scroll_step: usize) -> Self {
        let mut engine = Self {
            global: Vec::new(),
            schema: Vec::new(),
            grid: Vec::new(),
            editor: Vec::new(),
            semantic: Vec::new(),
            fast_scroll_step,
        };

        for (context_name, bindings) in &resolved.contexts {
            let Some(context) = KeyContext::from_name(context_name) else {
                warn!(
                    context = context_name.as_str(),
                    "ignoring unknown keymap context"
                );
                continue;
            };
            for binding in bindings {
                let Some(action) = Action::from_name(&binding.action) else {
                    warn!(
                        context = context_name.as_str(),
                        action = %binding.action,
                        "unknown keymap action"
                    );
                    continue;
                };
                if binding.sequence.combos.len() != 1 {
                    warn!(
                        context = context_name.as_str(),
                        action = %binding.action,
                        "skipping multi-step binding; chords are no longer supported"
                    );
                    continue;
                }
                let step = convert_combination(&binding.sequence.combos[0]);
                let binding = Binding { action, step };
                match context {
                    KeyContext::Global => engine.global.push(binding),
                    KeyContext::Schema => engine.schema.push(binding),
                    KeyContext::Grid => engine.grid.push(binding),
                    KeyContext::Editor => engine.editor.push(binding),
                    KeyContext::Semantic => engine.semantic.push(binding),
                }
            }
        }

        engine
    }

    pub fn resolve(&self, event: &KeyEvent, focus: FocusPanel) -> Option<Action> {
        if event.kind != KeyEventKind::Press {
            return None;
        }
        let current = KeyMatchStep::from_event(event);
        let contexts = KeyContext::for_focus(focus);
        for context in contexts {
            let bindings = self.bindings_for(context);
            for binding in bindings {
                if binding.step.matches(&current) {
                    return Some(resolve_movement(
                        binding.action,
                        &current,
                        self.fast_scroll_step,
                    ));
                }
            }
        }
        None
    }

    fn bindings_for(&self, context: KeyContext) -> &Vec<Binding> {
        match context {
            KeyContext::Global => &self.global,
            KeyContext::Schema => &self.schema,
            KeyContext::Grid => &self.grid,
            KeyContext::Editor => &self.editor,
            KeyContext::Semantic => &self.semantic,
        }
    }
}

impl Default for KeymapEngine {
    fn default() -> Self {
        Self::default_with_step(UiRuntimeSettings::default().fast_scroll_step())
    }
}

impl KeymapEngine {
    pub(crate) fn default_with_step(step: usize) -> Self {
        Self::from_config(&AppConfig::default(), step).expect("default keymap must resolve")
    }
}

impl KeyContext {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "global" => Some(Self::Global),
            "schema" => Some(Self::Schema),
            "grid" | "results" => Some(Self::Grid),
            "editor" => Some(Self::Editor),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }

    fn for_focus(focus: FocusPanel) -> Vec<Self> {
        match focus {
            FocusPanel::Schema => vec![Self::Schema, Self::Global],
            FocusPanel::Editor => vec![Self::Editor, Self::Global],
            FocusPanel::SemanticSearch => vec![Self::Semantic, Self::Global],
            FocusPanel::Status => vec![Self::Global],
            FocusPanel::Results => vec![Self::Grid, Self::Global],
        }
    }
}

impl KeyMatchStep {
    fn from_event(event: &KeyEvent) -> Self {
        match event.code {
            KeyCode::BackTab => Self {
                code: KeyCode::Tab,
                modifiers: event.modifiers | KeyModifiers::SHIFT,
            },
            _ => Self {
                code: event.code,
                modifiers: event.modifiers,
            },
        }
    }

    fn matches(&self, other: &Self) -> bool {
        self.code == other.code && self.modifiers == other.modifiers
    }
}

fn resolve_movement(action: Action, current: &KeyMatchStep, fast_scroll_step: usize) -> Action {
    if let Action::Move { dir } = action {
        if current.modifiers == KeyModifiers::SHIFT {
            return Action::FastMove {
                dir,
                step: fast_scroll_step,
            };
        }
    }
    action
}

fn convert_combination(combo: &KeyCombination) -> KeyMatchStep {
    let code = convert_code(&combo.code);
    let modifiers = convert_modifiers(combo.modifiers);
    KeyMatchStep { code, modifiers }
}

fn convert_code(code: &KeyCodeSpec) -> KeyCode {
    match code {
        KeyCodeSpec::Char(ch) => KeyCode::Char(*ch),
        KeyCodeSpec::Function(n) => KeyCode::F(*n),
        KeyCodeSpec::Enter => KeyCode::Enter,
        KeyCodeSpec::Escape => KeyCode::Esc,
        KeyCodeSpec::CapsLock => KeyCode::CapsLock,
        KeyCodeSpec::Backspace => KeyCode::Backspace,
        KeyCodeSpec::Tab => KeyCode::Tab,
        KeyCodeSpec::Space => KeyCode::Char(' '),
        KeyCodeSpec::Up => KeyCode::Up,
        KeyCodeSpec::Down => KeyCode::Down,
        KeyCodeSpec::Left => KeyCode::Left,
        KeyCodeSpec::Right => KeyCode::Right,
        KeyCodeSpec::PageUp => KeyCode::PageUp,
        KeyCodeSpec::PageDown => KeyCode::PageDown,
        KeyCodeSpec::Home => KeyCode::Home,
        KeyCodeSpec::End => KeyCode::End,
        KeyCodeSpec::Delete => KeyCode::Delete,
        KeyCodeSpec::Insert => KeyCode::Insert,
    }
}

fn convert_modifiers(mods: ConfigModifiers) -> KeyModifiers {
    let mut converted = KeyModifiers::empty();
    if mods.contains(ConfigModifiers::SHIFT) {
        converted.insert(KeyModifiers::SHIFT);
    }
    if mods.contains(ConfigModifiers::CONTROL) {
        converted.insert(KeyModifiers::CONTROL);
    }
    if mods.contains(ConfigModifiers::ALT) {
        converted.insert(KeyModifiers::ALT);
    }
    if mods.contains(ConfigModifiers::SUPER) {
        converted.insert(KeyModifiers::SUPER);
    }
    converted
}
