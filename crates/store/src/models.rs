use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredObjectKind {
    Table,
    Column,
    View,
    Snippet,
}

#[derive(Debug, Clone)]
pub struct Favorite {
    pub object_id: String,
    pub kind: StoredObjectKind,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RecentEntry {
    pub object_id: String,
    pub last_used: DateTime<Utc>,
    pub hits: u32,
}
