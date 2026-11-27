//! One segment: a DuckDB session that loads the project's macros, opens its inputs, runs the
//! step's SQL and leaves its rows at its output boundary.

use arrow::datatypes::{Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use mergus_ports::{Database, DatabaseConnection, DatabaseLocation, FileSystem, SqlValue};

use crate::exact::{conform, duckdb_type};
use crate::transport::{read_arrow, write_arrow};
use crate::{Boundary, EngineError, Transport};

/// The table a Parquet output is gathered in before it is written.
const OUTPUT_TABLE: &str = "mergus_output";

/// What a segment runs: the input of this crate, onto which the runtime maps the plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// The step's SQL.
    pub sql: String,
    /// The values bound to its parameters, in order (MG-REQ-0007).
    pub params: Vec<SqlValue>,
    /// The text of each of the project's macro files, in the order they are loaded.
    pub macros: Vec<String>,
    /// The boundaries the SQL reads.
    pub inputs: Vec<Input>,
    /// Where its rows go, with the transport the plan chose.
    pub output: Boundary,
}

/// A boundary a segment reads, as a table of its own name.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// The name the segment's SQL reads it by.
    pub name: String,
    /// Its columns and their types: the rows reach DuckDB in these types, or the step fails
    /// (MG-REQ-0180).
    pub schema: SchemaRef,
    /// Its file and transport.
    pub boundary: Boundary,
}

/// Runs `segment` in a new in-memory DuckDB session of `database`: executes each macro text
/// first (MG-REQ-0643), opens each input, runs the SQL and writes its rows to the output
/// boundary with the transport the plan recorded (MG-REQ-0633).
///
/// A shared-memory input is read as Arrow, conformed to its schema and appended to a table of
/// its name; a Parquet input becomes a view of its name that reads the file lazily. A
/// shared-memory output is written as an Arrow IPC file; a Parquet output is written by
/// DuckDB's `COPY`.
///
/// # Errors
///
/// Fails with [`EngineError::Unrepresentable`] (`MG-0101`) when a value of an input cannot be
/// represented exactly in its declared type, before the SQL runs; with
/// [`EngineError::Boundary`] when a boundary file is not Arrow IPC; and with
/// [`EngineError::Port`] when DuckDB or the file system fails.
pub fn run_segment(
    database: &dyn Database,
    files: &dyn FileSystem,
    segment: &Segment,
) -> Result<(), EngineError> {
    let mut session = database.connect(&DatabaseLocation::InMemory)?;
    for text in &segment.macros {
        session.execute(text, &[])?;
    }
    for (index, input) in segment.inputs.iter().enumerate() {
        open_input(session.as_mut(), files, index, input)?;
    }
    let batches = session.query_arrow(&segment.sql, &segment.params)?;
    let schema = batches
        .first()
        .map_or_else(|| SchemaRef::new(Schema::empty()), RecordBatch::schema);
    match segment.output.transport {
        Transport::SharedMemory => write_arrow(files, &segment.output.path, &schema, &batches),
        Transport::Parquet => {
            create_table(session.as_mut(), OUTPUT_TABLE, &schema)?;
            for batch in &batches {
                session.append_arrow(OUTPUT_TABLE, batch)?;
            }
            let path = path_value(&segment.output)?;
            session.execute("COPY \"mergus_output\" TO ? (FORMAT parquet)", &[path])?;
            Ok(())
        }
    }
}

/// Makes `input`, the `index`-th of its segment, readable by its name.
fn open_input(
    session: &mut dyn DatabaseConnection,
    files: &dyn FileSystem,
    index: usize,
    input: &Input,
) -> Result<(), EngineError> {
    match input.boundary.transport {
        Transport::SharedMemory => {
            let batches = read_arrow(files, &input.boundary.path)?;
            let conformed = batches
                .iter()
                .map(|batch| conform(&input.name, batch, &input.schema))
                .collect::<Result<Vec<_>, _>>()?;
            create_table(session, &input.name, &input.schema)?;
            for batch in &conformed {
                session.append_arrow(&input.name, batch)?;
            }
        }
        Transport::Parquet => {
            let variable = format!("mergus_input_{index}");
            let set = format!("SET VARIABLE {variable} = ?");
            session.execute(&set, &[path_value(&input.boundary)?])?;
            let name = quote_identifier(&input.name);
            let view = format!(
                // sql: identifiers quoted by quote_identifier
                "CREATE VIEW {name} AS SELECT * FROM read_parquet(getvariable('{variable}'))"
            );
            session.execute(&view, &[])?;
        }
    }
    Ok(())
}

/// Creates the table `name` with the columns of `schema`, each in the DuckDB type that holds
/// it exactly.
fn create_table(
    session: &mut dyn DatabaseConnection,
    name: &str,
    schema: &Schema,
) -> Result<(), EngineError> {
    let mut columns = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let data_type =
            duckdb_type(field.data_type()).map_err(|reason| EngineError::Unrepresentable {
                input: name.to_owned(),
                column: field.name().clone(),
                reason,
            })?;
        columns.push(format!("{} {data_type}", quote_identifier(field.name())));
    }
    let table = quote_identifier(name);
    let columns = columns.join(", ");
    // sql: identifiers quoted by quote_identifier
    let create = format!("CREATE TABLE {table} ({columns})");
    session.execute(&create, &[])?;
    Ok(())
}

/// Returns the path of `boundary` as a bound value.
fn path_value(boundary: &Boundary) -> Result<SqlValue, EngineError> {
    boundary
        .path
        .to_str()
        .map(|path| SqlValue::Text(path.to_owned()))
        .ok_or_else(|| EngineError::Boundary {
            path: boundary.path.display().to_string(),
            reason: "the path is not valid Unicode".to_owned(),
        })
}

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
