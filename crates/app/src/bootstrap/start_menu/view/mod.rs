mod profile_creation;
mod profile_selection;
mod test_data;
mod util;

use ratatui::{
    style::{Color, Style},
    widgets::Block,
};

use super::state::{App, UiState};
use profile_creation::draw_profile_creation;
use profile_selection::draw_profile_selection;
use test_data::draw_test_data_generation;

pub(super) type Frame<'a> = ratatui::Frame<'a>;

impl App {
    pub(super) fn draw(&mut self, frame: &mut Frame<'_>) {
        render_background(frame);

        // Clone to avoid holding an immutable borrow while rendering mutably.
        match self.state.clone() {
            UiState::ProfileSelection => draw_profile_selection(self, frame),
            UiState::ProfileCreation(form) => draw_profile_creation(self, frame, &form),
            UiState::TestDataGeneration { progress, error } => {
                draw_test_data_generation(frame, progress.as_deref(), error.as_deref());
            }
        }
    }
}

fn render_background(frame: &mut Frame<'_>) {
    let bg = Block::default().style(Style::default().bg(Color::Rgb(25, 25, 30)));
    frame.render_widget(bg, frame.area());
}
