//! The columns of a query result, and the DuckDB types Mergus writes into DDL
//! (MG-REQ-0604).

/// A column of the result of a query, as DuckDB types it (MG-REQ-0604).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultColumn {
    /// The name of the column.
    pub name: String,
    /// The DuckDB type of the column, as DuckDB writes it, such as `DECIMAL(18,3)`.
    pub data_type: String,
}

/// The DuckDB scalar types, as `DESCRIBE` writes them, that may name a column type in DDL.
const COLUMN_TYPES: [&str; 26] = [
    "BOOLEAN",
    "TINYINT",
    "SMALLINT",
    "INTEGER",
    "BIGINT",
    "HUGEINT",
    "UTINYINT",
    "USMALLINT",
    "UINTEGER",
    "UBIGINT",
    "UHUGEINT",
    "FLOAT",
    "DOUBLE",
    "VARCHAR",
    "BLOB",
    "UUID",
    "DATE",
    "TIME",
    "TIME WITH TIME ZONE",
    "TIMESTAMP",
    "TIMESTAMP WITH TIME ZONE",
    "TIMESTAMP_S",
    "TIMESTAMP_MS",
    "TIMESTAMP_NS",
    "INTERVAL",
    "BIT",
];

/// Returns whether `data_type` is a DuckDB scalar column type as `DESCRIBE` writes it (such
/// as `BIGINT` or `TIMESTAMP WITH TIME ZONE`), or
/// `DECIMAL(<p>,<s>)` with digits only.
#[must_use]
pub fn is_column_type(data_type: &str) -> bool {
    if COLUMN_TYPES.contains(&data_type) {
        return true;
    }
    let Some(arguments) = data_type
        .strip_prefix("DECIMAL(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return false;
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    arguments
        .split_once(',')
        .is_some_and(|(precision, scale)| digits(precision) && digits(scale))
}
