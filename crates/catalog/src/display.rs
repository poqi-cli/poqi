use std::fmt::Write as _;

/// Render a database identifier without allowing terminal control or bidi
/// characters to alter the surrounding metadata UI.
///
/// Backslashes are escaped too, so an identifier containing the literal text
/// `\u{000A}` remains visibly distinct from one containing a newline.
#[must_use]
pub fn display_identifier(value: &str) -> String {
    let mut displayed = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch == '\\' {
            displayed.push_str("\\\\");
        } else if ch.is_control() || is_bidi_control(ch) {
            write!(&mut displayed, "\\u{{{:04X}}}", u32::from(ch))
                .expect("writing to a String cannot fail");
        } else {
            displayed.push(ch);
        }
    }
    displayed
}

/// Whether selecting this identifier should require an explicit user action
/// before any table data is fetched.
#[must_use]
pub fn identifier_requires_explicit_preview(value: &str) -> bool {
    value
        .chars()
        .any(|ch| ch.is_control() || is_bidi_control(ch))
}

fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::QualifiedRelation;

    #[test]
    fn dangerous_characters_are_visible_and_injective() {
        assert_eq!(display_identifier("line\nname"), "line\\u{000A}name");
        assert_eq!(
            display_identifier("left\u{202E}right"),
            "left\\u{202E}right"
        );
        assert_eq!(display_identifier("c1\u{0085}"), "c1\\u{0085}");
        assert_eq!(display_identifier("literal\\name"), "literal\\\\name");

        assert_ne!(display_identifier("\n"), display_identifier("\\u{000A}"));
    }

    #[test]
    fn control_and_bidi_identifiers_require_explicit_preview() {
        for identifier in ["line\nname", "left\u{202E}right", "c1\u{0085}"] {
            assert!(identifier_requires_explicit_preview(identifier));
        }
        assert!(!identifier_requires_explicit_preview("slash\\name"));
        assert!(!identifier_requires_explicit_preview("public_widgets"));
    }

    #[test]
    fn every_unicode_bidi_control_is_escaped_and_gated() {
        for ch in [
            '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}',
            '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ] {
            let identifier = ch.to_string();
            assert!(identifier_requires_explicit_preview(&identifier));
            assert!(display_identifier(&identifier).starts_with("\\u{"));
        }
    }

    #[test]
    fn relation_display_is_escaped_but_sql_identity_stays_exact() {
        let relation = QualifiedRelation::in_schema("safe", "audit\u{202E}cod");

        assert_eq!(relation.display_name(), "\"safe\".\"audit\\u{202E}cod\"");
        assert_eq!(relation.quoted(), "\"safe\".\"audit\u{202E}cod\"");
        assert!(relation.requires_explicit_preview());
    }
}
