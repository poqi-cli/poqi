use crossterm::event::KeyModifiers;
use poqi_catalog::QualifiedRelation;
use poqi_engine::RowIdentity;

use super::ResultsState;

#[derive(Debug, Clone)]
pub(crate) struct EditSession {
    pub(crate) row: usize,
    pub(crate) column: usize,
    buffer: String,
    original_value: Option<String>,
    is_null: bool,
}

impl EditSession {
    fn new(row: usize, column: usize, initial: String, is_null: bool) -> Self {
        let original_value = (!is_null).then(|| initial.clone());
        Self {
            row,
            column,
            buffer: initial,
            original_value,
            is_null,
        }
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    pub(crate) fn is_null(&self) -> bool {
        self.is_null
    }

    pub(crate) fn insert_char(&mut self, ch: char) {
        if self.is_null {
            self.buffer.clear();
        }
        self.is_null = false;
        self.buffer.push(ch);
    }

    pub(crate) fn backspace(&mut self) {
        if self.is_null {
            self.buffer.clear();
        }
        self.is_null = false;
        self.buffer.pop();
    }

    fn set_null(&mut self) {
        self.buffer.clear();
        self.buffer.push_str("NULL");
        self.is_null = true;
    }

    fn value(&self) -> Option<String> {
        (!self.is_null).then(|| self.buffer.clone())
    }

    fn is_unchanged(&self) -> bool {
        self.value() == self.original_value
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingEdit {
    pub row: usize,
    pub column: usize,
    pub column_name: String,
    pub row_identity: RowIdentity,
    pub new_value: Option<String>,
    pub column_type: Option<String>,
}

impl ResultsState {
    pub(crate) fn current_value(&self) -> Option<&str> {
        if let Some(session) = &self.edit_session {
            if (session.row, session.column) == self.focus_cell {
                return Some(session.buffer());
            }
        }
        let (row, col) = self.focus_cell;
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .map(String::as_str)
    }

    pub(crate) fn can_edit(&self) -> bool {
        self.source_table.is_some()
            && !self.rows.is_empty()
            && self.row_identities.len() == self.rows.len()
            && self.row_identities.iter().all(Option::is_some)
            && self.source_columns.len() == self.headers.len()
            && self.source_columns.iter().all(Option::is_some)
            && self.column_types.len() == self.headers.len()
    }

    pub(crate) fn begin_edit(&mut self) -> bool {
        if self.headers.is_empty() || !self.can_edit() {
            return false;
        }
        if self.edit_session.is_some() {
            return true;
        }
        let (row, col) = self.focus_cell;
        let value = self
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default();
        let is_null = self
            .null_cells
            .get(row)
            .and_then(|values| values.get(col))
            .copied()
            .unwrap_or(false);
        self.edit_session = Some(EditSession::new(row, col, value, is_null));
        true
    }

    pub(crate) fn edit_session(&self) -> Option<&EditSession> {
        self.edit_session.as_ref()
    }

    pub(crate) fn edit_session_mut(&mut self) -> Option<&mut EditSession> {
        self.edit_session.as_mut()
    }

    pub(crate) fn cancel_edit(&mut self) {
        self.edit_session = None;
    }

    pub(crate) fn set_edit_value_null(&mut self) -> bool {
        let Some(session) = self.edit_session.as_mut() else {
            return false;
        };
        session.set_null();
        true
    }

    pub(crate) fn finish_edit(&mut self) -> Option<EditCompletion> {
        let session = self.edit_session.take()?;
        if session.is_unchanged() {
            return Some(EditCompletion::Unchanged);
        }
        let column_name = self.source_columns.get(session.column)?.clone()?;
        let row_identity = self.row_identities.get(session.row)?.clone()?;
        Some(EditCompletion::Changed(PendingEdit {
            row: session.row,
            column: session.column,
            column_name,
            row_identity,
            new_value: session.value(),
            column_type: self.column_types.get(session.column).cloned(),
        }))
    }

    pub(crate) fn focused_row_identity(&self) -> Option<(&QualifiedRelation, &RowIdentity)> {
        let table = self.source_table.as_ref()?;
        let row = self.focus_cell.0;
        let identity = self.row_identities.get(row)?.as_ref()?;
        Some((table, identity))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum EditCompletion {
    Unchanged,
    Changed(PendingEdit),
}

pub(crate) fn plain_text_modifiers(mods: KeyModifiers) -> bool {
    mods.is_empty() || mods == KeyModifiers::SHIFT
}
