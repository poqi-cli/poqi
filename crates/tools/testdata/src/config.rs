#[derive(Debug, Clone)]
pub struct TestDataConfig {
    pub schemas: Vec<SchemaConfig>,
}

#[derive(Debug, Clone)]
pub struct SchemaConfig {
    pub name: String,
    pub table_count: usize,
    pub rows_per_table: usize,
}

impl TestDataConfig {
    #[must_use]
    pub fn default_config() -> Self {
        Self {
            schemas: vec![
                SchemaConfig {
                    name: "schema1".to_string(),
                    table_count: 100,
                    rows_per_table: 100,
                },
                SchemaConfig {
                    name: "schema2".to_string(),
                    table_count: 100,
                    rows_per_table: 1000,
                },
                SchemaConfig {
                    name: "schema3".to_string(),
                    table_count: 100,
                    rows_per_table: 10_000,
                },
            ],
        }
    }
}
