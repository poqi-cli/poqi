use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ConnectionProfile {
    pub name: String,
    pub uri: String,
    pub max_pool_size: Option<usize>,
    pub connect_timeout: Option<Duration>,
}

impl ConnectionProfile {
    #[must_use]
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uri: uri.into(),
            max_pool_size: None,
            connect_timeout: None,
        }
    }
}
