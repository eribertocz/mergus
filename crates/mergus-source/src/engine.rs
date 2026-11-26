//! Database sources attached read-only through the `Database` port (MG-REQ-0421,
//! MG-REQ-0196), with the engine that answers identified before any data is read
//! (MG-REQ-0410).

use std::fmt;
use std::path::Path;

use arrow::array::{Array, AsArray};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use mergus_ports::{
    Database, DatabaseConnection, DatabaseLocation, PortError, PostgresSource, SqlValue,
};

/// A database Mergus reads (MG-REQ-0409.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DatabaseEngine {
    /// PostgreSQL, through DuckDB's `postgres` extension.
    PostgreSql,
    /// A DuckDB file, attached without extension.
    DuckDb,
}

impl fmt::Display for DatabaseEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PostgreSql => "postgresql",
            Self::DuckDb => "duckdb",
        })
    }
}

/// The engine that answered a source, with the version to record in the load registry
/// (MG-REQ-0410).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineIdentity {
    /// The engine, the declared one.
    pub engine: DatabaseEngine,
    /// The bundled DuckDB's version for a DuckDB file; the answer of `SELECT version()` for
    /// PostgreSQL.
    pub version: String,
}

/// A source attached read-only to an in-memory DuckDB connection, its engine identified.
pub struct AttachedSource {
    /// The connection the source is attached to.
    pub connection: Box<dyn DatabaseConnection>,
    /// The name the source is attached as.
    pub alias: String,
    /// The engine that answered.
    pub identity: EngineIdentity,
}

impl fmt::Debug for AttachedSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AttachedSource")
            .field("alias", &self.alias)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Why a source could not be attached.
#[derive(Debug, thiserror::Error)]
pub enum ConnectError {
    /// The engine that answers differs from the declared one, or cannot be identified
    /// (MG-REQ-0410). No data was read.
    #[error(
        "source `{source_name}` declares engine `{declared}` but {}",
        detected_text(detected.as_deref(), detail)
    )]
    EngineMismatch {
        /// The source name.
        source_name: String,
        /// The declared engine.
        declared: DatabaseEngine,
        /// The engine that answered, `None` when it cannot be identified.
        detected: Option<String>,
        /// Why the engine cannot be identified; empty when it was.
        detail: String,
    },
    /// The port failed for another reason, such as a server that cannot be reached.
    #[error(transparent)]
    Port(#[from] PortError),
}

/// The end of the message of a mismatch.
fn detected_text(detected: Option<&str>, detail: &str) -> String {
    match detected {
        Some(detected) => format!("the engine that answers is `{detected}`"),
        None => format!("the engine that answers cannot be identified: {detail}"),
    }
}

impl ConnectError {
    /// Returns the `MG-####` code of a mismatch (MG-REQ-0008); `None` for a port failure,
    /// which the caller reports with the code of its port error.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::EngineMismatch { .. } => Some("MG-0404"),
            Self::Port(_) => None,
        }
    }
}

/// Attaches the DuckDB file at `location`, of the source `source_name`, read-only to a new
/// in-memory connection (MG-REQ-0421), so Mergus never writes to it. The attach without
/// `TYPE`, which DuckDB refuses for a file of another format, identifies the engine, and the
/// bundled DuckDB's version is returned (MG-REQ-0410). The source is attached as the file
/// name without its extension.
///
/// # Errors
///
/// Fails with [`ConnectError::EngineMismatch`] when the file cannot be attached, and with
/// [`ConnectError::Port`] when the connection cannot be opened or the version read.
pub fn attach_duckdb_file(
    database: &dyn Database,
    source_name: &str,
    location: &Path,
) -> Result<AttachedSource, ConnectError> {
    let mut connection = database.connect(&DatabaseLocation::InMemory)?;
    if let Err(error) = connection.attach_duckdb_file(location) {
        return Err(unidentified(source_name, DatabaseEngine::DuckDb, &error));
    }
    let rows = connection.query_arrow("SELECT version()", &[])?;
    let version = first_text(&rows)
        .ok_or_else(|| PortError::Failed("DuckDB returned no version".to_owned()))?;
    let alias = location
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(AttachedSource {
        connection,
        alias,
        identity: EngineIdentity {
            engine: DatabaseEngine::DuckDb,
            version,
        },
    })
}

/// Attaches the PostgreSQL source `source`, named `source_name`, read-only to a new
/// in-memory connection (MG-REQ-0196.2), then identifies the engine by `SELECT version()`
/// sent through `postgres_query`, whose answer must start with `PostgreSQL`; the answer is
/// returned (MG-REQ-0410). On a mismatch the source is detached and its secret dropped.
///
/// # Errors
///
/// Fails with [`ConnectError::EngineMismatch`] when the answer does not start with
/// `PostgreSQL` or the engine cannot be identified, and with [`ConnectError::Port`] when the
/// source cannot be attached.
pub fn attach_postgres(
    database: &dyn Database,
    source_name: &str,
    source: &PostgresSource,
) -> Result<AttachedSource, ConnectError> {
    let mut connection = database.connect(&DatabaseLocation::InMemory)?;
    connection.attach_postgres(source)?;
    let declared = DatabaseEngine::PostgreSql;
    let params = [
        SqlValue::Text(source.alias.clone()),
        SqlValue::Text("SELECT version()".to_owned()),
    ];
    let answer = connection
        .query_arrow("SELECT * FROM postgres_query(?, ?)", &params)
        .and_then(|rows| {
            first_text(&rows)
                .ok_or_else(|| PortError::Failed("`version()` returned no text".to_owned()))
        });
    let error = match answer {
        Ok(version) if version.starts_with("PostgreSQL") => {
            return Ok(AttachedSource {
                connection,
                alias: source.alias.clone(),
                identity: EngineIdentity {
                    engine: declared,
                    version,
                },
            });
        }
        Ok(other) => ConnectError::EngineMismatch {
            source_name: source_name.to_owned(),
            declared,
            detected: Some(other),
            detail: String::new(),
        },
        Err(error) => unidentified(source_name, declared, &error),
    };
    // The mismatch is the error reported; the detach is a best effort.
    connection.detach(&source.alias).ok();
    Err(error)
}

/// The mismatch of an engine that cannot be identified because of `error`.
fn unidentified(source_name: &str, declared: DatabaseEngine, error: &PortError) -> ConnectError {
    ConnectError::EngineMismatch {
        source_name: source_name.to_owned(),
        declared,
        detected: None,
        detail: error.to_string(),
    }
}

/// The first value of the first column of `rows`, when it is text.
fn first_text(rows: &[RecordBatch]) -> Option<String> {
    let column = rows
        .iter()
        .find(|batch| batch.num_rows() > 0)?
        .columns()
        .first()?;
    if column.is_null(0) {
        return None;
    }
    match column.data_type() {
        DataType::Utf8 => Some(column.as_string::<i32>().value(0).to_owned()),
        DataType::LargeUtf8 => Some(column.as_string::<i64>().value(0).to_owned()),
        DataType::Utf8View => Some(column.as_string_view().value(0).to_owned()),
        _ => None,
    }
}
