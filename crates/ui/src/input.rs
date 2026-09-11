/// Logical UI panels that can be targeted by keyboard focus.
///
/// The order of the variants matches how the selector traverses the screen from
/// left-to-right before dropping to the status bar, making the `next` and
/// `prev` helpers read naturally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    Schema,
    Editor,
    SemanticSearch,
    Results,
    Status,
}

impl FocusPanel {
    /// Move focus clockwise through the primary panels, wrapping back to the
    /// schema browser after the status bar.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Schema => Self::Editor,
            Self::Editor => Self::SemanticSearch,
            Self::SemanticSearch => Self::Results,
            Self::Results => Self::Status,
            Self::Status => Self::Schema,
        }
    }

    /// Move focus counter-clockwise through the primary panels, wrapping up to
    /// the status bar when retreating past the schema browser.
    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Schema => Self::Status,
            Self::Editor => Self::Schema,
            Self::SemanticSearch => Self::Editor,
            Self::Results => Self::SemanticSearch,
            Self::Status => Self::Results,
        }
    }

    /// Move horizontally across the interactive panels on the top row,
    /// stopping at the rightmost panel (Results) without wrapping.
    #[must_use]
    pub fn next_horizontal(self) -> Self {
        match self {
            Self::Schema => Self::Editor,
            Self::Editor => Self::SemanticSearch,
            Self::SemanticSearch => Self::Results,
            Self::Results | Self::Status => self,
        }
    }

    /// Move horizontally backward across the top-row panels without wrapping.
    #[must_use]
    pub fn prev_horizontal(self) -> Self {
        match self {
            Self::Schema | Self::Editor => Self::Schema,
            Self::SemanticSearch | Self::Results => Self::Editor,
            Self::Status => Self::Status,
        }
    }

    #[must_use]
    pub const fn is_top_row(self) -> bool {
        matches!(self, Self::Schema | Self::Editor | Self::SemanticSearch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    BackToProfiles,
    Move { dir: MoveDirection },
    FastMove { dir: MoveDirection, step: usize },
    PageUp,
    PageDown,
    JumpRowStart,
    JumpRowEnd,
    JumpColStart,
    JumpColEnd,
    ExtendUp,
    ExtendDown,
    ExtendLeft,
    ExtendRight,
    SelectTop,
    TableDetail,
    InsertRow,
    UpdateRow,
    DeleteRow,
    Refresh,
    Open,
    Back,
    Copy,
    Export,
    Run,
    Cancel,
    Autocomplete,
    ToggleComment,
    RunSelection,
    OpenPalette,
    OpenSettings,
    ToggleDevConsole,
    FocusNextPanel,
    FocusPrevPanel,
    Filter,
    ToggleZoom,
    SemanticSearch,
}

impl Action {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "quit" => Some(Self::Quit),
            "move_up" | "cell_up" => Some(Self::Move {
                dir: MoveDirection::Up,
            }),
            "move_down" | "cell_down" => Some(Self::Move {
                dir: MoveDirection::Down,
            }),
            "move_left" | "cell_left" => Some(Self::Move {
                dir: MoveDirection::Left,
            }),
            "move_right" | "cell_right" => Some(Self::Move {
                dir: MoveDirection::Right,
            }),
            "page_up" => Some(Self::PageUp),
            "page_down" => Some(Self::PageDown),
            "extend_up" => Some(Self::ExtendUp),
            "extend_down" => Some(Self::ExtendDown),
            "extend_left" => Some(Self::ExtendLeft),
            "extend_right" => Some(Self::ExtendRight),
            "jump_first_row" | "go_first_row" => Some(Self::JumpRowStart),
            "jump_last_row" | "go_last_row" => Some(Self::JumpRowEnd),
            "jump_first_col" | "go_first_col" => Some(Self::JumpColStart),
            "jump_last_col" | "go_last_col" => Some(Self::JumpColEnd),
            "select_top" => Some(Self::SelectTop),
            "table_detail" => Some(Self::TableDetail),
            "insert_row" => Some(Self::InsertRow),
            "update_row" => Some(Self::UpdateRow),
            "delete_row" => Some(Self::DeleteRow),
            "refresh" => Some(Self::Refresh),
            "open" => Some(Self::Open),
            "back" => Some(Self::Back),
            "copy" => Some(Self::Copy),
            "export" => Some(Self::Export),
            "run" => Some(Self::Run),
            "run_selection" => Some(Self::RunSelection),
            "cancel" => Some(Self::Cancel),
            "autocomplete" => Some(Self::Autocomplete),
            "toggle_comment" => Some(Self::ToggleComment),
            "open_palette" | "palette" => Some(Self::OpenPalette),
            "open_settings" | "settings" => Some(Self::OpenSettings),
            "toggle_dev_console" | "dev_console" => Some(Self::ToggleDevConsole),
            "focus_next_panel" | "focus_next_pane" | "next_panel" | "next_pane" => {
                Some(Self::FocusNextPanel)
            }
            "focus_prev_panel" | "focus_prev_pane" | "prev_panel" | "prev_pane" => {
                Some(Self::FocusPrevPanel)
            }
            "filter" => Some(Self::Filter),
            "toggle_zoom" | "zoom" => Some(Self::ToggleZoom),
            "semantic_search" | "semantic_run" => Some(Self::SemanticSearch),
            _ => None,
        }
    }
}

impl MoveDirection {
    #[must_use]
    pub const fn vertical_delta(self) -> isize {
        match self {
            Self::Up => -1,
            Self::Down => 1,
            Self::Left | Self::Right => 0,
        }
    }

    #[must_use]
    pub const fn horizontal_delta(self) -> isize {
        match self {
            Self::Left => -1,
            Self::Right => 1,
            Self::Up | Self::Down => 0,
        }
    }
}
