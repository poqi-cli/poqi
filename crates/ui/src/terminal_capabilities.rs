use std::env;

const RESULTS_SCROLLBAR_ENV: &str = "POQI_RESULTS_HORIZONTAL_SCROLLBAR";
const GLYPH_OVERRIDE_ENV: &str = "POQI_GLYPHS";

#[derive(Debug, Clone, Copy)]
pub(crate) struct GlyphSet {
    pub(crate) schema_icon: &'static str,
    pub(crate) table_icon: &'static str,
    pub(crate) branch: &'static str,
    pub(crate) elbow: &'static str,
    pub(crate) space: &'static str,
}

/// Snapshot of terminal-only capabilities so we can tune rendering for pointer-less environments.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalCapabilities {
    results_horizontal_scrollbar: bool,
    glyphs: GlyphSet,
}

impl TerminalCapabilities {
    #[must_use]
    pub(crate) fn detect() -> Self {
        Self::from_lookup(|key| env::var_os(key).map(|value| value.to_string_lossy().into_owned()))
    }

    #[must_use]
    pub(crate) fn results_horizontal_scrollbar_enabled(self) -> bool {
        self.results_horizontal_scrollbar
    }

    #[must_use]
    pub(crate) fn glyphs(self) -> GlyphSet {
        self.glyphs
    }

    #[cfg(test)]
    pub(crate) fn testing(results_horizontal_scrollbar: bool) -> Self {
        Self {
            results_horizontal_scrollbar,
            glyphs: GlyphSet::unicode(),
        }
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let override_pref = lookup(RESULTS_SCROLLBAR_ENV)
            .and_then(|value| parse_scrollbar_override(value.as_str()));
        let pointerless = detect_pointerless_terminal(&mut lookup);
        let results_horizontal_scrollbar = match override_pref {
            Some(pref) => pref,
            None => !pointerless,
        };
        let glyphs = select_glyphs(&mut lookup);
        Self {
            results_horizontal_scrollbar,
            glyphs,
        }
    }
}

fn detect_pointerless_terminal(lookup: &mut impl FnMut(&str) -> Option<String>) -> bool {
    let term_program = lookup("TERM_PROGRAM");
    let from_term_program = term_program
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("vscode"));
    from_term_program
        || lookup("VSCODE_PID").is_some()
        || lookup("VSCODE_IPC_HOOK").is_some()
        || lookup("VSCODE_CWD").is_some()
}

fn parse_scrollbar_override(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "always" | "on" | "true" | "1" => Some(true),
        "never" | "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn select_glyphs(lookup: &mut impl FnMut(&str) -> Option<String>) -> GlyphSet {
    if let Some(value) = lookup(GLYPH_OVERRIDE_ENV) {
        let value = value.trim().to_ascii_lowercase();
        if value == "unicode" {
            return GlyphSet::unicode();
        }
    }

    GlyphSet::unicode()
}

impl GlyphSet {
    #[must_use]
    pub const fn unicode() -> Self {
        Self {
            schema_icon: "■",
            table_icon: "",
            branch: "  ├─",
            elbow: "  └─",
            space: "",
        }
    }
}

#[cfg(test)]
fn capabilities_from_pairs(entries: &[(&str, &str)]) -> TerminalCapabilities {
    TerminalCapabilities::from_lookup(|key| {
        entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| (*value).to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_allow_scrollbar() {
        let caps = capabilities_from_pairs(&[]);
        assert!(caps.results_horizontal_scrollbar_enabled());
    }

    #[test]
    fn scrollbar_override_controls_visibility() {
        let caps = capabilities_from_pairs(&[(RESULTS_SCROLLBAR_ENV, "never")]);
        assert!(!caps.results_horizontal_scrollbar_enabled());
        let invalid = capabilities_from_pairs(&[(RESULTS_SCROLLBAR_ENV, "invalid")]);
        assert!(invalid.results_horizontal_scrollbar_enabled());
    }

    #[test]
    fn vscode_terminal_disables_scrollbar() {
        let caps = capabilities_from_pairs(&[("TERM_PROGRAM", "vscode")]);
        assert!(!caps.results_horizontal_scrollbar_enabled());
    }

    #[test]
    fn override_can_force_scrollbar_on() {
        let caps = capabilities_from_pairs(&[
            ("TERM_PROGRAM", "vscode"),
            (RESULTS_SCROLLBAR_ENV, "always"),
        ]);
        assert!(caps.results_horizontal_scrollbar_enabled());
    }

    #[test]
    fn override_can_force_scrollbar_off() {
        let caps = capabilities_from_pairs(&[(RESULTS_SCROLLBAR_ENV, "never")]);
        assert!(!caps.results_horizontal_scrollbar_enabled());
    }
}
