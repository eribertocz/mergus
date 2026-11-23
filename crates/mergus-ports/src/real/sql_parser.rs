//! DuckDB's SQL parser, run in an in-memory database that reaches nothing outside itself
//! (MG-REQ-0193).

use std::sync::{Mutex, PoisonError};

use duckdb::{Config, Connection};

use crate::{PortError, SqlParser};

/// The query that turns a SQL text, given as its only bound parameter, into its tree. The
/// text is a value of the query and is never executed (MG-REQ-0007).
const SERIALIZE: &str = "SELECT json_serialize_sql(?::VARCHAR)";

/// DuckDB's SQL parser, in an in-memory database opened with `enable_external_access =
/// false` on the first call and kept for the later ones.
#[derive(Debug, Default)]
pub struct RealSqlParser {
    connection: Mutex<Option<Connection>>,
}

impl RealSqlParser {
    /// Returns a parser that opens its database on the first call.
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
        let connection = match &mut *connection {
            Some(connection) => connection,
            empty @ None => empty.insert(open().map_err(|error| failed(&error))?),
        };
        connection
            .query_row(SERIALIZE, [sql], |row| row.get::<_, String>(0))
            .map_err(|error| failed(&error))
    }
}

/// Opens an in-memory database that cannot reach files, the network or extensions.
fn open() -> duckdb::Result<Connection> {
    let config = Config::default()
        .enable_external_access(false)?
        .enable_autoload_extension(false)?;
    Connection::open_in_memory_with_flags(config)
}

/// Converts a DuckDB error into a [`PortError`].
fn failed(error: &duckdb::Error) -> PortError {
    PortError::Failed(format!("the SQL parser: {error}"))
}
