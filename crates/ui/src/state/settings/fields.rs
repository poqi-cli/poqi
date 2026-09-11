const FLOAT_STEP: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingKey {
    UiTheme,
    UiFastScrollStep,
    UiMouseScrollLines,
    UiSelectTopLimit,
    UiMainTickRate,
    UiMenuTickRate,
    UiStatusExpireSecs,
    UiStatusAutoClearSecs,
    SearchSemanticRuntimePreference,
    SearchSemanticBatchSize,
    SearchSemanticTopK,
    SearchSemanticScoreThreshold,
    SearchSemanticTitleColumn,
    SearchSemanticDim,
    KeymapProfile,
}

#[derive(Debug, Clone)]
pub(crate) enum SettingsRow {
    Field(SettingField),
    Action(SettingsAction),
}

impl SettingsRow {
    pub(crate) fn is_modified(&self) -> bool {
        match self {
            Self::Field(field) => field.is_modified(),
            Self::Action(_) => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SettingsAction {
    SaveAndExit,
}

#[derive(Debug, Clone)]
pub(crate) enum SettingField {
    Choice(ChoiceField),
    Number(NumberField),
    Float(FloatField),
    Text(TextField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    Choice,
    Number,
    Float,
    Text,
}

impl SettingField {
    pub(crate) fn kind(&self) -> FieldKind {
        match self {
            Self::Choice(_) => FieldKind::Choice,
            Self::Number(_) => FieldKind::Number,
            Self::Float(_) => FieldKind::Float,
            Self::Text(_) => FieldKind::Text,
        }
    }

    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Choice(field) => &field.label,
            Self::Number(field) => &field.label,
            Self::Float(field) => &field.label,
            Self::Text(field) => &field.label,
        }
    }

    pub(crate) fn value_display(&self) -> String {
        match self {
            Self::Choice(field) => field.current_label(),
            Self::Number(field) => field.value.to_string(),
            Self::Float(field) => match field.value {
                Some(value) => format!("{value:.2}"),
                None => "—".to_string(),
            },
            Self::Text(field) => field.value.clone().unwrap_or_else(|| "—".to_string()),
        }
    }

    pub(crate) fn is_editable(&self) -> bool {
        !matches!(self, Self::Choice(_))
    }

    pub(crate) fn is_modified(&self) -> bool {
        match self {
            Self::Choice(field) => field.selected != field.original_selected,
            Self::Number(field) => field.value != field.original_value,
            Self::Float(field) => field.value != field.original_value,
            Self::Text(field) => field.value != field.original_value,
        }
    }

    pub(crate) fn nudge(&mut self, forward: bool) {
        match self {
            SettingField::Choice(field) => field.nudge(forward),
            SettingField::Number(field) => field.nudge(forward),
            SettingField::Float(field) => field.nudge(forward),
            SettingField::Text(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ChoiceField {
    pub(crate) key: SettingKey,
    pub(crate) label: String,
    pub(crate) options: Vec<String>,
    pub(crate) selected: usize,
    pub(crate) original_selected: usize,
}

impl ChoiceField {
    pub(crate) fn current_label(&self) -> String {
        self.options
            .get(self.selected)
            .cloned()
            .unwrap_or_else(|| "—".to_string())
    }

    pub(crate) fn nudge(&mut self, forward: bool) {
        if self.options.is_empty() {
            return;
        }
        if forward {
            if self.selected + 1 < self.options.len() {
                self.selected += 1;
            }
        } else if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NumberField {
    pub(crate) key: SettingKey,
    pub(crate) label: String,
    pub(crate) value: u64,
    pub(crate) original_value: u64,
    pub(crate) min: u64,
}

impl NumberField {
    pub(crate) fn nudge(&mut self, forward: bool) {
        if forward {
            self.value = self.value.saturating_add(1);
        } else if self.value > self.min {
            self.value -= 1;
        }
        if self.value < self.min {
            self.value = self.min;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FloatField {
    pub(crate) key: SettingKey,
    pub(crate) label: String,
    pub(crate) value: Option<f32>,
    pub(crate) original_value: Option<f32>,
    pub(crate) min: f32,
    pub(crate) max: f32,
}

impl FloatField {
    pub(crate) fn nudge(&mut self, forward: bool) {
        let mut value = self.value.unwrap_or(0.0);
        if forward {
            value += FLOAT_STEP;
        } else {
            value -= FLOAT_STEP;
        }
        self.value = Some(value.clamp(self.min, self.max));
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TextField {
    pub(crate) key: SettingKey,
    pub(crate) label: String,
    pub(crate) value: Option<String>,
    pub(crate) original_value: Option<String>,
}
