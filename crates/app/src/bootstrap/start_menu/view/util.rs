/// Truncate long strings for narrow viewports while keeping the caller in control of the limit.
pub(super) fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_text;

    #[test]
    fn leaves_short_text() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn truncates_ascii() {
        assert_eq!(truncate_text("hello world", 5), "hello...");
    }

    #[test]
    fn truncates_utf8_without_panic() {
        assert_eq!(truncate_text("éclair", 3), "écl...");
    }
}
