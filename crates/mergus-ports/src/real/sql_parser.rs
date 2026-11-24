//! DuckDB's SQL parser, without executing anything (MG-REQ-0193).

use std::sync::{Mutex, PoisonError};

use duckdb::{Config, Connection};

use crate::{PortError, SqlParser};

/// The query that returns the tree of its one bound parameter. The SQL text is bound, never
/// concatenated (MG-REQ-0007), and `json_serialize_sql` only parses it.
const SERIALIZE: &str = "SELECT json_serialize_sql(CAST(? AS VARCHAR))::VARCHAR";

/// DuckDB's parser: an in-memory DuckDB with `enable_external_access = false`, opened on the
/// first call and shared by every later one.
#[derive(Debug, Default)]
pub struct RealSqlParser {
    connection: Mutex<Option<Connection>>,
}

impl RealSqlParser {
    /// Returns a parser; its DuckDB is opened on the first call to [`SqlParser::parse`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SqlParser for RealSqlParser {
    fn parse(&self, sql: &str) -> Result<String, PortError> {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if connection.is_none() {
            *connection = Some(open().map_err(failed)?);
        }
        let Some(connection) = connection.as_ref() else {
            return Err(PortError::Failed("the SQL parser is not open".to_owned()));
        };
        connection
            .query_row(SERIALIZE, [sql], |row| row.get::<_, String>(0))
            .map_err(failed)
    }
}

/// Opens the in-memory DuckDB of the parser, unable to reach files or the network.
fn open() -> duckdb::Result<Connection> {
    let config = Config::default()
        .enable_external_access(false)?
        .enable_autoload_extension(false)?;
    Connection::open_in_memory_with_flags(config)
}

/// Converts a DuckDB error into a [`PortError`].
fn failed(error: duckdb::Error) -> PortError {
    PortError::Failed(format!("the SQL parser failed: {error}"))
}
