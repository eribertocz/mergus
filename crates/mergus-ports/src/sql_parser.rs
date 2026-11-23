//! The `SqlParser` port: DuckDB's SQL parser, without executing anything (MG-REQ-0193).

use crate::PortError;

/// DuckDB's SQL parser.
pub trait SqlParser: Send + Sync {
    /// Returns, for the SQL text `sql`, the JSON text that DuckDB's `json_serialize_sql`
    /// produces for it, exactly. A text DuckDB cannot parse is not a failure of the port: the
    /// tree then reports the error, as `json_serialize_sql` does. The text is never executed.
    ///
    /// # Errors
    ///
    /// Fails when the parser itself cannot be reached.
    fn parse(&self, sql: &str) -> Result<String, PortError>;
}
