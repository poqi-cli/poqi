use crate::config::{SchemaConfig, TestDataConfig};
use crate::data::generate_insert_statements;
use crate::error::TestDataError;
use crate::schema::{
    generate_create_index_sql, generate_create_table_sql, generate_table_definitions,
};
use poqi_db::Database;

/// Generates test database with schemas, tables, and data.
///
/// # Errors
///
/// Returns `TestDataError::GenerationFailed` if schema creation, table creation, or data insertion fails.
pub async fn generate_test_database<F>(
    database: &Database,
    config: TestDataConfig,
    mut progress_callback: F,
) -> Result<(), TestDataError>
where
    F: FnMut(String),
{
    for schema_config in &config.schemas {
        progress_callback(format!("Creating schema {}...", schema_config.name));
        create_schema(database, &schema_config.name).await?;

        generate_schema_data(database, schema_config, &mut progress_callback).await?;
    }

    progress_callback("Test data generation complete!".to_string());
    Ok(())
}

async fn create_schema(database: &Database, schema_name: &str) -> Result<(), TestDataError> {
    let sql = format!("CREATE SCHEMA IF NOT EXISTS {schema_name}");
    database
        .batch_execute(&sql)
        .await
        .map_err(|e| TestDataError::GenerationFailed(format!("Failed to create schema: {e}")))?;
    Ok(())
}

async fn generate_schema_data<F>(
    database: &Database,
    schema_config: &SchemaConfig,
    progress_callback: &mut F,
) -> Result<(), TestDataError>
where
    F: FnMut(String),
{
    let tables = generate_table_definitions(schema_config.table_count);

    for (table_index, table_def) in tables.into_iter().enumerate() {
        if table_index % 10 == 0 {
            progress_callback(format!(
                "Creating {}: table {}/{}",
                schema_config.name,
                table_index + 1,
                schema_config.table_count
            ));
        }

        let create_table_sql = generate_create_table_sql(&schema_config.name, &table_def);
        database
            .batch_execute(&create_table_sql)
            .await
            .map_err(|e| TestDataError::GenerationFailed(format!("Failed to create table: {e}")))?;

        let insert_statements = generate_insert_statements(
            &schema_config.name,
            &table_def,
            1,
            schema_config.rows_per_table,
        );

        for insert_sql in insert_statements {
            database.batch_execute(&insert_sql).await.map_err(|e| {
                TestDataError::GenerationFailed(format!("Failed to insert data: {e}"))
            })?;
        }

        for index_def in &table_def.indexes {
            let create_index_sql =
                generate_create_index_sql(&schema_config.name, &table_def.name, index_def);
            database
                .batch_execute(&create_index_sql)
                .await
                .map_err(|e| {
                    TestDataError::GenerationFailed(format!("Failed to create index: {e}"))
                })?;
        }
    }

    Ok(())
}
