use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use poqi_test_support::{should_run_integration_tests, start_postgres_container};
use tokio::io::AsyncReadExt;

#[test]
fn validates_a_well_formed_uri_without_connecting() {
    validate_connection_uri("postgres://user:password@localhost:5432/app?sslmode=disable")
        .expect("URI validation should not open a network connection");
}

#[test]
fn validates_a_multihost_uri_without_rewriting_it_as_a_keyword_dsn() {
    validate_connection_uri(
        "postgresql://user@host1:1234,host2:5432/db?sslmode=require&application_name=poqi",
    )
    .expect("tokio-postgres multihost URLs should remain URL inputs");
}

#[test]
fn malformed_uri_hint_does_not_disclose_its_password() {
    let password = "not-for-display";
    let error = validate_connection_uri(&format!(
        "postgres://user:{password}@localhost:not-a-port/app?sslmode=disable"
    ))
    .expect_err("malformed port should be rejected");

    assert!(matches!(error, DatabaseError::InvalidUri(_)));
    assert!(!error.connection_hint().contains(password));
}

#[test]
fn validation_checks_client_auth_pairing_and_ca_tls_mode() {
    let missing_key = validate_connection_uri(
        "postgres://user:password@localhost/app?sslmode=require&sslcert=client.pem",
    );
    assert!(matches!(
        missing_key,
        Err(DatabaseError::TlsClientAuthIncomplete)
    ));

    let disabled_tls_with_ca = validate_connection_uri(
        "postgres://user:password@localhost/app?sslmode=disable&sslrootcert=ca.pem",
    );
    assert!(matches!(
        disabled_tls_with_ca,
        Err(DatabaseError::TlsRootCertWithDisabledTls)
    ));
}

fn profile_from_port(port: u16) -> ConnectionProfile {
    ConnectionProfile {
        name: "test".to_string(),
        uri: format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"),
        // Integration tests concurrently borrow multiple clients (e.g. to terminate a
        // backend connection) so the pool needs enough capacity to hand out at least
        // two connections without blocking forever.
        max_pool_size: Some(4),
        connect_timeout: Some(Duration::from_secs(5)),
    }
}

#[tokio::test]
async fn connection_hints_classify_login_and_missing_database_errors() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);

    Database::new()
        .connect(&profile)
        .await
        .expect("known container credentials should connect");

    let supplied_secret = "intentionally-wrong-password";
    let wrong_password = ConnectionProfile {
        name: "wrong password".to_string(),
        uri: format!(
            "postgres://postgres:{supplied_secret}@127.0.0.1:{port}/postgres?sslmode=disable"
        ),
        ..profile.clone()
    };
    let authentication_error = Database::new()
        .connect(&wrong_password)
        .await
        .expect_err("wrong password should be rejected");
    let authentication_hint = authentication_error.connection_hint();
    assert!(authentication_hint.contains("user name and password"));
    assert!(!authentication_hint.contains(supplied_secret));

    let missing_database = ConnectionProfile {
        name: "missing database".to_string(),
        uri: format!("postgres://postgres:postgres@127.0.0.1:{port}/this_database_does_not_exist?sslmode=disable"),
        ..profile
    };
    let database_error = Database::new()
        .connect(&missing_database)
        .await
        .expect_err("missing database should be rejected");
    assert!(database_error
        .connection_hint()
        .contains("requested PostgreSQL database"));
}

#[test]
fn database_new_returns_default() {
    let db = Database::new();
    let default_db = Database::default();
    assert_eq!(format!("{db:?}"), format!("{default_db:?}"));
}

#[tokio::test]
async fn connect_initializes_pool_and_recovers_after_termination() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);
    let db = Database::new();

    db.connect(&profile).await.expect("should connect");
    db.ping().await.expect("initial ping");

    db.batch_execute("CREATE TEMP TABLE poqi_health (id INT PRIMARY KEY)")
        .await
        .expect("create temp table");

    let client = db.acquire().await.expect("client from pool");
    let pid_row = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .expect("pid query");
    let pid: i32 = pid_row.get(0);

    let killer = db.acquire().await.expect("second client");
    killer
        .execute("SELECT pg_terminate_backend($1)", &[&pid])
        .await
        .expect("terminate backend");

    // This case exercises recycling a connection known to be closed before dispatch.
    // A request racing termination may instead report an unknown outcome without replay;
    // connection_loss_does_not_replay_operation_and_next_request_recovers covers that case.
    tokio::time::timeout(Duration::from_secs(3), async {
        while !client.is_closed() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("terminated connection should close before returning to the pool");

    drop(client);
    drop(killer);

    db.query_one("SELECT 1", &[])
        .await
        .expect("query after reconnect");
}

#[tokio::test]
async fn connection_loss_does_not_replay_operation_and_next_request_recovers() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let db = Database::new();
    db.connect(&profile_from_port(port))
        .await
        .expect("should connect");

    let invocations = Arc::new(AtomicUsize::new(0));
    let operation_db = db.clone();
    let operation_invocations = invocations.clone();
    let result = db
        .with_client(move |client| async move {
            let invocation = operation_invocations.fetch_add(1, Ordering::SeqCst);
            if invocation > 0 {
                return Ok(());
            }

            let pid: i32 = client
                .query_one("SELECT pg_backend_pid()", &[])
                .await?
                .get(0);
            let killer = operation_db
                .acquire()
                .await
                .expect("acquire termination client");
            killer
                .execute("SELECT pg_terminate_backend($1)", &[&pid])
                .await?;
            drop(killer);

            client.query_one("SELECT 1", &[]).await?;
            Ok(())
        })
        .await;

    assert!(matches!(result, Err(DatabaseError::ConnectionLost(_))));
    assert_eq!(invocations.load(Ordering::SeqCst), 1);
    db.query_one("SELECT 1", &[])
        .await
        .expect("later request should use a healthy connection");
}

#[tokio::test]
async fn cancel_current_stops_server_query_and_discards_connection() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let db = Database::new();
    db.connect(&profile_from_port(port))
        .await
        .expect("should connect");

    let observer = db.acquire().await.expect("acquire cancellation observer");
    let query_db = db.clone();
    let query = tokio::spawn(async move { query_db.query_one("SELECT pg_sleep(10)", &[]).await });
    // Registering a cancel token precedes the server receiving the SQL. Observe
    // execution first so this test exercises server cancellation, not that race.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let running: bool = observer
                .query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                     WHERE query = $1 AND state = 'active' AND wait_event = 'PgSleep')",
                    &[&"SELECT pg_sleep(10)"],
                )
                .await
                .expect("observe running server query")
                .get(0);
            if running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server query must begin before testing cancellation");
    drop(observer);
    let started = Instant::now();
    cancel_when_registered(&db).await;

    let result = tokio::time::timeout(Duration::from_secs(3), query)
        .await
        .expect("server query should stop promptly")
        .expect("query task should not panic");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(matches!(
        result,
        Err(DatabaseError::Postgres(error))
            if error.code() == Some(&SqlState::QUERY_CANCELED)
    ));
    db.query_one("SELECT 1", &[])
        .await
        .expect("canceled connection must not prevent later queries");
}

#[tokio::test]
async fn cancellation_while_waiting_for_pool_prevents_dispatch() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let mut profile = profile_from_port(port);
    profile.max_pool_size = Some(1);
    let db = Database::new();
    db.connect(&profile).await.expect("should connect");

    let held_client = db.acquire().await.expect("hold only pooled client");
    let query_db = db.clone();
    let query = tokio::spawn(async move { query_db.query_one("SELECT pg_sleep(10)", &[]).await });
    cancel_when_registered(&db).await;
    assert!(
        db.cancel_current()
            .await
            .expect("repeat pending cancellation"),
        "the pending generation remains canceled until acquisition settles"
    );
    drop(held_client);

    let result = query.await.expect("query task should not panic");
    assert!(matches!(result, Err(DatabaseError::OperationCanceled)));
    db.query_one("SELECT 1", &[])
        .await
        .expect("pool remains usable after pre-dispatch cancellation");
}

#[tokio::test]
async fn concurrent_operations_keep_independent_cancellation_generations() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let mut profile = profile_from_port(port);
    profile.max_pool_size = Some(2);
    let db = Database::new();
    db.connect(&profile).await.expect("should connect");

    let held_one = db.acquire().await.expect("hold first pooled client");
    let held_two = db.acquire().await.expect("hold second pooled client");
    let first_db = db.clone();
    let first = tokio::spawn(async move { first_db.query_one("SELECT 1", &[]).await });
    let second_db = db.clone();
    let second = tokio::spawn(async move { second_db.query_one("SELECT 2", &[]).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    drop(held_one);
    drop(held_two);

    let first_value: i32 = first
        .await
        .expect("first task")
        .expect("first operation")
        .get(0);
    let second_value: i32 = second
        .await
        .expect("second task")
        .expect("second operation")
        .get(0);
    assert_eq!((first_value, second_value), (1, 2));
}

async fn cancel_when_registered(db: &Database) {
    for _ in 0..100 {
        if db.cancel_current().await.expect("send cancellation") {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("database operation did not register for cancellation");
}

#[tokio::test]
async fn query_and_prepared_statement_round_trip() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);
    let db = Database::new();

    db.connect(&profile).await.expect("should connect");

    db.batch_execute(
        "CREATE TEMP TABLE poqi_prepare (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .await
    .expect("create table");

    let inserted = db
        .execute(
            "INSERT INTO poqi_prepare (name) VALUES ($1), ($2)",
            &[&"alpha", &"beta"],
        )
        .await
        .expect("insert rows");
    assert_eq!(inserted, 2);

    let rows = db
        .query(
            "SELECT name FROM poqi_prepare WHERE name LIKE $1 ORDER BY id",
            &[&"%a%"],
        )
        .await
        .expect("query rows");
    let names: Vec<String> = rows.into_iter().map(|row| row.get(0)).collect();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);

    let client = db.acquire().await.expect("prepared client");
    let statement = client
        .prepare_cached("SELECT name FROM poqi_prepare WHERE name = $1 ORDER BY id")
        .await
        .expect("prepare statement");
    let prepared_rows = client
        .query(&statement, &[&"beta"])
        .await
        .expect("query via prepared statement");
    assert_eq!(prepared_rows.len(), 1);
    assert_eq!(prepared_rows[0].get::<_, String>(0), "beta");
}

#[tokio::test]
async fn execute_reuses_cached_statement_on_same_connection() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);
    let db = Database::new();

    db.connect(&profile).await.expect("should connect");

    db.batch_execute(
        "CREATE TEMP TABLE poqi_prepare_cache (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .await
    .expect("create table");

    db.execute(
        "INSERT INTO poqi_prepare_cache (name) VALUES ($1)",
        &[&"first"],
    )
    .await
    .expect("first insert");
    db.execute(
        "INSERT INTO poqi_prepare_cache (name) VALUES ($1)",
        &[&"second"],
    )
    .await
    .expect("second insert");

    let cached_count = db
        .with_client(|client| async move {
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM pg_prepared_statements WHERE statement = $1",
                    &[&"INSERT INTO poqi_prepare_cache (name) VALUES ($1)"],
                )
                .await?;
            Ok::<i64, tokio_postgres::Error>(row.get(0))
        })
        .await
        .expect("inspect prepared statements");

    assert_eq!(cached_count, 1);
}

#[tokio::test]
async fn copy_out_csv_streams_table_rows() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);
    let db = Database::new();

    db.connect(&profile).await.expect("should connect");

    db.batch_execute(
        "CREATE TEMP TABLE poqi_copy (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .await
    .expect("create table");

    db.execute(
        "INSERT INTO poqi_copy (name) VALUES ($1), ($2)",
        &[&"alpha", &"beta"],
    )
    .await
    .expect("insert rows");

    let csv_bytes = db
        .copy_out_csv(
            "COPY (
                SELECT id, name FROM poqi_copy ORDER BY id
            ) TO STDOUT WITH (FORMAT csv, HEADER true)",
            &[],
        )
        .await
        .expect("copy out csv");

    let csv = String::from_utf8(csv_bytes).expect("utf8 csv");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "id,name");
    assert_eq!(lines[1], "1,alpha");
    assert_eq!(lines[2], "2,beta");
}

#[tokio::test]
async fn copy_out_csv_into_writes_to_async_writer() {
    if !should_run_integration_tests("poqi-db") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = profile_from_port(port);
    let db = Database::new();

    db.connect(&profile).await.expect("should connect");

    db.batch_execute(
        "CREATE TEMP TABLE poqi_copy_into (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )",
    )
    .await
    .expect("create table");

    db.execute(
        "INSERT INTO poqi_copy_into (name) VALUES ($1), ($2)",
        &[&"alpha", &"beta"],
    )
    .await
    .expect("insert rows");

    let (writer, mut reader) = tokio::io::duplex(4096);
    let mut writer = Box::pin(writer);
    let bytes_copied = db
        .copy_out_csv_into(
            "COPY (
                SELECT id, name FROM poqi_copy_into ORDER BY id
            ) TO STDOUT WITH (FORMAT csv, HEADER true)",
            &[],
            writer.as_mut(),
        )
        .await
        .expect("copy out into writer");

    drop(writer);

    let mut csv_bytes = Vec::new();
    reader
        .read_to_end(&mut csv_bytes)
        .await
        .expect("read csv bytes");

    assert_eq!(
        usize::try_from(bytes_copied).expect("fits usize"),
        csv_bytes.len()
    );

    let csv = String::from_utf8(csv_bytes).expect("utf8 csv");
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "id,name");
    assert_eq!(lines[1], "1,alpha");
    assert_eq!(lines[2], "2,beta");
}
