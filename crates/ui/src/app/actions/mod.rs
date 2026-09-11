use crate::{input::Action, ExitReason};

pub(super) use super::{App, UiLayer};

mod data;
mod editor;
mod flow;
mod navigation;
mod semantic;

impl App {
    /// Route a resolved `Action` to the appropriate handler.
    pub fn handle_action(&mut self, action: Action) -> bool {
        match action {
            Action::Quit => {
                self.signal_shutdown();
                self.exit_reason = Some(ExitReason::Quit);
                true
            }
            Action::BackToProfiles => {
                self.signal_shutdown();
                self.exit_reason = Some(ExitReason::BackToProfiles);
                true
            }
            Action::ToggleZoom => {
                if self.zoomed_panel == Some(self.focus) {
                    self.zoomed_panel = None;
                } else {
                    self.zoomed_panel = Some(self.focus);
                }
                false
            }
            Action::Move { dir } => {
                self.apply_move(dir, 1, false);
                false
            }
            Action::FastMove { dir, step } => {
                self.apply_move(dir, step, false);
                false
            }
            Action::FocusNextPanel
            | Action::FocusPrevPanel
            | Action::ExtendUp
            | Action::ExtendDown
            | Action::ExtendLeft
            | Action::ExtendRight
            | Action::PageUp
            | Action::PageDown
            | Action::JumpRowStart
            | Action::JumpRowEnd
            | Action::JumpColStart
            | Action::JumpColEnd => {
                self.handle_navigation_action(action);
                false
            }
            Action::SelectTop
            | Action::InsertRow
            | Action::UpdateRow
            | Action::DeleteRow
            | Action::Refresh => {
                self.handle_data_action(action);
                false
            }
            Action::Open | Action::Back | Action::TableDetail => {
                self.handle_flow_action(action);
                false
            }
            Action::Copy | Action::Export | Action::Run | Action::Cancel => {
                self.handle_editor_action(action);
                false
            }
            Action::SemanticSearch => {
                self.handle_semantic_action();
                false
            }
            Action::OpenSettings => {
                self.open_settings_modal();
                false
            }
            Action::Autocomplete
            | Action::ToggleComment
            | Action::RunSelection
            | Action::OpenPalette
            | Action::ToggleDevConsole
            | Action::Filter => {
                // Intentionally ignored: these actions are not implemented yet.
                false
            }
        }
    }
}
