#[derive(Debug, Clone)]
pub struct TableDefinition {
    pub name: String,
    pub columns: Vec<ColumnDefinition>,
    pub indexes: Vec<IndexDefinition>,
    pub foreign_keys: Vec<ForeignKeyDefinition>,
}

#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub semantic: ColumnSemantic,
}

#[derive(Debug, Clone)]
pub enum ColumnSemantic {
    Identifier,
    FirstName,
    LastName,
    Username,
    Email,
    Phone,
    City,
    Country,
    Street,
    PostalCode,
    Company,
    ProductName,
    Category,
    Sku,
    Title,
    Body,
    Status,
    Currency,
    Money,
    Quantity,
    Count,
    EventName,
    Device,
    IpAddress,
    JsonPayload,
    Timestamp,
    BooleanFlag,
    Text,
}

#[derive(Debug, Clone)]
pub enum DataType {
    Serial,
    Integer,
    Text,
    Boolean,
    Timestamp,
    Numeric,
}

impl DataType {
    pub(crate) fn to_sql(&self) -> &str {
        match self {
            Self::Serial => "SERIAL",
            Self::Integer => "INTEGER",
            Self::Text => "TEXT",
            Self::Boolean => "BOOLEAN",
            Self::Timestamp => "TIMESTAMP",
            Self::Numeric => "NUMERIC",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexDefinition {
    pub name: String,
    pub column: String,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyDefinition {
    pub column: String,
    pub references_table: String,
    pub references_column: String,
}
