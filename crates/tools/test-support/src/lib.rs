pub use testcontainers::{runners::AsyncRunner, ContainerAsync, ContainerRequest, ImageExt};
pub use testcontainers_modules::postgres::Postgres;

pub const DEFAULT_PG_TAG: &str = "18-alpine";
pub const POSTGRES_PORT: u16 = 5432;

/// Check whether the Docker-backed integration tests are allowed to run.
#[must_use]
pub fn integration_tests_enabled() -> bool {
    matches!(
        std::env::var("POQI_ENABLE_TESTCONTAINERS")
            .map(|value| value.to_ascii_lowercase()),
        Ok(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
    )
}

/// Print a skip notice (when needed) and return whether the suite should execute.
#[must_use]
pub fn should_run_integration_tests(suite: &str) -> bool {
    if integration_tests_enabled() {
        true
    } else {
        eprintln!("skipping {suite} integration test; set POQI_ENABLE_TESTCONTAINERS=1 to enable");
        false
    }
}

/// Resolve the Postgres Docker tag configurable via `POQI_TESTCONTAINERS_PG_TAG`.
#[must_use]
pub fn postgres_tag() -> String {
    match std::env::var("POQI_TESTCONTAINERS_PG_TAG") {
        Ok(tag) if !tag.trim().is_empty() => tag,
        _ => DEFAULT_PG_TAG.to_string(),
    }
}

/// Build a runnable Postgres image with the standard environment variables.
pub fn postgres_image() -> ContainerRequest<Postgres> {
    Postgres::default()
        .with_user("postgres")
        .with_password("postgres")
        .with_db_name("postgres")
        .with_tag(postgres_tag())
}

/// Start a `PostgreSQL` test container and return the container handle with its mapped port.
///
/// The container handle must stay in scope for the lifetime of the test.
///
/// # Panics
///
/// Panics if the container cannot be started or its mapped port cannot be resolved.
pub async fn start_postgres_container() -> (ContainerAsync<Postgres>, u16) {
    let node = postgres_image()
        .start()
        .await
        .expect("start postgres container");
    let port = node
        .get_host_port_ipv4(POSTGRES_PORT)
        .await
        .expect("map postgres port");
    (node, port)
}
