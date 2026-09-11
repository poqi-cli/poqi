//! Keyboard event handling with dual-path architecture.
//!
//! ## Return Value Semantics
//! - `handle_key() -> bool` where `true` = quit app, `false` = continue
//! - Only `Action::Quit` returns `true`; all navigation returns `false`
//!
//! ## Keybinding Paths
//! 1. **Global handlers** (Shift+Esc for quit) - processed first, always available
//! 2. **`PanelSelect` mode** - hardcoded WASD/arrow navigation for panel selection
//!    - Shift modifiers enable 10x faster cycling (same as `PanelFocused` mode)
//! 3. **`PanelFocused` mode** - config-driven keymap resolution for panel content
//!    - Shift modifiers enable fast scrolling in lists/grids
//!
//! ## Adding Universal Shortcuts
//! - Global: add to `handle_key()` before mode routing
//! - Mode-specific: update both `handle_window_select_key()` AND default config

use super::{App, UiLayer};

mod clipboard;
mod editor_input;
mod helpers;
mod panel_select;
mod results_input;
mod routing;
mod semantic_input;
mod settings;

#[cfg(test)]
mod tests;
