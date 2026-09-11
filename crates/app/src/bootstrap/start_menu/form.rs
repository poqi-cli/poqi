use poqi_store::StoredConnectionProfile;

use super::state::ProfileSelectionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TextInput {
    pub(super) value: String,
    pub(super) cursor: usize,
}

impl TextInput {
    pub(super) fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub(super) fn insert(&mut self, text: &str) {
        let byte = self.byte_index();
        self.value.insert_str(byte, text);
        self.cursor += text.chars().count();
    }

    pub(super) fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let start = self.byte_index();
        let end = self.next_byte_index();
        self.value.replace_range(start..end, "");
    }

    pub(super) fn delete(&mut self) {
        if self.cursor == self.value.chars().count() {
            return;
        }
        let start = self.byte_index();
        let end = self.next_byte_index();
        self.value.replace_range(start..end, "");
    }

    pub(super) fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub(super) fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub(super) fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    fn byte_index(&self) -> usize {
        self.value
            .char_indices()
            .nth(self.cursor)
            .map_or(self.value.len(), |(index, _)| index)
    }

    fn next_byte_index(&self) -> usize {
        self.value
            .char_indices()
            .nth(self.cursor + 1)
            .map_or(self.value.len(), |(index, _)| index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionMode {
    Url,
    Structured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TlsMode {
    Prefer,
    Require,
    Disable,
}

impl TlsMode {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Prefer => Self::Require,
            Self::Require => Self::Disable,
            Self::Disable => Self::Prefer,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::Disable => "disable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CreationField {
    Name,
    Mode,
    Uri,
    Host,
    Port,
    User,
    Password,
    Database,
    Tls,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileForm {
    pub(super) name: TextInput,
    pub(super) uri: TextInput,
    pub(super) host: TextInput,
    pub(super) port: TextInput,
    pub(super) user: TextInput,
    pub(super) password: TextInput,
    pub(super) database: TextInput,
    pub(super) mode: ConnectionMode,
    pub(super) tls: TlsMode,
    pub(super) active_field: CreationField,
    pub(super) error: Option<String>,
    pub(super) name_locked: bool,
}

impl ProfileForm {
    pub(crate) fn is_editing(&self) -> bool {
        self.name_locked
    }

    pub(super) fn new() -> Self {
        Self {
            name: TextInput::new(""),
            uri: TextInput::new(""),
            host: TextInput::new("localhost"),
            port: TextInput::new("5432"),
            user: TextInput::new(""),
            password: TextInput::new(""),
            database: TextInput::new(""),
            mode: ConnectionMode::Url,
            tls: TlsMode::Require,
            active_field: CreationField::Name,
            error: None,
            name_locked: false,
        }
    }

    pub(super) fn from_profile(
        name: impl Into<String>,
        uri: impl Into<String>,
        name_locked: bool,
        error: Option<String>,
    ) -> Self {
        Self {
            name: TextInput::new(name),
            uri: TextInput::new(uri),
            active_field: CreationField::Uri,
            error,
            name_locked,
            ..Self::new()
        }
    }

    pub(super) fn active_text_mut(&mut self) -> Option<&mut TextInput> {
        match self.active_field {
            CreationField::Name if !self.name_locked => Some(&mut self.name),
            CreationField::Uri => Some(&mut self.uri),
            CreationField::Host => Some(&mut self.host),
            CreationField::Port => Some(&mut self.port),
            CreationField::User => Some(&mut self.user),
            CreationField::Password => Some(&mut self.password),
            CreationField::Database => Some(&mut self.database),
            _ => None,
        }
    }

    pub(super) fn fields(&self) -> &'static [CreationField] {
        const URL: &[CreationField] =
            &[CreationField::Name, CreationField::Uri, CreationField::Mode];
        const STRUCTURED: &[CreationField] = &[
            CreationField::Name,
            CreationField::Host,
            CreationField::Port,
            CreationField::User,
            CreationField::Password,
            CreationField::Database,
            CreationField::Tls,
            CreationField::Mode,
        ];
        match self.mode {
            ConnectionMode::Url => URL,
            ConnectionMode::Structured => STRUCTURED,
        }
    }

    pub(super) fn move_field(&mut self, backwards: bool) {
        let fields = self.fields();
        let current = fields
            .iter()
            .position(|field| *field == self.active_field)
            .unwrap_or(0);
        let next = if backwards {
            current.checked_sub(1).unwrap_or(fields.len() - 1)
        } else {
            (current + 1) % fields.len()
        };
        self.active_field = fields[next];
    }

    pub(super) fn toggle_active(&mut self) {
        match self.active_field {
            CreationField::Mode => self.toggle_mode(),
            CreationField::Tls => self.tls = self.tls.next(),
            _ => {}
        }
    }

    fn toggle_mode(&mut self) {
        if self.name_locked {
            self.error = Some("Saved profiles stay in URL mode to preserve every option".into());
        } else if self.mode == ConnectionMode::Url && !self.uri.value.trim().is_empty() {
            self.error = Some("Clear the URL before switching so no options are lost".into());
        } else {
            if self.mode == ConnectionMode::Structured {
                if let Ok(uri) = self.structured_uri() {
                    self.uri = TextInput::new(uri);
                }
            }
            self.mode = if self.mode == ConnectionMode::Url {
                ConnectionMode::Structured
            } else {
                ConnectionMode::Url
            };
            self.active_field = CreationField::Mode;
            self.error = None;
        }
    }

    pub(super) fn submit(
        &self,
        profiles: &[StoredConnectionProfile],
    ) -> Result<ProfileSelectionResult, String> {
        let name = if self.name_locked {
            self.name.value.as_str()
        } else {
            self.name.value.trim()
        };
        if name.is_empty() {
            return Err("Profile name cannot be empty".into());
        }
        if !self.name_locked && profiles.iter().any(|profile| profile.name == name) {
            return Err(format!("A saved profile named '{name}' already exists"));
        }
        let uri = self.connection_uri()?;
        poqi_db::validate_connection_uri(&uri)
            .map_err(|error| error.connection_hint().to_string())?;
        Ok(ProfileSelectionResult::CreateNew {
            name: name.to_string(),
            uri,
            draft: Box::new(self.clone()),
        })
    }

    pub(super) fn connection_uri(&self) -> Result<String, String> {
        match self.mode {
            ConnectionMode::Url => {
                if self.uri.value.trim().is_empty() {
                    Err("Connection URL cannot be empty".into())
                } else {
                    Ok(self.uri.value.trim().to_string())
                }
            }
            ConnectionMode::Structured => self.structured_uri(),
        }
    }

    fn structured_uri(&self) -> Result<String, String> {
        let host = self.host.value.trim();
        let database = self.database.value.trim();
        if host.is_empty() || database.is_empty() {
            return Err("Host and database are required".into());
        }
        if self.user.value.trim().is_empty() {
            return Err("User is required".into());
        }
        let port = self
            .port
            .value
            .parse::<u16>()
            .map_err(|_| "Port must be a number from 1 to 65535".to_string())?;
        if port == 0 {
            return Err("Port must be a number from 1 to 65535".into());
        }
        self.build_structured_url(host, database, port)
    }

    fn build_structured_url(
        &self,
        host: &str,
        database: &str,
        port: u16,
    ) -> Result<String, String> {
        let mut url = url::Url::parse("postgresql://localhost")
            .map_err(|_| "Could not build the connection URL".to_string())?;
        url.set_host(Some(host))
            .map_err(|_| "Host is not valid".to_string())?;
        url.set_port(Some(port))
            .map_err(|()| "Port is not valid".to_string())?;
        url.set_username(self.user.value.trim())
            .map_err(|()| "User is not valid".to_string())?;
        if !self.password.value.is_empty() {
            url.set_password(Some(&self.password.value))
                .map_err(|()| "Password is not valid".to_string())?;
        }
        url.path_segments_mut()
            .map_err(|()| "Database is not valid".to_string())?
            .clear()
            .push(database);
        url.query_pairs_mut()
            .append_pair("sslmode", self.tls.label());
        Ok(url.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileForm, ProfileSelectionResult, TextInput};

    #[test]
    fn text_editing_is_unicode_safe() {
        let mut input = TextInput::new("a🦀é");
        input.move_left();
        input.backspace();
        input.insert("界");
        input.move_left();
        input.delete();
        assert_eq!(input.value, "aé");
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn structured_uri_percent_encodes_credentials_and_database() {
        let mut form = ProfileForm::new();
        form.mode = super::ConnectionMode::Structured;
        form.host = TextInput::new("db.example.com");
        form.port = TextInput::new("5433");
        form.user = TextInput::new("user@team");
        form.password = TextInput::new("p:a ss");
        form.database = TextInput::new("sales/eu");
        let uri = form.connection_uri().expect("structured URI should build");
        assert_eq!(
            uri,
            "postgresql://user%40team:p%3Aa%20ss@db.example.com:5433/sales%2Feu?sslmode=require"
        );
    }

    #[test]
    fn incomplete_structured_draft_can_return_to_url_mode() {
        let mut form = ProfileForm::new();
        form.mode = super::ConnectionMode::Structured;
        form.active_field = super::CreationField::Mode;
        form.host = TextInput::new("draft.example.com");
        form.database.clear();
        form.toggle_active();
        assert_eq!(form.mode, super::ConnectionMode::Url);
        assert_eq!(form.host.value, "draft.example.com");
        assert!(form.uri.value.is_empty());
        assert!(form.error.is_none());
    }

    #[test]
    fn submit_rejects_invalid_uri_with_safe_hint() {
        let mut form = ProfileForm::new();
        form.name = TextInput::new("invalid");
        form.uri = TextInput::new("not a postgres connection");
        let expected = poqi_db::validate_connection_uri(&form.uri.value)
            .expect_err("URI should be rejected")
            .connection_hint();
        assert_eq!(
            form.submit(&[]).expect_err("form should be rejected"),
            expected
        );
    }

    #[test]
    fn new_profile_cannot_overwrite_saved_name() {
        let store = poqi_store::Store::new_with_test_key(Some(":memory:".into()), [7; 32]);
        let saved = store
            .connection_profiles()
            .upsert(&poqi_store::NewConnectionProfile::new(
                "production",
                "postgres://user@host/db",
            ))
            .expect("create in-memory profile");
        let mut form = ProfileForm::new();
        form.name = TextInput::new("production");
        form.uri = TextInput::new("postgres://other@host/db");
        assert_eq!(
            form.submit(&[saved]).expect_err("collision should fail"),
            "A saved profile named 'production' already exists"
        );
    }

    #[test]
    fn locked_profile_preserves_exact_name() {
        let form = ProfileForm::from_profile(
            "Test Database (Docker:55432) ",
            "postgres://user@host/db?sslmode=disable",
            true,
            None,
        );
        let result = form.submit(&[]).expect("locked profile should validate");
        let ProfileSelectionResult::CreateNew { name, .. } = result else {
            panic!("form should return a connection");
        };
        assert_eq!(name, "Test Database (Docker:55432) ");
    }
}
