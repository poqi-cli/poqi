use crate::error::TestDataError;
use bollard::models::{
    ContainerCreateBody, HostConfig, PortBinding, RestartPolicy, RestartPolicyNameEnum,
};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
    RemoveContainerOptionsBuilder,
};
use bollard::Docker;
use futures::StreamExt;
use poqi_db::{ConnectionProfile, Database};
use rand::{rngs::OsRng, RngCore};
use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::time::{Duration, Instant};

pub struct TestContainer {
    pub connection_uri: String,
    pub port: u16,
    pub container_id: String,
}

const TESTDATA_LABEL: &str = "com.poqi.testdata";
const TESTDATA_LABEL_MANAGED_BY: &str = "com.poqi.testdata.managed-by";
const TESTDATA_LABEL_PORT: &str = "com.poqi.testdata.port";
const TESTDATA_LABEL_VALUE: &str = "poqi";
const POSTGRES_IMAGE: &str =
    "postgres:18-alpine@sha256:d3e1620b530c944afa6e887d22eb899824da68e19c52024bf98f5220c88a65b2";
const DEMO_PASSWORD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const DEMO_PASSWORD_LENGTH: usize = 32;
const HOST_PORT_START: u16 = 55_432;
const HOST_PORT_SPAN: u16 = 32;
const POSTGRES_READY_TIMEOUT: Duration = Duration::from_secs(30);
const POSTGRES_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
const POSTGRES_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Checks if Docker is available on the system.
///
/// # Errors
///
/// Returns `TestDataError::DockerNotAvailable` if Docker is not installed or running.
pub async fn check_docker_available() -> Result<(), TestDataError> {
    let docker = Docker::connect_with_local_defaults().map_err(|e| {
        tracing::error!("Failed to connect to Docker: {e}");
        TestDataError::DockerNotAvailable
    })?;

    docker.ping().await.map_err(|e| {
        tracing::error!("Docker ping failed: {e}");
        TestDataError::DockerNotAvailable
    })?;

    Ok(())
}

async fn collect_reserved_ports(docker: &Docker) -> Result<HashSet<u16>, TestDataError> {
    let mut filters = HashMap::new();
    filters.insert(
        "label".to_string(),
        vec![format!("{TESTDATA_LABEL}={TESTDATA_LABEL_VALUE}")],
    );

    let containers = docker
        .list_containers(Some(
            ListContainersOptionsBuilder::default()
                .all(true)
                .filters(&filters)
                .build(),
        ))
        .await
        .map_err(|e| {
            tracing::error!("Failed to list existing test data containers: {e}");
            TestDataError::ContainerStartFailed(format!(
                "Failed to list existing test data containers: {e}"
            ))
        })?;

    let mut reserved = HashSet::new();

    for container in containers {
        if let Some(ports) = container.ports {
            for port in ports {
                if let Some(public_port) = port.public_port {
                    reserved.insert(public_port);
                }
            }
        }
    }

    Ok(reserved)
}

fn is_port_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

async fn find_available_host_port(docker: &Docker) -> Result<u16, TestDataError> {
    let reserved = collect_reserved_ports(docker).await?;
    let max_port = HOST_PORT_START
        .checked_add(HOST_PORT_SPAN - 1)
        .ok_or_else(|| {
            TestDataError::ContainerStartFailed(
                "Invalid host port search range for test data container".to_string(),
            )
        })?;

    for port in HOST_PORT_START..=max_port {
        if reserved.contains(&port) {
            continue;
        }

        if is_port_free(port) {
            return Ok(port);
        }
    }

    Err(TestDataError::ContainerStartFailed(format!(
        "No available host ports in range {HOST_PORT_START}-{max_port}"
    )))
}

fn build_host_config(host_port: u16) -> HostConfig {
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "5432/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some(host_port.to_string()),
        }]),
    );

    HostConfig {
        port_bindings: Some(port_bindings),
        auto_remove: Some(false),
        restart_policy: Some(RestartPolicy {
            name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
            maximum_retry_count: Some(0),
        }),
        ..Default::default()
    }
}

fn build_labels(host_port: u16) -> HashMap<String, String> {
    let mut labels: HashMap<String, String> = HashMap::new();
    labels.insert(TESTDATA_LABEL.to_string(), TESTDATA_LABEL_VALUE.to_string());
    labels.insert(
        TESTDATA_LABEL_MANAGED_BY.to_string(),
        TESTDATA_LABEL_VALUE.to_string(),
    );
    labels.insert(TESTDATA_LABEL_PORT.to_string(), host_port.to_string());
    labels
}

fn generate_demo_password() -> Result<String, TestDataError> {
    let mut random_bytes = [0_u8; DEMO_PASSWORD_LENGTH];
    OsRng.try_fill_bytes(&mut random_bytes).map_err(|_| {
        TestDataError::ContainerStartFailed(
            "Failed to generate secure test database credentials".to_string(),
        )
    })?;

    Ok(random_bytes
        .iter()
        .map(|byte| char::from(DEMO_PASSWORD_ALPHABET[usize::from(byte & 0b11_1111)]))
        .collect())
}

fn build_connection_uri(host_port: u16, password: &str) -> String {
    format!("postgres://postgres:{password}@localhost:{host_port}/postgres?sslmode=disable")
}

async fn pull_image_if_missing<F>(
    docker: &Docker,
    image_name: &str,
    progress_callback: &mut F,
) -> Result<(), TestDataError>
where
    F: FnMut(String),
{
    progress_callback(format!("Pulling image if needed: {image_name}"));
    tracing::info!("Pulling Docker image if not present: {image_name}");

    let mut stream = docker.create_image(
        Some(
            CreateImageOptionsBuilder::default()
                .from_image(image_name)
                .build(),
        ),
        None,
        None,
    );

    while let Some(result) = stream.next().await {
        result.map(|_| ()).map_err(|e| {
            tracing::error!("Failed to pull image: {e}");
            TestDataError::ContainerStartFailed(format!("Failed to pull image: {e}"))
        })?;
    }

    Ok(())
}

async fn create_container_with_port<F>(
    docker: &Docker,
    image_name: &str,
    host_port: u16,
    container_name: &str,
    password: &str,
    progress_callback: &mut F,
) -> Result<String, TestDataError>
where
    F: FnMut(String),
{
    progress_callback("Image ready, creating container...".to_string());

    let config = build_container_config(image_name, host_port, password);

    let create_options = CreateContainerOptionsBuilder::default()
        .name(container_name)
        .build();

    let container = docker
        .create_container(Some(create_options), config)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create container: {e}");
            TestDataError::ContainerStartFailed(format!("Failed to create container: {e}"))
        })?;

    Ok(container.id)
}

fn build_container_config(image_name: &str, host_port: u16, password: &str) -> ContainerCreateBody {
    let env = vec![
        "POSTGRES_USER=postgres".to_string(),
        format!("POSTGRES_PASSWORD={password}"),
        "POSTGRES_DB=postgres".to_string(),
    ];

    ContainerCreateBody {
        image: Some(image_name.to_string()),
        env: Some(env),
        host_config: Some(build_host_config(host_port)),
        labels: Some(build_labels(host_port)),
        ..Default::default()
    }
}

async fn start_container(docker: &Docker, container_id: &str) -> Result<(), TestDataError> {
    docker
        .start_container(container_id, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to start container: {e}");
            TestDataError::ContainerStartFailed(format!("Failed to start container: {e}"))
        })
}

async fn remove_created_container(
    docker: &Docker,
    container_id: &str,
) -> Result<(), TestDataError> {
    docker
        .remove_container(
            container_id,
            Some(RemoveContainerOptionsBuilder::default().force(true).build()),
        )
        .await
        .map_err(|error| {
            TestDataError::ContainerStartFailed(format!(
                "Failed to remove test data container {container_id}: {error}"
            ))
        })
}

async fn cleanup_start_failure(
    docker: &Docker,
    container_id: &str,
    error: TestDataError,
) -> TestDataError {
    match remove_created_container(docker, container_id).await {
        Ok(()) => error,
        Err(cleanup_error) => TestDataError::ContainerStartFailed(format!(
            "{error}; cleanup of container {container_id} also failed: {cleanup_error}"
        )),
    }
}

/// Force-removes a test data container created by poqi.
///
/// # Errors
/// Returns an error if Docker cannot be reached or rejects the removal.
pub async fn remove_test_container(container_id: &str) -> Result<(), TestDataError> {
    let docker = Docker::connect_with_local_defaults().map_err(|error| {
        TestDataError::ContainerStartFailed(format!("Failed to connect to Docker: {error}"))
    })?;
    let inspect = docker
        .inspect_container(container_id, None)
        .await
        .map_err(|error| {
            TestDataError::ContainerStartFailed(format!(
                "Failed to inspect test data container {container_id}: {error}"
            ))
        })?;
    let labels = inspect.config.and_then(|config| config.labels);
    if !labels.as_ref().is_some_and(is_owned_testdata_labels) {
        return Err(TestDataError::ContainerStartFailed(format!(
            "Refusing to remove container {container_id}: it is not labelled as app-owned test data"
        )));
    }
    remove_created_container(&docker, container_id).await
}

fn is_owned_testdata_labels(labels: &HashMap<String, String>) -> bool {
    labels.get(TESTDATA_LABEL).map(String::as_str) == Some(TESTDATA_LABEL_VALUE)
        && labels.get(TESTDATA_LABEL_MANAGED_BY).map(String::as_str) == Some(TESTDATA_LABEL_VALUE)
}

async fn resolve_host_port(docker: &Docker, container_id: &str) -> Result<u16, TestDataError> {
    let inspect = docker
        .inspect_container(container_id, None)
        .await
        .map_err(|e| {
            tracing::error!("Failed to inspect container: {e}");
            TestDataError::ContainerStartFailed(format!("Failed to inspect container: {e}"))
        })?;

    inspect
        .network_settings
        .and_then(|ns| ns.ports)
        .and_then(|mut ports| ports.remove("5432/tcp"))
        .and_then(|bindings| bindings.and_then(|b| b.first().cloned()))
        .and_then(|binding| binding.host_port)
        .and_then(|port_str| port_str.parse::<u16>().ok())
        .ok_or_else(|| {
            tracing::error!("Failed to get host port from container");
            TestDataError::ContainerStartFailed(
                "Failed to get host port from container".to_string(),
            )
        })
}

async fn wait_for_postgres_ready<F>(
    connection_uri: &str,
    progress_callback: &mut F,
) -> Result<(), TestDataError>
where
    F: FnMut(String),
{
    wait_for_postgres_ready_with_policy(
        connection_uri,
        POSTGRES_READY_TIMEOUT,
        POSTGRES_RETRY_DELAY,
        progress_callback,
    )
    .await
}

async fn wait_for_postgres_ready_with_policy<F>(
    connection_uri: &str,
    timeout: Duration,
    retry_delay: Duration,
    progress_callback: &mut F,
) -> Result<(), TestDataError>
where
    F: FnMut(String),
{
    retry_until_ready(timeout, retry_delay, progress_callback, || async {
        let mut profile = ConnectionProfile::new("test-data-readiness", connection_uri);
        profile.max_pool_size = Some(1);
        profile.connect_timeout = Some(POSTGRES_CONNECT_TIMEOUT);
        let database = Database::new();
        database
            .connect(&profile)
            .await
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| {
        TestDataError::ContainerStartFailed(format!(
            "PostgreSQL did not become ready within {} seconds: {error}",
            timeout.as_secs_f32()
        ))
    })
}

async fn retry_until_ready<F, Fut, P>(
    timeout: Duration,
    retry_delay: Duration,
    progress_callback: &mut P,
    mut probe: F,
) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
    P: FnMut(String),
{
    let started = Instant::now();
    let mut attempts = 0_u32;
    let mut last_error = None;

    while started.elapsed() < timeout {
        attempts += 1;
        let remaining = timeout.saturating_sub(started.elapsed());
        match tokio::time::timeout(remaining, probe()).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => {
                last_error = Some("readiness probe timed out".to_string());
                break;
            }
        }

        if attempts == 1 || attempts.is_multiple_of(8) {
            progress_callback(format!(
                "Waiting for PostgreSQL readiness (attempt {attempts})..."
            ));
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break;
        }
        tokio::time::sleep(retry_delay.min(remaining)).await;
    }

    Err(last_error.unwrap_or_else(|| "readiness check did not complete".to_string()))
}

/// Starts a `PostgreSQL` test container using Docker.
///
/// # Errors
///
/// Returns `TestDataError::ContainerStartFailed` if the container fails to start.
pub async fn start_test_container() -> Result<TestContainer, TestDataError> {
    start_test_container_with_progress(|message| tracing::info!("{message}")).await
}

/// Starts a `PostgreSQL` test container using Docker, emitting progress updates through a callback.
///
/// # Errors
///
/// Returns `TestDataError::ContainerStartFailed` if the container fails to start.
pub async fn start_test_container_with_progress<F>(
    mut progress_callback: F,
) -> Result<TestContainer, TestDataError>
where
    F: FnMut(String),
{
    progress_callback("Starting PostgreSQL container...".to_string());
    tracing::info!("Starting PostgreSQL container...");

    let docker = Docker::connect_with_local_defaults().map_err(|e| {
        tracing::error!("Failed to connect to Docker: {e}");
        TestDataError::ContainerStartFailed(format!("Failed to connect to Docker: {e}"))
    })?;

    pull_image_if_missing(&docker, POSTGRES_IMAGE, &mut progress_callback).await?;
    let host_port = find_available_host_port(&docker).await?;
    progress_callback(format!("Reserving host port {host_port}"));
    let container_name = format!("poqi-testdb-{host_port}");
    let password = generate_demo_password()?;

    let container_id = create_container_with_port(
        &docker,
        POSTGRES_IMAGE,
        host_port,
        &container_name,
        &password,
        &mut progress_callback,
    )
    .await?;
    if let Err(error) = start_container(&docker, &container_id).await {
        return Err(cleanup_start_failure(&docker, &container_id, error).await);
    }

    progress_callback("Container started, checking PostgreSQL readiness...".to_string());
    let resolved_port = match resolve_host_port(&docker, &container_id).await {
        Ok(port) => port,
        Err(error) => {
            return Err(cleanup_start_failure(&docker, &container_id, error).await);
        }
    };
    let connection_uri = build_connection_uri(resolved_port, &password);

    if let Err(error) = wait_for_postgres_ready(&connection_uri, &mut progress_callback).await {
        return Err(cleanup_start_failure(&docker, &container_id, error).await);
    }

    progress_callback(format!("Container ready on port {resolved_port}"));
    tracing::info!("Container {container_id} started on port {resolved_port}");

    Ok(TestContainer {
        connection_uri,
        port: resolved_port,
        container_id,
    })
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::HashMap, net::TcpListener, time::Duration};

    use super::{
        build_connection_uri, build_container_config, generate_demo_password,
        is_owned_testdata_labels, remove_test_container, retry_until_ready,
        start_test_container_with_progress, wait_for_postgres_ready_with_policy,
        DEMO_PASSWORD_ALPHABET, DEMO_PASSWORD_LENGTH, POSTGRES_IMAGE, TESTDATA_LABEL,
        TESTDATA_LABEL_MANAGED_BY, TESTDATA_LABEL_VALUE,
    };
    use bollard::Docker;
    use poqi_db::{ConnectionProfile, Database};

    #[test]
    fn postgres_image_uses_a_pinned_sha256_digest() {
        let digest = POSTGRES_IMAGE
            .strip_prefix("postgres:18-alpine@sha256:")
            .expect("PostgreSQL image keeps its human-readable tag and pins a digest");

        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }

    #[test]
    fn demo_passwords_are_random_and_url_safe() {
        let first = generate_demo_password().expect("generate first demo password");
        let second = generate_demo_password().expect("generate second demo password");

        assert_eq!(first.len(), DEMO_PASSWORD_LENGTH);
        assert_eq!(second.len(), DEMO_PASSWORD_LENGTH);
        assert_ne!(first, second);
        assert!(first
            .bytes()
            .all(|byte| DEMO_PASSWORD_ALPHABET.contains(&byte)));
        assert!(second
            .bytes()
            .all(|byte| DEMO_PASSWORD_ALPHABET.contains(&byte)));
    }

    #[test]
    fn container_password_matches_explicit_non_tls_uri() {
        let password = generate_demo_password().expect("generate demo password");
        let host_port = 55_432;
        let config = build_container_config(POSTGRES_IMAGE, host_port, &password);
        let uri = build_connection_uri(host_port, &password);

        assert_eq!(config.image.as_deref(), Some(POSTGRES_IMAGE));
        assert!(config.env.as_ref().is_some_and(|env| env
            .iter()
            .any(|entry| entry == &format!("POSTGRES_PASSWORD={password}"))));
        assert_eq!(
            uri,
            format!(
                "postgres://postgres:{password}@localhost:{host_port}/postgres?sslmode=disable"
            )
        );
        assert!(config
            .labels
            .as_ref()
            .is_some_and(|labels| labels.values().all(|value| !value.contains(&password))));
    }

    #[tokio::test]
    async fn readiness_retries_transient_failures_until_probe_succeeds() {
        let attempts = Cell::new(0_u32);
        let mut progress = Vec::new();

        retry_until_ready(
            Duration::from_secs(1),
            Duration::ZERO,
            &mut |message| progress.push(message),
            || {
                let attempt = attempts.get() + 1;
                attempts.set(attempt);
                async move {
                    if attempt < 3 {
                        Err(format!("not ready on attempt {attempt}"))
                    } else {
                        Ok(())
                    }
                }
            },
        )
        .await
        .expect("third readiness probe succeeds");

        assert_eq!(attempts.get(), 3);
        assert_eq!(progress.len(), 1);
        assert!(progress[0].contains("attempt 1"));
    }

    #[tokio::test]
    async fn readiness_timeout_interrupts_a_hung_probe() {
        let started = std::time::Instant::now();
        let mut progress = |_| {};

        let error = retry_until_ready(
            Duration::from_millis(10),
            Duration::ZERO,
            &mut progress,
            || async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(())
            },
        )
        .await
        .expect_err("hung probe must time out");

        assert_eq!(error, "readiness probe timed out");
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn closed_local_endpoint_respects_short_readiness_deadline() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve local port");
        let port = listener.local_addr().expect("local address").port();
        let uri = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
        let started = std::time::Instant::now();
        let mut progress = |_| {};

        let error = wait_for_postgres_ready_with_policy(
            &uri,
            Duration::from_millis(50),
            Duration::from_millis(5),
            &mut progress,
        )
        .await
        .expect_err("non-PostgreSQL listener must not become ready");

        drop(listener);
        assert!(error.to_string().contains("did not become ready"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn cleanup_ownership_requires_both_poqi_labels() {
        let mut labels = HashMap::new();
        labels.insert(TESTDATA_LABEL.to_string(), TESTDATA_LABEL_VALUE.to_string());
        assert!(!is_owned_testdata_labels(&labels));

        labels.insert(
            TESTDATA_LABEL_MANAGED_BY.to_string(),
            TESTDATA_LABEL_VALUE.to_string(),
        );
        assert!(is_owned_testdata_labels(&labels));
    }

    #[tokio::test]
    async fn opt_in_container_is_query_ready_and_removable() {
        if !matches!(
            std::env::var("POQI_ENABLE_TESTCONTAINERS").map(|value| value.to_ascii_lowercase()),
            Ok(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
        ) {
            eprintln!(
                "skipping poqi-testdata Docker regression; set POQI_ENABLE_TESTCONTAINERS=1 to enable"
            );
            return;
        }

        let container = start_test_container_with_progress(|_| {})
            .await
            .expect("start ready PostgreSQL container");
        let docker = Docker::connect_with_local_defaults().expect("connect to Docker");
        let inspect = docker
            .inspect_container(&container.container_id, None)
            .await;
        let mut profile = ConnectionProfile::new("testdata-integration", &container.connection_uri);
        profile.max_pool_size = Some(1);
        profile.connect_timeout = Some(Duration::from_secs(2));
        let database = Database::new();
        let readiness = database.connect(&profile).await;

        let old_password_uri = format!(
            "postgres://postgres:postgres@localhost:{}/postgres?sslmode=disable",
            container.port
        );
        let mut old_password_profile =
            ConnectionProfile::new("testdata-old-password", &old_password_uri);
        old_password_profile.max_pool_size = Some(1);
        old_password_profile.connect_timeout = Some(Duration::from_secs(2));
        let old_password_result = Database::new().connect(&old_password_profile).await;
        let cleanup = remove_test_container(&container.container_id).await;

        readiness.expect("returned container accepts SELECT 1 health check");
        old_password_result.expect_err("the former predictable demo password must be rejected");
        let inspect = inspect.expect("inspect newly created PostgreSQL container");
        let uri_password = container
            .connection_uri
            .strip_prefix("postgres://postgres:")
            .and_then(|value| value.split_once('@'))
            .map(|(password, _)| password)
            .expect("returned demo URI contains its password");
        assert!(inspect
            .config
            .and_then(|config| config.env)
            .is_some_and(|env| env
                .iter()
                .any(|entry| entry == &format!("POSTGRES_PASSWORD={uri_password}"))));
        cleanup.expect("returned poqi-owned container is removable");
    }
}
