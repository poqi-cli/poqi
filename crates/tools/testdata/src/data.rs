use crate::schema::{
    ColumnDefinition, ColumnSemantic, DataType, ForeignKeyDefinition, TableDefinition,
};
use fake::faker::{
    address::en::{CityName, CountryName, StreetName, ZipCode},
    company::en::CompanyName,
    internet::en::{FreeEmail, Username},
    lorem::en::{Paragraph, Sentence, Words},
    name::en::{FirstName, LastName},
    phone_number::en::PhoneNumber,
};
use fake::Fake;
use rand::{rngs::ThreadRng, seq::SliceRandom, Rng};
use std::collections::HashMap;

pub fn generate_insert_statements(
    schema: &str,
    table: &TableDefinition,
    start_row: usize,
    row_count: usize,
) -> Vec<String> {
    let mut statements = Vec::new();
    let batch_size = 100;
    let mut rng = rand::thread_rng();
    let foreign_keys = foreign_key_lookup(table);

    for batch_start in (0..row_count).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(row_count);
        let batch_count = batch_end - batch_start;

        let mut values = Vec::new();

        for i in 0..batch_count {
            let row_number = start_row + batch_start + i;
            let row_values =
                generate_row_values(table, row_number, row_count, &mut rng, &foreign_keys);
            values.push(format!("({})", row_values.join(", ")));
        }

        let column_names: Vec<String> = table
            .columns
            .iter()
            .filter(|col| col.name != "id")
            .map(|col| col.name.clone())
            .collect();

        let insert_sql = format!(
            "INSERT INTO {}.{} ({}) VALUES {};",
            schema,
            table.name,
            column_names.join(", "),
            values.join(", ")
        );

        statements.push(insert_sql);
    }

    statements
}

fn foreign_key_lookup(table: &TableDefinition) -> HashMap<&str, &ForeignKeyDefinition> {
    table
        .foreign_keys
        .iter()
        .map(|fk| (fk.column.as_str(), fk))
        .collect()
}

fn generate_row_values(
    table: &TableDefinition,
    row_number: usize,
    row_count: usize,
    rng: &mut ThreadRng,
    foreign_keys: &HashMap<&str, &ForeignKeyDefinition>,
) -> Vec<String> {
    table
        .columns
        .iter()
        .filter(|col| col.name != "id")
        .map(|col| generate_value(col, row_number, row_count, rng, foreign_keys))
        .collect()
}

fn generate_value(
    column: &ColumnDefinition,
    row_number: usize,
    row_count: usize,
    rng: &mut ThreadRng,
    foreign_keys: &HashMap<&str, &ForeignKeyDefinition>,
) -> String {
    if column.nullable && rng.gen_bool(0.1) {
        return "NULL".to_string();
    }

    if foreign_keys.contains_key(column.name.as_str()) {
        let max_id = row_count.max(1);
        let value = rng.gen_range(1..=max_id);
        return value.to_string();
    }

    match column.data_type {
        DataType::Serial => unreachable!("Serial columns are auto-generated"),
        DataType::Integer => generate_integer_value(column, row_number, rng),
        DataType::Numeric => generate_numeric_value(column, rng),
        DataType::Text => generate_text_value(column, rng),
        DataType::Boolean => generate_boolean_value(rng),
        DataType::Timestamp => generate_timestamp_value(rng),
    }
}

fn generate_integer_value(
    column: &ColumnDefinition,
    row_number: usize,
    rng: &mut ThreadRng,
) -> String {
    match column.semantic {
        ColumnSemantic::Quantity => rng.gen_range(1..=20).to_string(),
        ColumnSemantic::Count => rng.gen_range(0..=5_000).to_string(),
        ColumnSemantic::Identifier => row_number.to_string(),
        _ => rng.gen_range(0..=10_000).to_string(),
    }
}

fn generate_numeric_value(column: &ColumnDefinition, rng: &mut ThreadRng) -> String {
    match column.semantic {
        ColumnSemantic::Money => {
            let cents = rng.gen_range(500..=50_000);
            format!("{:.2}", f64::from(cents) / 100.0)
        }
        _ => format!("{:.2}", f64::from(rng.gen_range(0..=10_000)) / 10.0),
    }
}

fn generate_timestamp_value(rng: &mut ThreadRng) -> String {
    let days = rng.gen_range(0..=730);
    let seconds = rng.gen_range(0..86_400);
    format!("'2023-01-01 00:00:00'::timestamp + interval '{days} days {seconds} seconds'")
}

fn generate_boolean_value(rng: &mut ThreadRng) -> String {
    if rng.gen_bool(0.55) {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

fn generate_text_value(column: &ColumnDefinition, rng: &mut ThreadRng) -> String {
    let value = match column.semantic {
        ColumnSemantic::FirstName => FirstName().fake::<String>(),
        ColumnSemantic::LastName => LastName().fake::<String>(),
        ColumnSemantic::Username => Username().fake::<String>(),
        ColumnSemantic::Email => FreeEmail().fake::<String>(),
        ColumnSemantic::Phone => PhoneNumber().fake::<String>(),
        ColumnSemantic::City => CityName().fake::<String>(),
        ColumnSemantic::Country => CountryName().fake::<String>(),
        ColumnSemantic::Street => StreetName().fake::<String>(),
        ColumnSemantic::PostalCode => ZipCode().fake::<String>(),
        ColumnSemantic::Company => CompanyName().fake::<String>(),
        ColumnSemantic::ProductName => Words(2..4).fake::<Vec<String>>().join(" "),
        ColumnSemantic::Category => Words(1..3).fake::<Vec<String>>().join(" "),
        ColumnSemantic::Sku => format!("SKU-{:05}", rng.gen_range(0..100_000)),
        ColumnSemantic::Title => Sentence(2..6).fake::<String>(),
        ColumnSemantic::Body => Paragraph(2..4).fake::<String>(),
        ColumnSemantic::Status => random_status(rng),
        ColumnSemantic::Currency => random_currency(rng),
        ColumnSemantic::EventName => random_event(rng),
        ColumnSemantic::Device => random_device(rng),
        ColumnSemantic::IpAddress => format!(
            "{}.{}.{}.{}",
            rng.gen_range(10..=200),
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
            rng.gen_range(0..=255)
        ),
        ColumnSemantic::JsonPayload => format!(
            r#"{{"message":"{}","code":{},"success":{}}}"#,
            Sentence(3..8).fake::<String>(),
            rng.gen_range(100..999),
            generate_boolean_value(rng)
        ),
        ColumnSemantic::BooleanFlag => generate_boolean_value(rng),
        ColumnSemantic::Text => Words(2..5).fake::<Vec<String>>().join(" "),
        ColumnSemantic::Timestamp => return generate_timestamp_value(rng),
        ColumnSemantic::Identifier => row_number_string(rng),
        ColumnSemantic::Count | ColumnSemantic::Quantity | ColumnSemantic::Money => {
            rng.gen_range(0..=10_000).to_string()
        }
    };

    quote(&value)
}

fn row_number_string(rng: &mut ThreadRng) -> String {
    rng.gen_range(1..=1_000_000).to_string()
}

fn random_status(rng: &mut ThreadRng) -> String {
    let statuses = [
        "pending",
        "processing",
        "completed",
        "failed",
        "canceled",
        "refunded",
    ];
    statuses
        .choose(rng)
        .copied()
        .unwrap_or("pending")
        .to_string()
}

fn random_currency(rng: &mut ThreadRng) -> String {
    let currencies = ["USD", "EUR", "GBP", "JPY", "AUD", "CAD"];
    currencies.choose(rng).copied().unwrap_or("USD").to_string()
}

fn random_event(rng: &mut ThreadRng) -> String {
    let events = [
        "page_view",
        "signup",
        "login",
        "add_to_cart",
        "checkout",
        "purchase",
        "error",
    ];
    events
        .choose(rng)
        .copied()
        .unwrap_or("page_view")
        .to_string()
}

fn random_device(rng: &mut ThreadRng) -> String {
    let devices = [
        "iPhone", "Android", "macOS", "Windows", "Linux", "iPad", "ChromeOS",
    ];
    devices
        .choose(rng)
        .copied()
        .unwrap_or("Unknown")
        .to_string()
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
