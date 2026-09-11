use arboard::{Clipboard, Error as ClipboardError};

use crate::input::FocusPanel;

use super::App;

impl App {
    /// Run a clipboard operation while centralising error reporting.
    fn with_clipboard<F>(&mut self, err_context: &'static str, action: F) -> bool
    where
        F: FnOnce(&mut Clipboard, &mut Self) -> Result<bool, ClipboardError>,
    {
        match Clipboard::new() {
            Ok(mut clipboard) => match action(&mut clipboard, self) {
                Ok(result) => result,
                Err(err) => {
                    self.status.warning(format!("{err_context}: {err}"));
                    false
                }
            },
            Err(err) => {
                self.status.warning(format!("Clipboard unavailable: {err}"));
                false
            }
        }
    }

    pub(super) fn paste_from_clipboard(&mut self) -> bool {
        self.with_clipboard("Clipboard read failed", |clipboard, app| {
            let text = clipboard.get_text()?;
            if text.is_empty() {
                app.status.info("Clipboard is empty");
                return Ok(false);
            }
            match app.focus {
                FocusPanel::Editor => {
                    app.editor.insert_text(&text);
                    app.refresh_editor_post_edit();
                    Ok(true)
                }
                FocusPanel::SemanticSearch => {
                    app.semantic.insert_text(&text);
                    app.refresh_semantic_post_edit();
                    Ok(true)
                }
                _ => {
                    app.status.info("Paste is only available in text panels");
                    Ok(false)
                }
            }
        })
    }

    pub(super) fn copy_editor_buffer(&mut self) -> bool {
        let payload = if self.focus == FocusPanel::SemanticSearch {
            self.semantic_copy_payload()
        } else {
            self.editor_copy_payload()
        };
        let Some((text, message)) = payload else {
            self.status.info("Nothing to copy");
            return false;
        };
        self.with_clipboard("Clipboard write failed", move |clipboard, app| {
            clipboard.set_text(text)?;
            app.status.success(message);
            Ok(true)
        })
    }

    pub(crate) fn editor_copy_payload(&self) -> Option<(String, &'static str)> {
        if let Some(selection) = self.editor.selected_text() {
            return Some((selection, "Copied selection to clipboard"));
        }
        let text = self.editor.text();
        if text.is_empty() {
            None
        } else {
            Some((text, "Copied editor text to clipboard"))
        }
    }

    pub(crate) fn semantic_copy_payload(&self) -> Option<(String, &'static str)> {
        if let Some(selection) = self.semantic.selected_text() {
            return Some((selection, "Copied selection to clipboard"));
        }
        let text = self.semantic.query();
        if text.trim().is_empty() {
            None
        } else {
            Some((text, "Copied semantic query to clipboard"))
        }
    }
}
