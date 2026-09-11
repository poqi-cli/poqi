#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod keymap;
mod persistence;
mod types;

pub use crate::keymap::{
    KeyBinding, KeyCodeSpec, KeyCombination, KeyModifiers, KeySequence, KeymapError, ResolvedKeymap,
};
pub use crate::types::*;
pub use poqi_store::SemanticRuntimePreference;

#[cfg(test)]
mod tests;
