use poqi_db::ConnectionProfile;

use super::connection_detail;

#[test]
fn database_url_preserves_explicit_and_empty_override() {
    for value in ["postgres://current", ""] {
        let selected = super::database_url_from_lookup(|name| {
            assert_eq!(name, "POQI_DATABASE_URL");
            Ok(value.into())
        })
        .expect("lookup succeeds");
        assert_eq!(selected, Some(("POQI_DATABASE_URL", value.into())));
    }
}

#[test]
fn missing_database_environment_has_no_override() {
    let selected = super::database_url_from_lookup(|name| {
        assert_eq!(name, "POQI_DATABASE_URL");
        Err(std::env::VarError::NotPresent)
    })
    .expect("lookup succeeds");
    assert_eq!(selected, None);
}

#[test]
fn invalid_database_environment_does_not_fall_back_or_leak_value() {
    let error =
        super::database_url_from_lookup(|_| Err(std::env::VarError::NotUnicode("secret".into())))
            .expect_err("invalid environment must be reported")
            .to_string();
    assert!(error.contains("POQI_DATABASE_URL"));
    assert!(!error.contains("secret"));
}

#[test]
fn loading_detail_redacts_url_password() {
    let profile = ConnectionProfile::new("primary", "postgres://user:top-secret@example.com/app");

    let detail = connection_detail(&profile);

    assert_eq!(detail, "Dialing postgres://user:***@example.com/app...");
    assert!(!detail.contains("top-secret"));
}

#[test]
fn loading_detail_redacts_sensitive_query_values() {
    let profile = ConnectionProfile::new(
        "primary",
        "postgres://user@example.com/app?sslpassword=cert-secret&sslmode=require",
    );

    let detail = connection_detail(&profile);

    assert!(detail.contains("sslpassword=***"));
    assert!(detail.contains("sslmode=require"));
    assert!(!detail.contains("cert-secret"));
}

#[test]
fn loading_detail_redacts_keyword_dsn_passwords() {
    let profile = ConnectionProfile::new(
        "primary",
        "host=localhost user=pg password='very secret' sslpassword=cert-secret",
    );

    let detail = connection_detail(&profile);

    assert!(detail.contains("password='***'"));
    assert!(detail.contains("sslpassword=***"));
    assert!(!detail.contains("very secret"));
    assert!(!detail.contains("cert-secret"));
}

#[test]
fn loading_detail_hides_malformed_uri_credentials() {
    let profile = ConnectionProfile::new(
        "primary",
        "postgres://user:top-secret@localhost:invalid/app",
    );

    let detail = connection_detail(&profile);

    assert_eq!(
        detail,
        "Dialing <invalid connection URI; credentials hidden>..."
    );
    assert!(!detail.contains("top-secret"));
}
