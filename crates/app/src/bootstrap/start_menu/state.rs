use poqi_db::ConnectionProfile;
use poqi_store::{Store, StoredConnectionProfile};
use ratatui::layout::Rect;

use super::form::ProfileForm;

#[derive(Debug, Clone)]
pub enum ProfileSelectionResult {
    Selected {
        profile: StoredConnectionProfile,
    },
    CreateNew {
        name: String,
        uri: String,
        draft: Box<ProfileForm>,
    },
    GenerateTestData,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProfileRecovery<'a> {
    pub profile: &'a ConnectionProfile,
    pub message: &'a str,
    pub editing: bool,
    pub draft: Option<&'a ProfileForm>,
}

#[derive(Debug, Clone)]
pub(super) enum StatusKind {
    Info,
    Error,
}

#[derive(Debug, Clone)]
pub(super) struct SelectionStatus {
    pub(super) message: String,
    pub(super) kind: StatusKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UiState {
    ProfileSelection,
    ProfileCreation(Box<ProfileForm>),
    TestDataGeneration {
        progress: Option<String>,
        error: Option<String>,
    },
}

pub(super) struct App {
    pub(super) store: Store,
    pub(super) profiles: Vec<StoredConnectionProfile>,
    pub(super) selected: usize,
    pub(super) state: UiState,
    pub(super) result: Option<ProfileSelectionResult>,
    pub(super) status: Option<SelectionStatus>,
    pub(super) form_field_rects: Vec<(super::form::CreationField, Rect)>,
    pub(super) profile_list_area: Option<Rect>,
    pub(super) profile_list_offset: usize,
    pub(super) form_size_ok: bool,
    pub(super) from_main_ui: bool,
}

impl App {
    pub(super) fn new(
        store: Store,
        profiles: Vec<StoredConnectionProfile>,
        initial: usize,
        from_main_ui: bool,
        recovery: Option<ProfileRecovery<'_>>,
    ) -> Self {
        let total = profiles.len() + 2;
        let state = recovery.map_or(UiState::ProfileSelection, |recovery| {
            UiState::ProfileCreation(Box::new(recovered_form(recovery)))
        });
        Self {
            store,
            profiles,
            selected: initial.min(total.saturating_sub(1)),
            state,
            result: None,
            status: None,
            form_field_rects: Vec::new(),
            profile_list_area: None,
            profile_list_offset: 0,
            form_size_ok: true,
            from_main_ui,
        }
    }
}

fn recovered_form(recovery: ProfileRecovery<'_>) -> ProfileForm {
    if let Some(draft) = recovery.draft {
        let mut form = draft.clone();
        form.error = Some(recovery.message.to_string());
        form.name_locked = recovery.editing;
        form
    } else {
        ProfileForm::from_profile(
            recovery.profile.name.clone(),
            recovery.profile.uri.clone(),
            recovery.editing,
            Some(recovery.message.to_string()),
        )
    }
}
