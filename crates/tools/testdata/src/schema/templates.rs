use super::definitions::{
    ColumnDefinition, ColumnSemantic, DataType, ForeignKeyDefinition, IndexDefinition,
    TableDefinition,
};

pub fn generate_table_definitions(table_count: usize) -> Vec<TableDefinition> {
    let base_templates = base_templates();
    let mut tables = Vec::with_capacity(table_count);

    let mut group_index = 0_usize;
    while tables.len() < table_count {
        for template in &base_templates {
            if tables.len() == table_count {
                break;
            }

            let suffix = if group_index == 0 {
                String::new()
            } else {
                format!("_{group_index}")
            };

            tables.push(template.instantiate(&suffix));
        }

        group_index += 1;
    }

    tables.truncate(table_count);
    tables
}

#[derive(Debug, Clone)]
struct TableTemplate {
    name: &'static str,
    columns: Vec<ColumnDefinition>,
    indexed_columns: Vec<&'static str>,
    foreign_keys: Vec<ForeignKeyDefinition>,
}

impl TableTemplate {
    fn instantiate(&self, suffix: &str) -> TableDefinition {
        let table_name = format!("{}{}", self.name, suffix);
        let indexes = self
            .indexed_columns
            .iter()
            .filter(|column| self.columns.iter().any(|col| col.name == **column))
            .map(|column| IndexDefinition {
                name: format!("idx_{table_name}_{column}"),
                column: (*column).to_string(),
            })
            .collect();

        let foreign_keys = self
            .foreign_keys
            .iter()
            .map(|fk| ForeignKeyDefinition {
                column: fk.column.clone(),
                references_table: format!("{}{}", fk.references_table, suffix),
                references_column: fk.references_column.clone(),
            })
            .collect();

        TableDefinition {
            name: table_name,
            columns: self.columns.clone(),
            indexes,
            foreign_keys,
        }
    }
}

fn base_templates() -> Vec<TableTemplate> {
    vec![
        users_template(),
        addresses_template(),
        products_template(),
        orders_template(),
        order_items_template(),
        payments_template(),
        sessions_template(),
        events_template(),
        reviews_template(),
    ]
}

fn users_template() -> TableTemplate {
    TableTemplate {
        name: "users",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "first_name".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::FirstName,
            },
            ColumnDefinition {
                name: "last_name".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::LastName,
            },
            ColumnDefinition {
                name: "email".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Email,
            },
            ColumnDefinition {
                name: "username".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Username,
            },
            ColumnDefinition {
                name: "phone".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Phone,
            },
            ColumnDefinition {
                name: "city".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::City,
            },
            ColumnDefinition {
                name: "country".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Country,
            },
            ColumnDefinition {
                name: "created_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "updated_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "active".to_string(),
                data_type: DataType::Boolean,
                nullable: false,
                semantic: ColumnSemantic::BooleanFlag,
            },
        ],
        indexed_columns: vec!["email", "username"],
        foreign_keys: Vec::new(),
    }
}

fn addresses_template() -> TableTemplate {
    TableTemplate {
        name: "addresses",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "user_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "line1".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Street,
            },
            ColumnDefinition {
                name: "line2".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Street,
            },
            ColumnDefinition {
                name: "city".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::City,
            },
            ColumnDefinition {
                name: "state".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Text,
            },
            ColumnDefinition {
                name: "postal_code".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::PostalCode,
            },
            ColumnDefinition {
                name: "country".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Country,
            },
            ColumnDefinition {
                name: "created_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "updated_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "is_primary".to_string(),
                data_type: DataType::Boolean,
                nullable: false,
                semantic: ColumnSemantic::BooleanFlag,
            },
        ],
        indexed_columns: vec!["user_id", "postal_code"],
        foreign_keys: vec![ForeignKeyDefinition {
            column: "user_id".to_string(),
            references_table: "users".to_string(),
            references_column: "id".to_string(),
        }],
    }
}

fn products_template() -> TableTemplate {
    TableTemplate {
        name: "products",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "name".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::ProductName,
            },
            ColumnDefinition {
                name: "sku".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Sku,
            },
            ColumnDefinition {
                name: "category".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Category,
            },
            ColumnDefinition {
                name: "description".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Body,
            },
            ColumnDefinition {
                name: "price".to_string(),
                data_type: DataType::Numeric,
                nullable: false,
                semantic: ColumnSemantic::Money,
            },
            ColumnDefinition {
                name: "currency".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Currency,
            },
            ColumnDefinition {
                name: "inventory_count".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Count,
            },
            ColumnDefinition {
                name: "available".to_string(),
                data_type: DataType::Boolean,
                nullable: false,
                semantic: ColumnSemantic::BooleanFlag,
            },
            ColumnDefinition {
                name: "created_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "updated_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["sku", "category"],
        foreign_keys: Vec::new(),
    }
}

fn orders_template() -> TableTemplate {
    TableTemplate {
        name: "orders",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "user_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "total_amount".to_string(),
                data_type: DataType::Numeric,
                nullable: false,
                semantic: ColumnSemantic::Money,
            },
            ColumnDefinition {
                name: "currency".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Currency,
            },
            ColumnDefinition {
                name: "status".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Status,
            },
            ColumnDefinition {
                name: "ordered_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "shipped_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: true,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["user_id", "ordered_at"],
        foreign_keys: vec![ForeignKeyDefinition {
            column: "user_id".to_string(),
            references_table: "users".to_string(),
            references_column: "id".to_string(),
        }],
    }
}

fn order_items_template() -> TableTemplate {
    TableTemplate {
        name: "order_items",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "order_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "product_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "quantity".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Quantity,
            },
            ColumnDefinition {
                name: "unit_price".to_string(),
                data_type: DataType::Numeric,
                nullable: false,
                semantic: ColumnSemantic::Money,
            },
            ColumnDefinition {
                name: "currency".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Currency,
            },
            ColumnDefinition {
                name: "created_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["order_id", "product_id"],
        foreign_keys: vec![
            ForeignKeyDefinition {
                column: "order_id".to_string(),
                references_table: "orders".to_string(),
                references_column: "id".to_string(),
            },
            ForeignKeyDefinition {
                column: "product_id".to_string(),
                references_table: "products".to_string(),
                references_column: "id".to_string(),
            },
        ],
    }
}

fn payments_template() -> TableTemplate {
    TableTemplate {
        name: "payments",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "order_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "provider".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Company,
            },
            ColumnDefinition {
                name: "status".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Status,
            },
            ColumnDefinition {
                name: "amount".to_string(),
                data_type: DataType::Numeric,
                nullable: false,
                semantic: ColumnSemantic::Money,
            },
            ColumnDefinition {
                name: "currency".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Currency,
            },
            ColumnDefinition {
                name: "paid_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["order_id", "status"],
        foreign_keys: vec![ForeignKeyDefinition {
            column: "order_id".to_string(),
            references_table: "orders".to_string(),
            references_column: "id".to_string(),
        }],
    }
}

fn sessions_template() -> TableTemplate {
    TableTemplate {
        name: "sessions",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "user_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "device".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Device,
            },
            ColumnDefinition {
                name: "ip_address".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::IpAddress,
            },
            ColumnDefinition {
                name: "started_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "ended_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: true,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["user_id", "started_at"],
        foreign_keys: vec![ForeignKeyDefinition {
            column: "user_id".to_string(),
            references_table: "users".to_string(),
            references_column: "id".to_string(),
        }],
    }
}

fn events_template() -> TableTemplate {
    TableTemplate {
        name: "events",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "user_id".to_string(),
                data_type: DataType::Integer,
                nullable: true,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "session_id".to_string(),
                data_type: DataType::Integer,
                nullable: true,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "event_name".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::EventName,
            },
            ColumnDefinition {
                name: "payload".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::JsonPayload,
            },
            ColumnDefinition {
                name: "occurred_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
            ColumnDefinition {
                name: "successful".to_string(),
                data_type: DataType::Boolean,
                nullable: false,
                semantic: ColumnSemantic::BooleanFlag,
            },
        ],
        indexed_columns: vec!["user_id", "session_id", "occurred_at"],
        foreign_keys: vec![
            ForeignKeyDefinition {
                column: "user_id".to_string(),
                references_table: "users".to_string(),
                references_column: "id".to_string(),
            },
            ForeignKeyDefinition {
                column: "session_id".to_string(),
                references_table: "sessions".to_string(),
                references_column: "id".to_string(),
            },
        ],
    }
}

fn reviews_template() -> TableTemplate {
    TableTemplate {
        name: "reviews",
        columns: vec![
            ColumnDefinition {
                name: "id".to_string(),
                data_type: DataType::Serial,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "user_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "product_id".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Identifier,
            },
            ColumnDefinition {
                name: "rating".to_string(),
                data_type: DataType::Integer,
                nullable: false,
                semantic: ColumnSemantic::Count,
            },
            ColumnDefinition {
                name: "title".to_string(),
                data_type: DataType::Text,
                nullable: false,
                semantic: ColumnSemantic::Title,
            },
            ColumnDefinition {
                name: "body".to_string(),
                data_type: DataType::Text,
                nullable: true,
                semantic: ColumnSemantic::Body,
            },
            ColumnDefinition {
                name: "created_at".to_string(),
                data_type: DataType::Timestamp,
                nullable: false,
                semantic: ColumnSemantic::Timestamp,
            },
        ],
        indexed_columns: vec!["user_id", "product_id"],
        foreign_keys: vec![
            ForeignKeyDefinition {
                column: "user_id".to_string(),
                references_table: "users".to_string(),
                references_column: "id".to_string(),
            },
            ForeignKeyDefinition {
                column: "product_id".to_string(),
                references_table: "products".to_string(),
                references_column: "id".to_string(),
            },
        ],
    }
}
