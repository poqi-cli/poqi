/// Keywords surfaced to the UI so syntax highlighters and completion share one list.
pub const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "ON",
    "USING",
    "GROUP",
    "BY",
    "ORDER",
    "HAVING",
    "WINDOW",
    "INSERT",
    "INSERT INTO",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "DELETE FROM",
    "RETURNING",
    "LIMIT",
    "OFFSET",
    "FETCH",
    "DISTINCT",
    "UNION",
    "ALL",
    "EXCEPT",
    "INTERSECT",
    "CREATE",
    "ALTER",
    "DROP",
    "ADD",
    "TABLE",
    "COLUMN",
    "VIEW",
    "MATERIALIZED",
    "GRANT",
    "REVOKE",
    "TRUNCATE",
    "CREATE TABLE",
    "ALTER TABLE",
    "DROP TABLE",
    "TRUNCATE TABLE",
    "ADD COLUMN",
    "DROP COLUMN",
    "RENAME COLUMN",
    "TO",
    "RENAME",
    "WITH",
    "AS",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "EXISTS",
    "IS",
    "LIKE",
    "ILIKE",
    "DISTINCT ON",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
];

// The analyzer lowercases tokens before comparing, so we keep display-friendly casing here.
pub(crate) const START_KEYWORDS: &[&str] =
    &["SELECT", "UPDATE", "WITH", "INSERT INTO", "DELETE FROM"];
pub(crate) const START_DDL_KEYWORDS: &[&str] = &[
    "CREATE TABLE",
    "ALTER TABLE",
    "DROP TABLE",
    "TRUNCATE TABLE",
];
pub(crate) const ALTER_TABLE_ACTIONS: &[&str] =
    &["ADD COLUMN", "DROP COLUMN", "RENAME COLUMN", "RENAME TO"];
pub(crate) const SELECT_LIST_TOKENS: &[&str] = &["*", "DISTINCT", "DISTINCT ON", "CASE", "FROM"];
pub(crate) const JOIN_TYPES: &[&str] = &[
    "JOIN",
    "INNER JOIN",
    "LEFT JOIN",
    "LEFT OUTER JOIN",
    "RIGHT JOIN",
    "RIGHT OUTER JOIN",
    "FULL JOIN",
    "FULL OUTER JOIN",
    "CROSS JOIN",
    "NATURAL JOIN",
];
pub(crate) const LOGICAL_OPS: &[&str] = &["AND", "OR", "NOT"];
pub(crate) const CMP_OPS: &[&str] = &[
    "=",
    "<>",
    "!=",
    "<",
    "<=",
    ">",
    ">=",
    "IN",
    "NOT IN",
    "BETWEEN",
    "NOT BETWEEN",
    "LIKE",
    "NOT LIKE",
    "ILIKE",
    "NOT ILIKE",
    "IS",
    "IS NOT",
    "IS NULL",
    "IS NOT NULL",
];
pub(crate) const ORDER_MODS: &[&str] = &["ASC", "DESC", "NULLS FIRST", "NULLS LAST"];
pub(crate) const VALUE_KEYWORDS: &[&str] = &[
    "NULL",
    "TRUE",
    "FALSE",
    "NOW()",
    "CURRENT_TIMESTAMP",
    "CURRENT_DATE",
];
pub(crate) const GROUPING_TOKENS: &[&str] = &["ROLLUP", "CUBE", "GROUPING SETS", "HAVING"];
pub(crate) const WINDOW_TOKENS: &[&str] = &["PARTITION BY", "ORDER BY", "ROWS", "RANGE", "GROUPS"];
pub(crate) const AGG_FUNCS: &[&str] = &["COUNT", "SUM", "AVG", "MIN", "MAX"];
pub(crate) const CLAUSE_TOKENS: &[&str] =
    &["WHERE", "JOIN", "GROUP BY", "ORDER BY", "LIMIT", "OFFSET"];
pub(crate) const DATA_SOURCE_TOKENS: &[&str] = &["LATERAL", "TABLE", "VALUES"];
