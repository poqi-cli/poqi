use crate::input::FocusPanel;

use super::App;

impl App {
    pub(super) fn handle_semantic_action(&mut self) {
        if self.focus != FocusPanel::SemanticSearch {
            self.status
                .info("Focus the Semantic Search panel to run it");
            return;
        }
        self.trigger_semantic_search();
    }
}
