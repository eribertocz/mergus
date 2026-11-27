//! One segment: a DuckDB session that loads the project's macros, opens its inputs, runs the
//! step's SQL and leaves its rows at its output boundary.

use arrow::array::AsArray;
use arrow::datatypes::{Schema, SchemaRef};
use mergus_ports::{Database, DatabaseConnection, DatabaseLocation, FileSystem, SqlValue};

use crate::exact::{Zones, conform, duckdb_type, nullable, unrepresentable};
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
/// Fails with [`EngineError::Unrepresentable`] (`MG-0101`) when a value of an input or of the
/// output cannot be represented exactly in its declared type, before anything is written;
/// with [`EngineError::Boundary`] when a boundary file is not Arrow IPC; and with
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
    let output = &segment.output;
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
        Transport::Parquet => {
            create_table(session.as_mut(), &output.name, OUTPUT_TABLE, &output.schema)?;
            for batch in &batches {
                session.append_arrow(OUTPUT_TABLE, batch)?;
            }
            let path = path_value(&output.boundary)?;
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
    input: &Table,
) -> Result<(), EngineError> {
    match input.boundary.transport {
        Transport::SharedMemory => {
            let batches = read_arrow(files, &input.boundary.path)?;
            let conformed = batches
                .iter()
                .map(|batch| conform(&input.name, batch, &input.schema, Zones::Keep))
                .collect::<Result<Vec<_>, _>>()?;
            create_table(session, &input.name, &input.name, &input.schema)?;
            for batch in &conformed {
                session.append_arrow(&input.name, batch)?;
            }
        }
        Transport::Parquet => {
            let variable = format!("mergus_input_{index}");
            let set = format!("SET VARIABLE {variable} = ?");
            session.execute(&set, &[path_value(&input.boundary)?])?;
            let file = format!("read_parquet(getvariable('{variable}'))");
            check_parquet(session, input, &file)?;
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

/// Returns the statement that describes the columns of the Parquet file `file` reads.
fn describe(file: &str) -> String {
    format!("DESCRIBE SELECT * FROM {file}")
}

/// Checks that the Parquet file `file` reads holds every declared column of `input` in the
/// DuckDB type of its declared type (MG-REQ-0180).
fn check_parquet(
    session: &mut dyn DatabaseConnection,
    input: &Table,
    file: &str,
) -> Result<(), EngineError> {
    let mut found = Vec::new();
    for batch in &session.query_arrow(&describe(file), &[])? {
        let text = |name: &str| batch.column_by_name(name)?.as_string_opt::<i32>().cloned();
        let (Some(names), Some(types)) = (text("column_name"), text("column_type")) else {
            let reason = "DuckDB did not describe the Parquet file".to_owned();
            return Err(unrepresentable(&input.name, "", reason));
        };
        for (name, data_type) in names.iter().zip(types.iter()) {
            found.push((
                name.unwrap_or_default().to_owned(),
                data_type.unwrap_or_default().to_owned(),
            ));
        }
    }
    for field in input.schema.fields() {
        let declared = duckdb_type(field.data_type())
            .map_err(|reason| unrepresentable(&input.name, field.name(), reason))?;
        let reason = match found.iter().find(|(name, _)| name == field.name()) {
            Some((_, actual)) if *actual == declared => continue,
            Some((_, actual)) => {
                format!("the Parquet file holds {actual}, the schema declares {declared}")
            }
            None => "the Parquet file has no such column".to_owned(),
        };
        return Err(unrepresentable(&input.name, field.name(), reason));
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
