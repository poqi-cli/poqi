use super::definitions::{IndexDefinition, TableDefinition};

pub fn generate_create_table_sql(schema: &str, table: &TableDefinition) -> String {
    let mut sql = format!("CREATE TABLE {}.{} (\n", schema, table.name);

    let mut column_defs: Vec<String> = table
        .columns
        .iter()
        .map(|col| {
            let null_constraint = if col.nullable { "" } else { " NOT NULL" };
            let primary_key = if col.name == "id" { " PRIMARY KEY" } else { "" };
            format!(
                "  {} {}{}{}",
                col.name,
                col.data_type.to_sql(),
                null_constraint,
                primary_key
            )
        })
        .collect();

    append_foreign_keys(schema, table, &mut column_defs);

    sql.push_str(&column_defs.join(",\n"));
    sql.push_str("\n);");

    sql
}

pub fn generate_create_index_sql(schema: &str, table: &str, index: &IndexDefinition) -> String {
    format!(
        "CREATE INDEX {} ON {}.{} ({});",
        index.name, schema, table, index.column
    )
}

fn append_foreign_keys(schema: &str, table: &TableDefinition, column_defs: &mut Vec<String>) {
    for fk in &table.foreign_keys {
        column_defs.push(format!(
            "  FOREIGN KEY ({}) REFERENCES {}.{}({})",
            fk.column, schema, fk.references_table, fk.references_column
        ));
    }
}
