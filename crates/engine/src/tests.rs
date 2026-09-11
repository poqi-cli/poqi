use super::{query_analysis, *};
use poqi_config::AppConfig;
use poqi_db::ConnectionProfile;
use poqi_test_support::{should_run_integration_tests, start_postgres_container};

fn unqualified(name: &str) -> QualifiedRelation {
    QualifiedRelation::unqualified(name)
}

fn qualified(schema: &str, name: &str) -> QualifiedRelation {
    QualifiedRelation::in_schema(schema, name)
}

fn simple_qualified(value: &str) -> QualifiedRelation {
    let (schema, name) = value.split_once('.').expect("qualified test relation");
    qualified(schema, name)
}

#[test]
fn stub_engine_execute_returns_not_implemented() {
    let engine = StubEngine;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let err = runtime
        .block_on(engine.execute("select 1"))
        .expect_err("stub execute should error");
    let engine_err = err
        .downcast_ref::<EngineError>()
        .expect("error should downcast to EngineError");
    assert!(matches!(engine_err, EngineError::NotImplemented));
}

#[test]
fn stub_engine_crud_returns_not_implemented() {
    let engine = StubEngine;
    let action = CrudAction::SelectTop {
        table: qualified("public", "users"),
        limit: 25,
        relation_kind: RelationKind::Table,
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let err = runtime
        .block_on(engine.crud(action))
        .expect_err("stub crud should error");
    let engine_err = err
        .downcast_ref::<EngineError>()
        .expect("error should downcast to EngineError");
    assert!(matches!(engine_err, EngineError::NotImplemented));
}

#[test]
fn cancel_current_is_noop() {
    let engine = StubEngine;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    assert!(!runtime
        .block_on(engine.cancel_current())
        .expect("stub cancellation"));
}

#[test]
fn plan_run_sql_detects_simple_query() {
    let plan = query_analysis::plan_run_sql("select * from public.demo").expect("plan");
    match plan.origin {
        ResultOrigin::RunSql {
            refresh: RunSqlRefresh::SimpleSingleTable { ref table },
            ..
        } => assert_eq!(table, &qualified("public", "demo")),
        other => panic!("unexpected origin: {other:?}"),
    }
    assert!(plan
        .editable_sql
        .as_deref()
        .is_some_and(|sql| { sql.contains(CTID_ALIAS) && sql.contains(TABLEOID_ALIAS) }));
    assert_eq!(plan.source_table, Some(qualified("public", "demo")));
}

#[test]
fn plan_run_sql_marks_filtered_query_complex() {
    let plan =
        query_analysis::plan_run_sql("select id from public.demo where id > 10").expect("plan");
    match plan.origin {
        ResultOrigin::RunSql {
            refresh: RunSqlRefresh::ComplexSingleTable { ref table },
            ..
        } => assert_eq!(table, &qualified("public", "demo")),
        other => panic!("unexpected origin: {other:?}"),
    }
    assert!(plan.editable_sql.is_none());
    assert!(plan.source_columns.is_none());
}

#[test]
fn plan_run_sql_preserves_alias_source_column() {
    let plan = query_analysis::plan_run_sql("SELECT a AS b FROM public.demo").expect("plan");
    let columns = plan
        .source_columns
        .expect("source projection")
        .resolve(&["b".to_string()]);
    assert_eq!(columns, vec![Some("a".to_string())]);
}

#[test]
fn plan_run_sql_keeps_row_membership_queries_read_only() {
    for sql in [
        "SELECT a FROM public.demo LIMIT 1",
        "SELECT a FROM public.demo OFFSET 1",
        "SELECT a FROM public.demo FOR UPDATE",
    ] {
        let plan = query_analysis::plan_run_sql(sql).expect("plan");
        assert!(plan.editable_sql.is_none(), "unexpected rewrite for {sql}");
        assert!(
            plan.source_columns.is_none(),
            "unexpected mapping for {sql}"
        );
    }
}

#[test]
fn quote_type_name_reconstructs_only_identifier_components() {
    assert_eq!(
        query_analysis::quote_type_name("pg_catalog.text").as_deref(),
        Some("\"pg_catalog\".\"text\"")
    );
    assert_eq!(
        query_analysis::quote_type_name("\"types.with.dot\".\"status.type\"").as_deref(),
        Some("\"types.with.dot\".\"status.type\"")
    );
    assert_eq!(
        query_analysis::quote_type_name("\"types\"\"quoted\".\"status\"\"type\"").as_deref(),
        Some("\"types\"\"quoted\".\"status\"\"type\"")
    );

    for unsafe_type in [
        "pg_catalog.text WHERE true",
        "pg_catalog.text; SELECT 1",
        "one.two.three",
        "text[]",
        "numeric(10)",
    ] {
        assert!(
            query_analysis::quote_type_name(unsafe_type).is_none(),
            "unexpected accepted type: {unsafe_type}"
        );
    }
}

#[test]
fn plan_run_sql_retains_quoted_relation_identifiers() {
    let plan = query_analysis::plan_run_sql(
        "SELECT \"CamelColumn\" AS shown FROM \"Mixed Schema\".\"Mixed Table\"",
    )
    .expect("plan");
    let relation = plan.source_table.expect("source relation");
    assert_eq!(relation.schema.as_deref(), Some("Mixed Schema"));
    assert_eq!(relation.name, "Mixed Table");
    assert!(plan.editable_sql.is_some());

    let dotted =
        query_analysis::plan_run_sql("SELECT value FROM \"schema.with.dot\".\"table.with.dot\"")
            .expect("dotted plan");
    assert!(dotted.editable_sql.is_some());
    assert_eq!(
        dotted.source_table,
        Some(qualified("schema.with.dot", "table.with.dot"))
    );
}

#[test]
fn plan_run_sql_keeps_semantically_sensitive_selects_read_only() {
    for sql in [
        "SELECT count(*) FROM public.demo",
        "SELECT DISTINCT a FROM public.demo",
        "SELECT a FROM public.demo ORDER BY 1",
        "SELECT row_number() OVER (), a FROM public.demo",
        "SELECT a + 1 FROM public.demo",
        "SELECT renamed FROM public.demo AS d(renamed)",
    ] {
        let plan = query_analysis::plan_run_sql(sql).expect("plan");
        assert!(plan.editable_sql.is_none(), "unexpected rewrite for {sql}");
        assert!(
            plan.source_columns.is_none(),
            "unexpected mapping for {sql}"
        );
    }
}

#[test]
fn plan_run_sql_rejects_statement_batches() {
    let error = query_analysis::plan_run_sql("SELECT 1; DELETE FROM public.demo")
        .expect_err("batch must be rejected");
    assert!(matches!(error, EngineError::UnsupportedSqlBatch(2)));
}

#[tokio::test]
async fn query_error_displays_postgres_message_through_engine_wrapper() {
    if !should_run_integration_tests("poqi-engine") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let profile = ConnectionProfile::new(
        "error-display",
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"),
    );
    let database = Database::new();
    database.connect(&profile).await.expect("connect");
    let engine = DatabaseEngine::new(
        database,
        AppConfig::default().db.defaults.statement_timeout,
        50,
    );

    for (sql, expected_code) in [(r#"SELECT "hello""#, "42703"), ("SELECT 1 / 0", "22012")] {
        let error = engine.execute(sql).await.expect_err("invalid query");
        let postgres = error
            .chain()
            .find_map(|source| source.downcast_ref::<tokio_postgres::Error>())
            .expect("PostgreSQL source remains available");
        let server_error = postgres.as_db_error().expect("server error");
        assert_eq!(server_error.code().code(), expected_code);
        let displayed = error.to_string();
        assert!(displayed.contains(server_error.message()), "{displayed}");
        assert!(displayed.contains(expected_code), "{displayed}");
    }

    let result = engine
        .execute("SELECT 'hello'")
        .await
        .expect("text literal");
    assert_eq!(result.rows[0][0], "hello");
}

#[tokio::test]
async fn database_engine_select_top_round_trip() {
    if !should_run_integration_tests("poqi-engine") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let mut profile = ConnectionProfile::new(
        "integration",
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"),
    );
    profile.max_pool_size = Some(1);

    let database = Database::new();
    database
        .connect(&profile)
        .await
        .expect("connect to postgres");

    database
        .batch_execute("CREATE TEMP TABLE poqi_engine (id SERIAL PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .expect("create temp table");

    database
        .execute(
            "INSERT INTO poqi_engine (name) VALUES ($1), ($2), ($3)",
            &[&"alpha", &"beta", &"gamma"],
        )
        .await
        .expect("insert rows");

    let engine = DatabaseEngine::new(
        database,
        AppConfig::default().db.defaults.statement_timeout,
        50,
    );
    let result = engine
        .crud(CrudAction::SelectTop {
            table: unqualified("poqi_engine"),
            limit: 2,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("select top");

    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn database_engine_select_top_respects_requested_limit() {
    if !should_run_integration_tests("poqi-engine") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let mut profile = ConnectionProfile::new(
        "integration",
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"),
    );
    profile.max_pool_size = Some(1);

    let database = Database::new();
    database
        .connect(&profile)
        .await
        .expect("connect to postgres");

    database
        .batch_execute("CREATE TEMP TABLE poqi_engine_limit (id SERIAL PRIMARY KEY)")
        .await
        .expect("create temp table");

    let engine = DatabaseEngine::new(
        database,
        AppConfig::default().db.defaults.statement_timeout,
        50,
    );
    let result = engine
        .crud(CrudAction::SelectTop {
            table: unqualified("poqi_engine_limit"),
            limit: 200,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("select top");

    match result.metadata.origin {
        ResultOrigin::SelectTop { table, limit } => {
            assert_eq!(table, unqualified("poqi_engine_limit"));
            assert_eq!(limit, 200);
        }
        other => panic!("unexpected origin: {other:?}"),
    }
}

#[tokio::test]
async fn database_engine_preserves_queries_and_targets_physical_rows() {
    if !should_run_integration_tests("poqi-engine") {
        return;
    }

    let (_node, port) = start_postgres_container().await;
    let mut profile = ConnectionProfile::new(
        "engine-safety",
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable"),
    );
    profile.max_pool_size = Some(2);

    let database = Database::new();
    database
        .connect(&profile)
        .await
        .expect("connect to postgres");
    prepare_engine_safety_fixture(&database).await;

    let engine = DatabaseEngine::new(
        database.clone(),
        AppConfig::default().db.defaults.statement_timeout,
        50,
    );

    assert_read_only_queries_are_unchanged(&engine).await;
    assert_alias_update_and_batch_rejection(&engine, &database).await;
    assert_quoted_identifiers_are_editable(&engine, &database).await;
    assert_dotted_identifiers_are_editable(&engine, &database).await;
    assert_null_provenance_and_updates(&engine, &database).await;
    assert_view_and_oid_decoding(&engine).await;
    assert_metadata_name_collisions_are_preserved(&engine).await;
    assert_wrong_row_update_is_rejected(&engine, &database).await;
    assert_wrong_row_delete_is_rejected(&engine, &database).await;
    assert_stale_version_is_rejected(&engine, &database).await;
    assert_composite_key_and_refresh(&engine, &database).await;
    assert_custom_type_names_round_trip(&engine, &database).await;
    assert_no_key_and_acl_reads_remain_available(&engine, &database).await;
    assert_automatic_view_preview_is_read_only(&engine, &database).await;
    assert_relation_name_is_bound(&engine, &database).await;
    assert_statement_timeout_uses_query_canceled_code(&database).await;

    assert_partition_identity_is_scoped(
        &engine,
        &database,
        "engine_review.partition_parent",
        RelationKind::PartitionedTable,
    )
    .await;
}

async fn prepare_engine_safety_fixture(database: &Database) {
    database
        .batch_execute(
            "CREATE SCHEMA engine_review;
             CREATE TABLE engine_review.alias_target (a INT PRIMARY KEY, b INT NOT NULL);
             INSERT INTO engine_review.alias_target VALUES (11, 20);
             CREATE SCHEMA \"Mixed Schema\";
             CREATE TABLE \"Mixed Schema\".\"Mixed Table\" (\"CamelColumn\" INT PRIMARY KEY);
             INSERT INTO \"Mixed Schema\".\"Mixed Table\" VALUES (41);
             CREATE SCHEMA \"schema.with.dot\";
             CREATE TABLE \"schema.with.dot\".\"table.with.dot\" (\"column.with.dot\" TEXT PRIMARY KEY);
             INSERT INTO \"schema.with.dot\".\"table.with.dot\" VALUES ('before');
             CREATE TABLE engine_review.null_values (id INT PRIMARY KEY, value TEXT);
             INSERT INTO engine_review.null_values VALUES (1, NULL), (2, 'NULL');
             CREATE TABLE engine_review.ordering (a INT NOT NULL);
             INSERT INTO engine_review.ordering VALUES (2), (1), (1);
             CREATE TABLE engine_review.metadata_collision (
                 poqi__tableoid TEXT PRIMARY KEY,
                 poqi__ctid TEXT NOT NULL
             );
             INSERT INTO engine_review.metadata_collision VALUES ('business oid', 'business ctid');
             CREATE VIEW engine_review.ordering_view AS SELECT a FROM engine_review.ordering;
             CREATE TABLE engine_review.partition_parent (
                 partition_key INT,
                 value TEXT,
                 PRIMARY KEY (partition_key)
             )
                 PARTITION BY LIST (partition_key);
             CREATE TABLE engine_review.partition_one
                 PARTITION OF engine_review.partition_parent FOR VALUES IN (1);
             CREATE TABLE engine_review.partition_two
                 PARTITION OF engine_review.partition_parent FOR VALUES IN (2);
             INSERT INTO engine_review.partition_parent VALUES (1, 'one'), (2, 'two');
             CREATE TABLE engine_review.inherited_parent (value TEXT);
             CREATE TABLE engine_review.inherited_child ()
                  INHERITS (engine_review.inherited_parent);
             INSERT INTO engine_review.inherited_parent VALUES ('parent');
             INSERT INTO engine_review.inherited_child VALUES ('child');
             CREATE TABLE engine_review.wrong_update (id INT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO engine_review.wrong_update VALUES (1, 'first'), (2, 'second');
             CREATE TABLE engine_review.wrong_delete (id INT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO engine_review.wrong_delete VALUES (1, 'first'), (2, 'second');
             CREATE TABLE engine_review.stale_version (id INT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO engine_review.stale_version VALUES (1, 'before');
             CREATE TABLE engine_review.composite_key (
                 tenant_id INT,
                 item_id INT,
                 value TEXT NOT NULL,
                 PRIMARY KEY (tenant_id, item_id)
             );
             INSERT INTO engine_review.composite_key VALUES (7, 9, 'before');
             CREATE SCHEMA \"types.with.dot\";
             CREATE TYPE \"types.with.dot\".\"status.type\" AS ENUM ('before', 'after');
             CREATE SCHEMA \"types\"\"quoted\";
             CREATE TYPE \"types\"\"quoted\".\"status\"\"type\" AS ENUM ('before', 'after');
             CREATE TABLE engine_review.custom_types (
                 id INT PRIMARY KEY,
                 dotted \"types.with.dot\".\"status.type\" NOT NULL,
                 quoted \"types\"\"quoted\".\"status\"\"type\" NOT NULL
             );
             INSERT INTO engine_review.custom_types VALUES (1, 'before', 'before');
             CREATE TABLE engine_review.no_key (shown TEXT NOT NULL);
             INSERT INTO engine_review.no_key VALUES ('readable');
             CREATE TABLE engine_review.preview_side_effect_log (value INT NOT NULL);
             CREATE FUNCTION engine_review.preview_side_effect() RETURNS INT
                 LANGUAGE plpgsql AS $$
                 BEGIN
                     INSERT INTO engine_review.preview_side_effect_log VALUES (1);
                     RETURN 1;
                 END
                 $$;
             CREATE VIEW engine_review.side_effect_view AS
                 SELECT engine_review.preview_side_effect() AS value;
             CREATE TABLE engine_review.restricted_key (id INT PRIMARY KEY, shown TEXT NOT NULL);
             INSERT INTO engine_review.restricted_key VALUES (1, 'visible');
             CREATE ROLE engine_limited;
             GRANT USAGE ON SCHEMA engine_review TO engine_limited;
             GRANT SELECT (shown) ON engine_review.restricted_key TO engine_limited;
             CREATE TABLE engine_review.\"backslash\\quote'table\" (
                 \"id\\key'\" INT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             INSERT INTO engine_review.\"backslash\\quote'table\" VALUES (1, 'before');",
        )
        .await
        .expect("create safety fixtures");
}

#[test]
fn query_cancellation_classifier_handles_predispatch_cancellation() {
    let canceled = anyhow::Error::new(EngineError::Database(DatabaseError::OperationCanceled));
    assert!(is_query_canceled(&canceled));

    let unrelated = anyhow::Error::msg("not a database cancellation");
    assert!(!is_query_canceled(&unrelated));
}

async fn assert_metadata_name_collisions_are_preserved(engine: &DatabaseEngine) {
    let result = engine
        .execute("SELECT * FROM engine_review.metadata_collision")
        .await
        .expect("metadata collision query");
    assert_eq!(
        result.columns,
        vec!["poqi__tableoid".to_string(), "poqi__ctid".to_string()]
    );
    assert_eq!(
        result.rows,
        vec![vec![
            "business oid".to_string(),
            "business ctid".to_string()
        ]]
    );
    assert_eq!(
        result.metadata.source_columns,
        vec![Some("poqi__tableoid".into()), Some("poqi__ctid".into())]
    );
    assert!(result.metadata.row_identities[0].is_some());
}

async fn assert_quoted_identifiers_are_editable(engine: &DatabaseEngine, database: &Database) {
    let result = engine
        .execute("SELECT \"CamelColumn\" AS shown FROM \"Mixed Schema\".\"Mixed Table\"")
        .await
        .expect("quoted relation query");
    assert_eq!(
        result.metadata.source_columns,
        vec![Some("CamelColumn".into())]
    );
    let row_identity = result.metadata.row_identities[0]
        .clone()
        .expect("quoted relation row identity");

    engine
        .crud(CrudAction::UpdateCell {
            table: qualified("Mixed Schema", "Mixed Table"),
            column: "CamelColumn".into(),
            new_value: Some("42".into()),
            row_identity,
            column_type: Some("pg_catalog.int4".into()),
        })
        .await
        .expect("quoted relation update");
    let rows = database
        .query(
            "SELECT \"CamelColumn\" FROM \"Mixed Schema\".\"Mixed Table\"",
            &[],
        )
        .await
        .expect("read quoted relation");
    assert_eq!(rows[0].get::<_, i32>(0), 42);
}

async fn assert_dotted_identifiers_are_editable(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("schema.with.dot", "table.with.dot");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("dotted relation preview");
    assert_eq!(preview.rows, vec![vec!["before".to_string()]]);
    let identity = preview.metadata.row_identities[0]
        .clone()
        .expect("dotted relation identity");
    engine
        .crud(CrudAction::UpdateCell {
            table: relation.clone(),
            column: "column.with.dot".into(),
            new_value: Some("after".into()),
            row_identity: identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect("dotted relation update");
    let rows = database
        .query(
            "SELECT \"column.with.dot\" FROM \"schema.with.dot\".\"table.with.dot\"",
            &[],
        )
        .await
        .expect("reload dotted relation");
    assert_eq!(rows[0].get::<_, String>(0), "after");
}

async fn assert_null_provenance_and_updates(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "null_values");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("nullable values preview");
    assert_eq!(preview.rows.len(), 2);
    assert!(preview.rows.iter().all(|row| row[1] == "NULL"));
    let null_index = preview
        .metadata
        .null_cells
        .iter()
        .position(|row| row[1])
        .expect("SQL NULL provenance");
    let literal_index = preview
        .metadata
        .null_cells
        .iter()
        .position(|row| !row[1])
        .expect("literal NULL provenance");
    assert_ne!(null_index, literal_index);

    let identity = preview.metadata.row_identities[literal_index]
        .clone()
        .expect("literal NULL row identity");
    engine
        .crud(CrudAction::UpdateCell {
            table: relation,
            column: "value".into(),
            new_value: None,
            row_identity: identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect("set value to SQL NULL");
    let null_flags = database
        .query(
            "SELECT value IS NULL FROM engine_review.null_values ORDER BY id",
            &[],
        )
        .await
        .expect("reload nullable values");
    assert!(null_flags.iter().all(|row| row.get::<_, bool>(0)));
}

async fn assert_read_only_queries_are_unchanged(engine: &DatabaseEngine) {
    let count = engine
        .execute("SELECT count(*) FROM engine_review.ordering")
        .await
        .expect("aggregate query");
    assert_eq!(count.rows, vec![vec!["3".to_string()]]);
    assert!(count.metadata.row_identities.iter().all(Option::is_none));

    let distinct = engine
        .execute("SELECT DISTINCT a FROM engine_review.ordering ORDER BY a")
        .await
        .expect("distinct query");
    assert_eq!(
        distinct.rows,
        vec![vec!["1".to_string()], vec!["2".to_string()]]
    );

    let ordinal = engine
        .execute("SELECT a FROM engine_review.ordering ORDER BY 1")
        .await
        .expect("ordinal order query");
    assert_eq!(
        ordinal.rows,
        vec![
            vec!["1".to_string()],
            vec!["1".to_string()],
            vec!["2".to_string()]
        ]
    );
    assert!(ordinal.metadata.row_identities.iter().all(Option::is_none));
}

async fn assert_alias_update_and_batch_rejection(engine: &DatabaseEngine, database: &Database) {
    let aliased = engine
        .execute("SELECT a AS b FROM engine_review.alias_target")
        .await
        .expect("aliased direct column query");
    assert_eq!(aliased.columns, vec!["b".to_string()]);
    assert_eq!(aliased.metadata.source_columns, vec![Some("a".to_string())]);
    let alias_identity = aliased.metadata.row_identities[0]
        .clone()
        .expect("editable row identity");
    engine
        .crud(CrudAction::UpdateCell {
            table: qualified("engine_review", "alias_target"),
            column: aliased.metadata.source_columns[0]
                .clone()
                .expect("physical source column"),
            new_value: Some("99".to_string()),
            row_identity: alias_identity,
            column_type: Some("pg_catalog.int4".to_string()),
        })
        .await
        .expect("update aliased source column");
    let alias_values = database
        .query("SELECT a, b FROM engine_review.alias_target", &[])
        .await
        .expect("read alias target");
    assert_eq!(alias_values[0].get::<_, i32>(0), 99);
    assert_eq!(alias_values[0].get::<_, i32>(1), 20);

    let before_batch = database
        .query("SELECT count(*) FROM engine_review.alias_target", &[])
        .await
        .expect("count before batch")[0]
        .get::<_, i64>(0);
    let batch_error = engine
        .execute(
            "INSERT INTO engine_review.alias_target VALUES (30, 40); DELETE FROM engine_review.alias_target;",
        )
        .await
        .expect_err("SQL batch must fail before execution");
    assert!(batch_error
        .downcast_ref::<EngineError>()
        .is_some_and(|error| matches!(error, EngineError::UnsupportedSqlBatch(2))));
    let after_batch = database
        .query("SELECT count(*) FROM engine_review.alias_target", &[])
        .await
        .expect("count after batch")[0]
        .get::<_, i64>(0);
    assert_eq!(after_batch, before_batch);
}

async fn assert_view_and_oid_decoding(engine: &DatabaseEngine) {
    let view = engine
        .crud(CrudAction::SelectTop {
            table: qualified("engine_review", "ordering_view"),
            limit: 10,
            relation_kind: RelationKind::View,
        })
        .await
        .expect("view preview");
    assert_eq!(view.rows.len(), 3);
    assert!(view.metadata.row_identities.iter().all(Option::is_none));
    assert!(view.metadata.source_columns.iter().all(Option::is_none));

    let oid = engine
        .execute("SELECT 1::oid")
        .await
        .expect("OID query should not panic");
    assert_eq!(oid.rows, vec![vec!["1".to_string()]]);
}

async fn assert_wrong_row_update_is_rejected(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "wrong_update");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("wrong-update preview");
    let selected = preview
        .rows
        .iter()
        .position(|row| row[0] == "1")
        .expect("selected row");
    let stale_identity = preview.metadata.row_identities[selected]
        .clone()
        .expect("stable row identity");

    database
        .batch_execute("DELETE FROM engine_review.wrong_update WHERE id = 1")
        .await
        .expect("delete selected update row");
    database
        .batch_execute("VACUUM FULL engine_review.wrong_update")
        .await
        .expect("reuse update CTID");
    assert_reused_locator(database, "wrong_update", &stale_identity).await;

    let error = engine
        .crud(CrudAction::UpdateCell {
            table: relation,
            column: "value".into(),
            new_value: Some("corrupted".into()),
            row_identity: stale_identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect_err("reused locator must be stale");
    assert!(error
        .downcast_ref::<EngineError>()
        .is_some_and(|error| matches!(error, EngineError::StaleRow)));
    let value = database
        .query(
            "SELECT value FROM engine_review.wrong_update WHERE id = 2",
            &[],
        )
        .await
        .expect("read protected update row")[0]
        .get::<_, String>(0);
    assert_eq!(value, "second");
}

async fn assert_wrong_row_delete_is_rejected(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "wrong_delete");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("wrong-delete preview");
    let selected = preview
        .rows
        .iter()
        .position(|row| row[0] == "1")
        .expect("selected row");
    let stale_identity = preview.metadata.row_identities[selected]
        .clone()
        .expect("stable row identity");

    database
        .batch_execute("DELETE FROM engine_review.wrong_delete WHERE id = 1")
        .await
        .expect("delete selected delete row");
    database
        .batch_execute("VACUUM FULL engine_review.wrong_delete")
        .await
        .expect("reuse delete CTID");
    assert_reused_locator(database, "wrong_delete", &stale_identity).await;

    let error = engine
        .crud(CrudAction::DeleteRow {
            table: relation,
            row_identity: stale_identity,
        })
        .await
        .expect_err("reused locator must not delete");
    assert!(error
        .downcast_ref::<EngineError>()
        .is_some_and(|error| matches!(error, EngineError::StaleRow)));
    let ids = database
        .query("SELECT id FROM engine_review.wrong_delete ORDER BY id", &[])
        .await
        .expect("read protected delete row")
        .into_iter()
        .map(|row| row.get::<_, i32>(0))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![2]);
}

async fn assert_reused_locator(database: &Database, table: &str, identity: &RowIdentity) {
    let rows = database
        .query(
            &format!(
                "SELECT ctid::text, xmin::text FROM engine_review.{} WHERE id = 2",
                quote_identifier(table)
            ),
            &[],
        )
        .await
        .expect("read reused locator");
    assert_eq!(rows[0].get::<_, String>(0), identity.ctid);
    assert_eq!(
        rows[0].get::<_, String>(1),
        identity.xmin,
        "bulk-inserted rows must demonstrate that xmin alone is insufficient"
    );
}

async fn assert_stale_version_is_rejected(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "stale_version");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("stale-version preview");
    let stale_identity = preview.metadata.row_identities[0]
        .clone()
        .expect("stale-version identity");
    database
        .batch_execute("UPDATE engine_review.stale_version SET value = 'concurrent' WHERE id = 1")
        .await
        .expect("concurrent row change");

    let error = engine
        .crud(CrudAction::UpdateCell {
            table: relation,
            column: "value".into(),
            new_value: Some("lost update".into()),
            row_identity: stale_identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect_err("concurrent change must be stale");
    assert!(error
        .downcast_ref::<EngineError>()
        .is_some_and(|error| matches!(error, EngineError::StaleRow)));
    let value = database
        .query(
            "SELECT value FROM engine_review.stale_version WHERE id = 1",
            &[],
        )
        .await
        .expect("read concurrent value")[0]
        .get::<_, String>(0);
    assert_eq!(value, "concurrent");
}

async fn assert_composite_key_and_refresh(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "composite_key");
    let preview = engine
        .execute("SELECT value AS shown FROM engine_review.composite_key AS item")
        .await
        .expect("composite projected query");
    assert_eq!(preview.columns, vec!["shown"]);
    let identity = preview.metadata.row_identities[0]
        .clone()
        .expect("composite identity");
    assert_eq!(
        identity
            .primary_key
            .iter()
            .map(|key| key.column.as_str())
            .collect::<Vec<_>>(),
        vec!["tenant_id", "item_id"]
    );

    let update = engine
        .crud(CrudAction::UpdateCell {
            table: relation.clone(),
            column: "value".into(),
            new_value: Some("after".into()),
            row_identity: identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect("composite update");
    let updated_identity = update.metadata.row_identities[0]
        .clone()
        .expect("updated composite identity");
    let refreshed = engine
        .crud(CrudAction::RefreshRow {
            table: relation.clone(),
            row_identity: updated_identity.clone(),
        })
        .await
        .expect("validated row refresh");
    assert_eq!(
        refreshed.rows,
        vec![vec!["7".to_string(), "9".to_string(), "after".to_string()]]
    );
    engine
        .crud(CrudAction::DeleteRow {
            table: relation,
            row_identity: updated_identity,
        })
        .await
        .expect("composite delete");
    let count = database
        .query("SELECT count(*) FROM engine_review.composite_key", &[])
        .await
        .expect("count composite rows")[0]
        .get::<_, i64>(0);
    assert_eq!(count, 0);
}

async fn assert_custom_type_names_round_trip(engine: &DatabaseEngine, database: &Database) {
    let relation = qualified("engine_review", "custom_types");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("custom-type preview");
    assert_eq!(
        preview.metadata.column_types,
        vec![
            "\"pg_catalog\".\"int4\"",
            "\"types.with.dot\".\"status.type\"",
            "\"types\"\"quoted\".\"status\"\"type\"",
        ]
    );
    let identity = preview.metadata.row_identities[0]
        .clone()
        .expect("custom-type identity");
    let first_update = engine
        .crud(CrudAction::UpdateCell {
            table: relation.clone(),
            column: "dotted".into(),
            new_value: Some("after".into()),
            row_identity: identity,
            column_type: preview.metadata.column_types.get(1).cloned(),
        })
        .await
        .expect("dotted custom-type update");
    let updated_identity = first_update.metadata.row_identities[0]
        .clone()
        .expect("updated custom-type identity");
    engine
        .crud(CrudAction::UpdateCell {
            table: relation,
            column: "quoted".into(),
            new_value: Some("after".into()),
            row_identity: updated_identity,
            column_type: preview.metadata.column_types.get(2).cloned(),
        })
        .await
        .expect("quoted custom-type update");

    let row = &database
        .query(
            "SELECT dotted::text, quoted::text FROM engine_review.custom_types",
            &[],
        )
        .await
        .expect("read custom-type values")[0];
    assert_eq!(row.get::<_, String>(0), "after");
    assert_eq!(row.get::<_, String>(1), "after");
}

async fn assert_statement_timeout_uses_query_canceled_code(database: &Database) {
    let engine = DatabaseEngine::new(database.clone(), std::time::Duration::from_millis(1), 50);
    let error = engine
        .execute("SELECT pg_sleep(0.1)")
        .await
        .expect_err("statement timeout must fail");
    assert!(is_query_canceled(&error));
}

async fn assert_no_key_and_acl_reads_remain_available(
    engine: &DatabaseEngine,
    database: &Database,
) {
    let no_key = engine
        .execute("SELECT shown AS alias FROM engine_review.no_key")
        .await
        .expect("no-key query remains readable");
    assert_eq!(no_key.rows, vec![vec!["readable".to_string()]]);
    assert!(no_key.metadata.row_identities.iter().all(Option::is_none));

    let inherited = engine
        .crud(CrudAction::SelectTop {
            table: qualified("engine_review", "inherited_parent"),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("inheritance query remains readable");
    assert_eq!(inherited.rows.len(), 2);
    assert!(inherited
        .metadata
        .row_identities
        .iter()
        .all(Option::is_none));

    database
        .batch_execute("SET ROLE engine_limited")
        .await
        .expect("assume limited role");
    let restricted = engine
        .execute("SELECT shown FROM engine_review.restricted_key")
        .await
        .expect("column-limited query remains readable");
    database
        .batch_execute("RESET ROLE")
        .await
        .expect("restore owner role");
    assert_eq!(restricted.rows, vec![vec!["visible".to_string()]]);
    assert!(restricted
        .metadata
        .row_identities
        .iter()
        .all(Option::is_none));
}

async fn assert_automatic_view_preview_is_read_only(engine: &DatabaseEngine, database: &Database) {
    engine
        .crud(CrudAction::SelectTop {
            table: qualified("engine_review", "side_effect_view"),
            limit: 10,
            relation_kind: RelationKind::View,
        })
        .await
        .expect_err("automatic preview must reject a mutating function");
    let count = database
        .query(
            "SELECT count(*) FROM engine_review.preview_side_effect_log",
            &[],
        )
        .await
        .expect("read preview side-effect log")[0]
        .get::<_, i64>(0);
    assert_eq!(count, 0);
}

async fn assert_relation_name_is_bound(engine: &DatabaseEngine, database: &Database) {
    database
        .batch_execute("SET standard_conforming_strings = off")
        .await
        .expect("set nonstandard string literal parsing");
    let relation = qualified("engine_review", "backslash\\quote'table");
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: relation.clone(),
            limit: 10,
            relation_kind: RelationKind::Table,
        })
        .await
        .expect("quoted relation preview");
    let identity = preview.metadata.row_identities[0]
        .clone()
        .expect("quoted relation identity");
    engine
        .crud(CrudAction::UpdateCell {
            table: relation,
            column: "value".into(),
            new_value: Some("after".into()),
            row_identity: identity,
            column_type: Some("pg_catalog.text".into()),
        })
        .await
        .expect("bound relation update");
    database
        .batch_execute("RESET standard_conforming_strings")
        .await
        .expect("restore string parsing");
    let value = database
        .query(
            "SELECT value FROM engine_review.\"backslash\\quote'table\"",
            &[],
        )
        .await
        .expect("read awkward relation")[0]
        .get::<_, String>(0);
    assert_eq!(value, "after");
}

async fn assert_partition_identity_is_scoped(
    engine: &DatabaseEngine,
    database: &Database,
    table: &str,
    relation_kind: RelationKind,
) {
    let preview = engine
        .crud(CrudAction::SelectTop {
            table: simple_qualified(table),
            limit: 10,
            relation_kind,
        })
        .await
        .expect("row identity preview");
    assert_eq!(preview.rows.len(), 2);
    let first = preview.metadata.row_identities[0]
        .clone()
        .expect("first row identity");
    let second = preview.metadata.row_identities[1]
        .clone()
        .expect("second row identity");
    assert_eq!(first.ctid, second.ctid, "fixture must reproduce CTID reuse");
    assert_eq!(first.relation_oid, second.relation_oid);
    assert_ne!(
        first.table_oid, second.table_oid,
        "physical relation must disambiguate rows"
    );
    assert_eq!(first.primary_key.len(), 1);
    assert_eq!(first.primary_key[0].column, "partition_key");

    engine
        .crud(CrudAction::DeleteRow {
            table: simple_qualified(table),
            row_identity: first.clone(),
        })
        .await
        .expect("delete one physical row");
    let qualified = simple_qualified(table).quoted();
    let remaining = database
        .query(&format!("SELECT count(*) FROM {qualified}"), &[])
        .await
        .expect("count remaining rows")[0]
        .get::<_, i64>(0);
    assert_eq!(remaining, 1);

    assert!(engine
        .crud(CrudAction::DeleteRow {
            table: simple_qualified(table),
            row_identity: first,
        })
        .await
        .is_err());
    let still_remaining = database
        .query(&format!("SELECT count(*) FROM {qualified}"), &[])
        .await
        .expect("count after stale delete")[0]
        .get::<_, i64>(0);
    assert_eq!(still_remaining, 1);
}
