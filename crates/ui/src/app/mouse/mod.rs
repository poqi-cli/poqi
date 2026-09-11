//! Mouse orchestration for the main `App`.
//!
//! Delegates to specialised submodules so each concern stays focused.

mod clicks;
mod drag;
mod hit_test;
mod scroll;
mod settings;

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::{App, UiLayer};

impl App {
    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if matches!(self.layer, UiLayer::Settings) {
            self.handle_settings_mouse(mouse);
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight => {
                self.handle_scroll(mouse);
            }
            MouseEventKind::Down(MouseButton::Left) => self.handle_mouse_click(mouse),
            MouseEventKind::Up(MouseButton::Left) => {
                self.clear_editor_drag_anchor();
                self.clear_semantic_drag_anchor();
                self.scroll_drag = None;
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.handle_mouse_drag(mouse);
            }
            _ => {}
        }
    }
}
