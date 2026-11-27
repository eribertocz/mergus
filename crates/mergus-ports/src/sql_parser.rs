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

    /// Returns the tree of `SELECT <expression>`, as [`SqlParser::parse`] does, for
    /// `expression`, such as the definition of a scalar macro (MG-REQ-0644.1). The statement
    /// is composed by DuckDB from the bound `expression`, never by Mergus (MG-REQ-0184).
    ///
    /// The default fails: only a parser that can compose the statement implements it.
    ///
    /// # Errors
    ///
    /// Fails when the parser itself cannot be reached.
    fn parse_select_list(&self, expression: &str) -> Result<String, PortError> {
        let _ = expression;
        Err(PortError::Failed(
            "this parser cannot parse a select list".to_owned(),
        ))
    }
}
