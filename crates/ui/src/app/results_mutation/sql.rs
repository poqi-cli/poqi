use super::super::App;

impl App {
    pub(super) fn quote_identifier(value: &str) -> String {
        poqi_catalog::quote_identifier(value)
    }

    pub(super) fn literal(value: &str) -> String {
        let escaped = value.replace('\'', "''");
        format!("'{escaped}'")
    }

    pub(super) fn row_identity_preview(identity: &poqi_engine::RowIdentity) -> String {
        let mut predicates = vec![
            format!("tableoid = {}::oid", identity.table_oid),
            format!("ctid = {}::tid", Self::literal(&identity.ctid)),
            format!("xmin::text = {}", Self::literal(&identity.xmin)),
        ];
        for (index, key) in identity.primary_key.iter().enumerate() {
            predicates.push(format!(
                "{} = ${}",
                Self::quote_identifier(&key.column),
                index + 1
            ));
        }
        format!(
            "{}\n-- Primary-key parameters and relation identity are checked by the engine.",
            predicates.join(" AND ")
        )
    }
}
