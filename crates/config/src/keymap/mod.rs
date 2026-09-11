use std::collections::HashMap;

use bitflags::bitflags;
use thiserror::Error;

use crate::{KeymapConfig, KeymapProfile};

bitflags! {
    /// Normalized modifier flags parsed from key specifications.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0001;
        const CONTROL = 0b0010;
        const ALT = 0b0100;
        const SUPER = 0b1000;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCodeSpec {
    Char(char),
    Function(u8),
    Enter,
    Escape,
    CapsLock,
    Backspace,
    Tab,
    Space,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Delete,
    Insert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombination {
    pub code: KeyCodeSpec,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeySequence {
    pub combos: Vec<KeyCombination>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub action: String,
    pub sequence: KeySequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKeymap {
    pub contexts: HashMap<String, Vec<KeyBinding>>,
}

#[derive(Debug, Error)]
pub enum KeymapError {
    #[error("keymap profile '{0}' not found")]
    UnknownProfile(String),
    #[error("keymap profile '{0}' has no contexts")]
    EmptyProfile(String),
    #[error("invalid key spec '{spec}' for action '{action}' in context '{context}': {source}")]
    InvalidKeySpec {
        context: String,
        action: String,
        spec: String,
        #[source]
        source: KeySpecError,
    },
}

#[derive(Debug, Error)]
pub enum KeySpecError {
    #[error("empty key sequence")]
    EmptySequence,
    #[error("missing key code in '{0}'")]
    MissingKey(String),
    #[error("duplicate modifier '{modifier}' in '{spec}'")]
    DuplicateModifier {
        modifier: &'static str,
        spec: String,
    },
    #[error("unknown modifier '{modifier}' in '{spec}'")]
    UnknownModifier { modifier: String, spec: String },
    #[error("unknown key token '{0}'")]
    UnknownKey(String),
    #[error("function key out of range '{0}'")]
    FunctionOutOfRange(u8),
}

type Result<T> = std::result::Result<T, KeySpecError>;

impl KeySequence {
    fn parse(spec: &str) -> Result<Self> {
        let mut combos = Vec::new();
        let raw_parts: Vec<&str> = spec.split(',').collect();
        let mut idx = 0;
        while idx < raw_parts.len() {
            let part = raw_parts[idx].trim();
            if part.is_empty() {
                idx += 1;
                continue;
            }
            let mut combined = part.to_string();
            if combined.ends_with('+') {
                let mut lookahead = idx + 1;
                while lookahead < raw_parts.len() && raw_parts[lookahead].trim().is_empty() {
                    combined.push(',');
                    idx = lookahead;
                    lookahead += 1;
                }
            }
            combos.push(KeyCombination::parse(&combined)?);
            idx += 1;
        }
        if combos.is_empty() {
            return Err(KeySpecError::EmptySequence);
        }
        Ok(Self { combos })
    }
}

impl KeyCombination {
    fn parse(spec: &str) -> Result<Self> {
        let mut modifiers = KeyModifiers::empty();
        let mut key: Option<KeyCodeSpec> = None;
        for token in spec.split('+') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            let lowered = token.to_ascii_lowercase();
            match lowered.as_str() {
                "shift" => insert_modifier(&mut modifiers, KeyModifiers::SHIFT, "shift", spec)?,
                "ctrl" | "control" => {
                    insert_modifier(&mut modifiers, KeyModifiers::CONTROL, "ctrl", spec)?;
                }
                "alt" | "option" => {
                    insert_modifier(&mut modifiers, KeyModifiers::ALT, "alt", spec)?;
                }
                "super" | "meta" | "command" | "cmd" => {
                    insert_modifier(&mut modifiers, KeyModifiers::SUPER, "super", spec)?;
                }
                _ => {
                    if key.is_some() {
                        return Err(KeySpecError::UnknownModifier {
                            modifier: token.to_string(),
                            spec: spec.to_string(),
                        });
                    }
                    key = Some(KeyCodeSpec::from_token(token)?);
                }
            }
        }
        let mut code = key.ok_or_else(|| KeySpecError::MissingKey(spec.to_string()))?;
        if modifiers.contains(KeyModifiers::SHIFT) {
            if let KeyCodeSpec::Char(ch) = &mut code {
                if ch.is_ascii_alphabetic() {
                    *ch = ch.to_ascii_uppercase();
                }
            }
        }
        Ok(Self { code, modifiers })
    }
}

impl KeyCodeSpec {
    fn from_token(token: &str) -> Result<Self> {
        let normalized = token.trim();
        if normalized.is_empty() {
            return Err(KeySpecError::MissingKey(token.to_string()));
        }
        let lower = normalized.to_ascii_lowercase();
        match lower.as_str() {
            "enter" | "return" => Ok(Self::Enter),
            "esc" | "escape" => Ok(Self::Escape),
            "backspace" => Ok(Self::Backspace),
            "tab" => Ok(Self::Tab),
            "space" => Ok(Self::Space),
            "capslock" | "caps_lock" => Ok(Self::CapsLock),
            "up" | "arrow_up" => Ok(Self::Up),
            "down" | "arrow_down" => Ok(Self::Down),
            "left" | "arrow_left" => Ok(Self::Left),
            "right" | "arrow_right" => Ok(Self::Right),
            "pageup" | "page_up" | "pgup" => Ok(Self::PageUp),
            "pagedown" | "page_down" | "pgdn" => Ok(Self::PageDown),
            "home" => Ok(Self::Home),
            "end" => Ok(Self::End),
            "delete" | "del" => Ok(Self::Delete),
            "insert" | "ins" => Ok(Self::Insert),
            _ => {
                if let Some(stripped) = lower.strip_prefix('f') {
                    if !stripped.is_empty() {
                        let value: u8 = stripped
                            .parse()
                            .map_err(|_| KeySpecError::UnknownKey(token.to_string()))?;
                        if (1..=24).contains(&value) {
                            return Ok(Self::Function(value));
                        }
                        return Err(KeySpecError::FunctionOutOfRange(value));
                    }
                }
                if normalized.chars().count() == 1 {
                    let ch = normalized.chars().next().expect("char exists");
                    return Ok(Self::Char(ch));
                }
                Err(KeySpecError::UnknownKey(token.to_string()))
            }
        }
    }
}

fn insert_modifier(
    modifiers: &mut KeyModifiers,
    modifier: KeyModifiers,
    name: &'static str,
    spec: &str,
) -> Result<()> {
    if modifiers.contains(modifier) {
        return Err(KeySpecError::DuplicateModifier {
            modifier: name,
            spec: spec.to_string(),
        });
    }
    modifiers.insert(modifier);
    Ok(())
}

impl KeymapConfig {
    /// Resolve the keymap profile data for the requested profile name, or the active profile.
    ///
    /// # Errors
    /// Returns an error when the profile is unknown, empty, or contains invalid key specifications.
    pub fn resolve_profile(
        &self,
        profile: Option<&str>,
    ) -> std::result::Result<ResolvedKeymap, KeymapError> {
        let profile_name = profile.unwrap_or(&self.profile);
        let data = self
            .profiles
            .get(profile_name)
            .ok_or_else(|| KeymapError::UnknownProfile(profile_name.to_string()))?;
        if data.contexts.is_empty() {
            return Err(KeymapError::EmptyProfile(profile_name.to_string()));
        }
        let mut contexts = HashMap::new();
        for (context, actions) in &data.contexts {
            let mut bindings = Vec::new();
            for (action, spec) in actions {
                for seq in spec.split('|') {
                    let seq = seq.trim();
                    if seq.is_empty() {
                        continue;
                    }
                    let sequence =
                        KeySequence::parse(seq).map_err(|source| KeymapError::InvalidKeySpec {
                            context: context.clone(),
                            action: action.clone(),
                            spec: seq.to_string(),
                            source,
                        })?;
                    bindings.push(KeyBinding {
                        action: action.clone(),
                        sequence,
                    });
                }
            }
            contexts.insert(context.clone(), bindings);
        }
        Ok(ResolvedKeymap { contexts })
    }

    /// Resolve the active keymap profile configured for the application.
    ///
    /// # Errors
    /// Propagates the same error conditions as [`KeymapConfig::resolve_profile`].
    pub fn active_profile(&self) -> std::result::Result<ResolvedKeymap, KeymapError> {
        self.resolve_profile(None)
    }
}

impl KeymapProfile {
    #[must_use]
    pub fn context(&self, name: &str) -> Option<&HashMap<String, String>> {
        self.contexts.get(name)
    }
}

#[cfg(test)]
mod tests;
