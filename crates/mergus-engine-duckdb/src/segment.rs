//! One segment: a DuckDB session that loads the project's macros, opens its inputs, runs the
//! step's SQL and leaves its rows at its output boundary.

use arrow::datatypes::{Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use mergus_ports::{Database, DatabaseConnection, DatabaseLocation, FileSystem, SqlValue};

use crate::checks::{declared, described, without_icu};
use crate::exact::{Zones, conform, duckdb_type, nullable, unrepresentable};
use crate::transport::{boundary, open_arrow, write_arrow};
use crate::{Boundary, EngineError, Transport};

/// The table a Parquet output is gathered in before it is written, with `_` added until no
/// input has its name.
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
    /// The boundaries the SQL reads, each as a table of its name.
    pub inputs: Vec<Table>,
    /// The step's output: its name, its declared columns and where its rows go.
    pub output: Table,
}

/// A table at a boundary: an input a segment reads, or the output it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// The name the segment's SQL reads an input by, or the name of the output.
    pub name: String,
    /// Its declared columns and their types: the rows cross the boundary in these types, or
    /// the step fails (MG-REQ-0180).
    pub schema: SchemaRef,
    /// Its file and the transport the plan chose for it.
    pub boundary: Boundary,
}

/// Runs `segment` in a new in-memory DuckDB session of `database`: executes each macro text (MG-REQ-0643), opens each input, runs the SQL, conforms its rows
/// to the declared output schema and writes them with the transport the plan recorded
/// (MG-REQ-0633).
///
/// Every declared schema is checked first. The session must not have `icu` loaded, so that
/// DuckDB gives zoned timestamps back in UTC: it is checked when the session opens, after the
/// macros and before the result is read. The step's result is described before it runs, and
/// every declared column must have its declared type, or a type it widens exactly, even when
/// there are no rows.
///
/// A shared-memory input is read as Arrow, conformed to its schema and appended to a table of
/// its name. A Parquet input is described first, every declared column must have its declared
/// type, then it becomes a view of its name that reads the file lazily. A shared-memory output
/// is written as an Arrow IPC file; a Parquet output by DuckDB's `COPY`. Both keep the
/// declared columns when there are no rows, and the declared time zones: DuckDB keeps only the
/// instant of a `TIMESTAMPTZ` and, without the `icu` extension, which the port never loads,
/// gives it back in UTC whatever the host's zone; the declared zone is put back on the same
/// instants.
///
/// # Errors
///
/// Fails with [`EngineError::Unrepresentable`] (`MG-0101`) when a declared schema has a type
/// DuckDB cannot hold exactly or a column twice, before the session opens; when a Parquet
/// input or the step's result, as DuckDB describes them, lacks a declared column or has it in
/// another type, whatever the number of rows; or when a value cannot be represented exactly
/// in its declared type, before anything is written. Fails with
/// [`EngineError::ZoneExtensionLoaded`] (`MG-0102`) when the session has `icu` loaded;
/// with [`EngineError::Boundary`] when a boundary file is not Arrow IPC; and with
/// [`EngineError::Port`] when DuckDB or the file system fails.
pub fn run_segment(
    database: &dyn Database,
    files: &dyn FileSystem,
    segment: &Segment,
) -> Result<(), EngineError> {
    for table in segment.inputs.iter().chain([&segment.output]) {
        declared(table)?;
    }
    let mut session = database.connect(&DatabaseLocation::InMemory)?;
    without_icu(session.as_mut())?;
    for text in &segment.macros {
        session.execute(text, &[])?;
    }
    without_icu(session.as_mut())?;
    for (index, input) in segment.inputs.iter().enumerate() {
        open_input(session.as_mut(), files, index, input)?;
    }
    let output = &segment.output;
    let describe = format!("DESCRIBE {}", segment.sql);
    let result = ("the step's result", true);
    described(session.as_mut(), &describe, &segment.params, output, result)?;
    without_icu(session.as_mut())?;
    let batches = session
        .query_arrow(&segment.sql, &segment.params)?
        .iter()
        .map(|batch| conform(&output.name, batch, &output.schema, Zones::Restore))
        .collect::<Result<Vec<_>, _>>()?;
    match output.boundary.transport {
        Transport::SharedMemory => {
            let schema: SchemaRef = nullable(&output.schema);
            write_arrow(files, &output.boundary.path, &schema, &batches)
        }
        Transport::Parquet => write_parquet(session.as_mut(), segment, &batches),
    }
}

/// Writes `batches`, the rows of the output of `segment`, as its Parquet file: gathered in a
/// table whose name no input of the segment has, then copied by DuckDB.
fn write_parquet(
    session: &mut dyn DatabaseConnection,
    segment: &Segment,
    batches: &[RecordBatch],
) -> Result<(), EngineError> {
    let output = &segment.output;
    let mut table = OUTPUT_TABLE.to_owned();
    while segment.inputs.iter().any(|input| input.name == table) {
        table.push('_');
    }
    create_table(session, &output.name, &table, &output.schema)?;
    for batch in batches {
        session.append_arrow(&table, batch)?;
    }
    let path = path_value(&output.boundary)?;
    let quoted = quote_identifier(&table);
    // sql: identifiers quoted by quote_identifier
    let copy = format!("COPY {quoted} TO ? (FORMAT parquet)");
    session.execute(&copy, &[path])?;
    Ok(())
}

/// Makes `input`, the `index`-th of its segment, readable by its name.
fn open_input(
    session: &mut dyn DatabaseConnection,
    files: &dyn FileSystem,
    index: usize,
    input: &Table,
) -> Result<(), EngineError> {
    match input.boundary.transport {
        Transport::SharedMemory => {
            let path = &input.boundary.path;
            let reader = open_arrow(files, path)?;
            create_table(session, &input.name, &input.name, &input.schema)?;
            for batch in reader {
                let batch = batch.map_err(|error| boundary(path, &error))?;
                let batch = conform(&input.name, &batch, &input.schema, Zones::Keep)?;
                session.append_arrow(&input.name, &batch)?;
            }
        }
        Transport::Parquet => {
            let variable = format!("mergus_input_{index}");
            let set = format!("SET VARIABLE {variable} = ?");
            session.execute(&set, &[path_value(&input.boundary)?])?;
            let file = format!("read_parquet(getvariable('{variable}'))");
            let describe = format!("DESCRIBE SELECT * FROM {file}");
            described(session, &describe, &[], input, ("the Parquet file", false))?;
            let name = quote_identifier(&input.name);
            let columns = input
                .schema
                .fields()
                .iter()
                .map(|field| quote_identifier(field.name()))
                .collect::<Vec<_>>()
                .join(", ");
            // sql: identifiers quoted by quote_identifier
            let view = format!("CREATE VIEW {name} AS SELECT {columns} FROM {file}");
            session.execute(&view, &[])?;
        }
    }
    Ok(())
}

/// Creates the table `table` with the columns of `schema`, each in the DuckDB type that holds
/// it exactly. `label` names the input or output in errors.
fn create_table(
    session: &mut dyn DatabaseConnection,
    label: &str,
    table: &str,
    schema: &Schema,
) -> Result<(), EngineError> {
    let mut columns = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let data_type = duckdb_type(field.data_type())
            .map_err(|reason| unrepresentable(label, field.name(), reason))?;
        columns.push(format!("{} {data_type}", quote_identifier(field.name())));
    }
    let table = quote_identifier(table);
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
