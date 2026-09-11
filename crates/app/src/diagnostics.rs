use std::{fmt::Write as _, path::PathBuf};

use anyhow::Result;
use poqi_config::{AppConfig, SemanticRuntimePreference};
use poqi_store::Store;
use url::Url;

const GPU_MODEL_FILE: &str = "model.onnx";
const CPU_MODEL_FILE: &str = "model-quint8-avx2.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";

const WINDOWS_INSTALLER_URL: &str =
    "https://github.com/poqi-cli/poqi/releases/latest/download/poqi-installer.cmd";
const POWERSHELL_INSTALLER_URL: &str =
    "https://github.com/poqi-cli/poqi/releases/latest/download/install.ps1";
const UNIX_INSTALLER_URL: &str =
    "https://github.com/poqi-cli/poqi/releases/latest/download/install.sh";

pub fn render_doctor(config: &AppConfig, store: &Store) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "poqi doctor")?;
    writeln!(out, "==========")?;
    writeln!(out)?;

    render_installation(&mut out)?;
    render_config(&mut out, config)?;
    render_connection(&mut out, config, store)?;
    render_semantic(&mut out, config)?;
    render_distribution(&mut out)?;

    Ok(out)
}

fn render_installation(out: &mut String) -> Result<()> {
    let exe = std::env::current_exe()?;
    writeln!(out, "Installation")?;
    writeln!(out, "- executable: {}", exe.display())?;
    writeln!(out, "- version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(out)?;
    Ok(())
}

fn render_config(out: &mut String, config: &AppConfig) -> Result<()> {
    writeln!(out, "Config")?;
    match AppConfig::config_dir() {
        Ok(dir) => writeln!(out, "- config dir: {}", dir.display())?,
        Err(err) => writeln!(out, "- config dir: unavailable ({err})")?,
    }
    writeln!(out, "- keymap: {}", config.keymap.profile)?;
    writeln!(
        out,
        "- statement timeout: {}s",
        config.db.defaults.statement_timeout.as_secs()
    )?;
    writeln!(out, "- select limit: {}", config.db.defaults.page_size)?;
    writeln!(out)?;
    Ok(())
}

fn render_connection(out: &mut String, config: &AppConfig, store: &Store) -> Result<()> {
    let env_setting = crate::bootstrap::database_url_from_lookup(std::env::var)?;
    let (env_name, env_url) = env_setting
        .as_ref()
        .map_or(("POQI_DATABASE_URL", None), |(name, value)| {
            (*name, Some(value.as_str()))
        });
    let profiles = store.connection_profiles().list()?;

    writeln!(out, "Connection onboarding")?;
    writeln!(
        out,
        "- configuration only: no login or network test was performed"
    )?;
    writeln!(
        out,
        "- test login: poqi --check-connection --profile NAME (or --url URL)"
    )?;
    writeln!(
        out,
        "- startup: profile menu; login checks use explicit URL/profile > environment > saved primary"
    )?;
    if let Some(primary) = &config.db.primary {
        writeln!(
            out,
            "- config primary: {} ({})",
            primary.name.as_deref().unwrap_or("primary"),
            redact_uri(&primary.uri)
        )?;
    } else {
        writeln!(out, "- config primary: not set")?;
    }

    match env_url.map(str::trim).filter(|value| !value.is_empty()) {
        Some(url) => writeln!(out, "- {env_name}: set ({})", redact_uri(url))?,
        None => writeln!(out, "- {env_name}: not set or empty")?,
    }

    writeln!(out, "- saved profiles: {}", profiles.len())?;
    if config.db.primary.is_none() && env_url.is_none() && profiles.is_empty() {
        writeln!(
            out,
            "- next step: run `poqi` and create a profile or generate a Docker demo database"
        )?;
    }
    writeln!(out)?;
    Ok(())
}

fn render_semantic(out: &mut String, config: &AppConfig) -> Result<()> {
    let root = semantic_model_root(config);
    let preference = config.search.semantic_runtime_preference;
    let windows_auto =
        cfg!(target_os = "windows") && matches!(preference, SemanticRuntimePreference::Auto);
    let missing = missing_semantic_assets(&root, preference, cfg!(target_os = "windows"));

    writeln!(out, "Semantic search")?;
    writeln!(
        out,
        "- runtime preference: {}",
        semantic_runtime_label(preference)
    )?;
    writeln!(out, "- model dir: {}", root.display())?;
    if matches!(preference, SemanticRuntimePreference::Off) {
        writeln!(
            out,
            "- downloads: disabled until semantic runtime is enabled"
        )?;
    } else if windows_auto {
        render_windows_auto_asset_state(out, &root)?;
    } else if missing.is_empty() {
        writeln!(out, "- model assets: present")?;
    } else {
        writeln!(out, "- missing model assets: {}", missing.join(", "))?;
        writeln!(
            out,
            "- downloads: required on first semantic search startup"
        )?;
    }
    writeln!(out)?;
    Ok(())
}

fn render_windows_auto_asset_state(out: &mut String, root: &std::path::Path) -> Result<()> {
    writeln!(
        out,
        "- tokenizer cache: {} ({TOKENIZER_FILE})",
        cache_state(root, TOKENIZER_FILE)
    )?;
    writeln!(
        out,
        "- DirectML model cache: {} ({GPU_MODEL_FILE})",
        cache_state(root, GPU_MODEL_FILE)
    )?;
    writeln!(
        out,
        "- Explicit CPU model cache: {} ({CPU_MODEL_FILE})",
        cache_state(root, CPU_MODEL_FILE)
    )?;
    writeln!(
        out,
        "- downloads: runtime selection may download the selected missing model or tokenizer"
    )?;
    Ok(())
}

fn cache_state(root: &std::path::Path, asset: &str) -> &'static str {
    if root.join(asset).exists() {
        "present"
    } else {
        "missing"
    }
}

fn render_distribution(out: &mut String) -> Result<()> {
    writeln!(out, "Distribution")?;
    writeln!(out, "- Windows bootstrapper: {WINDOWS_INSTALLER_URL}")?;
    writeln!(out, "- Windows PowerShell: {POWERSHELL_INSTALLER_URL}")?;
    writeln!(out, "- macOS/Linux shell: {UNIX_INSTALLER_URL}")?;
    writeln!(out)?;
    Ok(())
}

fn semantic_model_root(config: &AppConfig) -> PathBuf {
    if config.search.model_dir.is_absolute() {
        config.search.model_dir.clone()
    } else {
        AppConfig::config_dir().map_or_else(
            |_| config.search.model_dir.clone(),
            |dir| dir.join(&config.search.model_dir),
        )
    }
}

fn missing_semantic_assets(
    root: &std::path::Path,
    preference: SemanticRuntimePreference,
    is_windows: bool,
) -> Vec<&'static str> {
    required_semantic_assets(preference, is_windows)
        .into_iter()
        .filter(|asset| !root.join(asset).exists())
        .collect()
}

fn required_semantic_assets(
    preference: SemanticRuntimePreference,
    is_windows: bool,
) -> [&'static str; 2] {
    let model = match preference {
        SemanticRuntimePreference::Gpu => GPU_MODEL_FILE,
        SemanticRuntimePreference::Auto if is_windows => GPU_MODEL_FILE,
        SemanticRuntimePreference::Off
        | SemanticRuntimePreference::Auto
        | SemanticRuntimePreference::Cpu => CPU_MODEL_FILE,
    };
    [model, TOKENIZER_FILE]
}

fn semantic_runtime_label(preference: SemanticRuntimePreference) -> &'static str {
    match preference {
        SemanticRuntimePreference::Off => "off",
        SemanticRuntimePreference::Auto => "auto",
        SemanticRuntimePreference::Gpu => "gpu",
        SemanticRuntimePreference::Cpu => "cpu",
    }
}

pub(crate) fn redact_uri(uri: &str) -> String {
    let Ok(mut parsed) = Url::parse(uri) else {
        if uri.contains("://") {
            return "<invalid connection URI; credentials hidden>".to_string();
        }
        return redact_keyword_connection_string(uri);
    };
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("***"));
    }
    if parsed
        .query_pairs()
        .any(|(key, _)| is_sensitive_connection_key(key.as_ref()))
    {
        let pairs = parsed
            .query_pairs()
            .map(|(key, value)| {
                let value = if is_sensitive_connection_key(key.as_ref()) {
                    "***".to_string()
                } else {
                    value.into_owned()
                };
                (key.into_owned(), value)
            })
            .collect::<Vec<_>>();
        parsed.query_pairs_mut().clear().extend_pairs(pairs);
    }
    parsed.to_string()
}

fn redact_keyword_connection_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if let Some(redacted) = redact_sensitive_assignment_at(input, index) {
            out.push_str(&redacted.replacement);
            index = redacted.next_index;
        } else {
            let ch = input[index..]
                .chars()
                .next()
                .expect("index is always inside input");
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

struct RedactedAssignment {
    replacement: String,
    next_index: usize,
}

fn redact_sensitive_assignment_at(input: &str, index: usize) -> Option<RedactedAssignment> {
    if !is_assignment_boundary(input, index) {
        return None;
    }

    for key in ["password", "pass", "sslpassword"] {
        if !starts_with_ignore_ascii_case(&input[index..], key) {
            continue;
        }

        let mut cursor = index + key.len();
        cursor = skip_ascii_whitespace(input, cursor);
        if input.as_bytes().get(cursor).copied() != Some(b'=') {
            continue;
        }
        cursor += 1;
        cursor = skip_ascii_whitespace(input, cursor);

        let (replacement, next_index) = if input.as_bytes().get(cursor).copied() == Some(b'\'') {
            let value_end = scan_single_quoted_value(input, cursor + 1);
            (format!("{}'***'", &input[index..cursor]), value_end)
        } else {
            let value_end = scan_unquoted_value(input, cursor);
            (format!("{}***", &input[index..cursor]), value_end)
        };
        return Some(RedactedAssignment {
            replacement,
            next_index,
        });
    }

    None
}

fn is_assignment_boundary(input: &str, index: usize) -> bool {
    index == 0
        || input[..index]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
}

fn skip_ascii_whitespace(input: &str, mut index: usize) -> usize {
    while input
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    index
}

fn scan_unquoted_value(input: &str, mut index: usize) -> usize {
    while index < input.len() {
        let Some(ch) = input[index..].chars().next() else {
            break;
        };
        if ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn scan_single_quoted_value(input: &str, mut index: usize) -> usize {
    while index < input.len() {
        let Some(ch) = input[index..].chars().next() else {
            break;
        };
        index += ch.len_utf8();
        if ch == '\'' {
            break;
        }
        if ch == '\\' && index < input.len() {
            let Some(escaped) = input[index..].chars().next() else {
                break;
            };
            index += escaped.len_utf8();
        }
    }
    index
}

fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn is_sensitive_connection_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "password" | "pass" | "sslpassword"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_uri_hides_password() {
        let redacted = redact_uri("postgres://user:secret@example.com/db");

        assert_eq!(redacted, "postgres://user:***@example.com/db");
    }

    #[test]
    fn redact_uri_hides_password_query_params() {
        let redacted =
            redact_uri("postgres://user@example.com/db?sslpassword=secret&sslmode=require");

        assert_eq!(
            redacted,
            "postgres://user@example.com/db?sslpassword=***&sslmode=require"
        );
    }

    #[test]
    fn redact_uri_hides_keyword_dsn_passwords() {
        let redacted =
            redact_uri("host=localhost user=pg password='very secret' sslpassword=cert-secret");

        assert_eq!(
            redacted,
            "host=localhost user=pg password='***' sslpassword=***"
        );
    }

    #[test]
    fn redact_uri_fails_closed_for_malformed_uri() {
        for malformed in [
            "postgres://user:top-secret@localhost:invalid/app",
            "postgres://user:top-secret@[broken-host/app",
        ] {
            let redacted = redact_uri(malformed);

            assert_eq!(redacted, "<invalid connection URI; credentials hidden>");
            assert!(!redacted.contains("top-secret"));
        }
    }

    #[test]
    fn semantic_assets_match_runtime_backend() {
        assert_eq!(
            required_semantic_assets(SemanticRuntimePreference::Cpu, true),
            [CPU_MODEL_FILE, TOKENIZER_FILE]
        );
        assert_eq!(
            required_semantic_assets(SemanticRuntimePreference::Gpu, true),
            [GPU_MODEL_FILE, TOKENIZER_FILE]
        );
        assert_eq!(
            required_semantic_assets(SemanticRuntimePreference::Auto, true),
            [GPU_MODEL_FILE, TOKENIZER_FILE]
        );
        assert_eq!(
            required_semantic_assets(SemanticRuntimePreference::Auto, false),
            [CPU_MODEL_FILE, TOKENIZER_FILE]
        );
    }

    #[test]
    fn windows_auto_diagnostics_label_int8_as_explicit_cpu_cache() {
        let mut output = String::new();
        render_windows_auto_asset_state(&mut output, std::path::Path::new("missing-model-root"))
            .unwrap();

        assert!(output.contains("Explicit CPU model cache"));
        assert!(!output.contains("CPU fallback model cache"));
    }
}
