use std::borrow::Cow;

pub const NULL_SENTINEL: &str = "[NULL]";
const MAX_VALUE_PREVIEW: usize = 512;

/// Preserve the user's query as plain text for the symmetric Granite encoder.
#[must_use]
pub fn format_query_prompt(user_query: &str) -> String {
    user_query.trim().to_string()
}

/// Flatten a row into plain labelled text for the symmetric Granite encoder.
/// Each column tuple contains `(name, display_value, is_sql_null)`.
#[must_use]
pub fn format_row_prompt(title: Option<&str>, columns: &[(&str, &str, bool)]) -> String {
    let title = title.map(str::trim).filter(|value| !value.is_empty());
    let mut parts = Vec::with_capacity(columns.len() + usize::from(title.is_some()));
    if let Some(title) = title {
        parts.push(title.to_string());
    }
    for (name, value, is_null) in columns {
        let trimmed = value.trim();
        let normalized = if *is_null { NULL_SENTINEL } else { trimmed };
        let clipped = clip_value(normalized, MAX_VALUE_PREVIEW);
        parts.push(format!("{name}: {clipped}"));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(" | ")
    }
}

fn clip_value(value: &str, limit: usize) -> Cow<'_, str> {
    if value.chars().count() <= limit {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(value.chars().take(limit).collect())
    }
}
