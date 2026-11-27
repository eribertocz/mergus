//! A `.sql` model as a step: one query (MG-REQ-0601) whose run parameters are bound
//! (MG-REQ-0603) and whose schema DuckDB gives against empty inputs (MG-REQ-0604).

use std::collections::BTreeMap;

use mergus_ports::{Database, DatabaseLocation, PortError, ResultColumn, SqlParser, SqlValue};

use super::{KnownTables, walk_sql};
use crate::{LayoutError, ProjectError};

/// A `.sql` model that passed the checks of MG-REQ-0374 and MG-REQ-0603.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlStep {
    /// The path of the model, relative to the project root.
    pub path: String,
    /// The text of the model, exactly as in its file.
    pub text: String,
    /// The statement Mergus sends: [`SqlStep::text`] with each `$period_start` and
    /// `$period_end` read as `CAST($period_start AS TIMESTAMPTZ)`, so that DuckDB types them
    /// `TIMESTAMPTZ` from the bound instant (see [`SqlValue::TimestampTz`]). Values are never
    /// written into it.
    pub sql: String,
    /// The run parameters the query names (`$run_date` is `run_date`), in the order DuckDB
    /// numbers them, which is the order [`SqlStep::bind`] gives their values in.
    pub parameters: Vec<String>,
}

/// The values of the run parameters, each of its fixed type (MG-REQ-0603). The default, all
/// zero, gives the typed placeholders of MG-REQ-0604.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunParameters {
    /// `$run_date`, a `DATE`: days since 1970-01-01.
    pub run_date: i32,
    /// `$period_start`, a `TIMESTAMPTZ`: microseconds since 1970-01-01 00:00:00 UTC.
    pub period_start: i64,
    /// `$period_end`, a `TIMESTAMPTZ`: microseconds since 1970-01-01 00:00:00 UTC.
    pub period_end: i64,
}

impl RunParameters {
    /// The value of the run parameter `name`, when it is one.
    fn value(&self, name: &str) -> Option<SqlValue> {
        match name {
            "run_date" => Some(SqlValue::Date(self.run_date)),
            "period_start" => Some(SqlValue::TimestampTz(self.period_start)),
            "period_end" => Some(SqlValue::TimestampTz(self.period_end)),
            _ => None,
        }
    }
}

impl SqlStep {
    /// Returns the values of `run` for the parameters of the step, in the order of
    /// [`SqlStep::parameters`], to bind with [`SqlStep::sql`].
    #[must_use]
    pub fn bind(&self, run: &RunParameters) -> Vec<SqlValue> {
        self.parameters
            .iter()
            .filter_map(|name| run.value(name))
            .collect()
    }
}

/// Reads the `.sql` model at `path` holding `bytes`, parsed by `parser`, as a step.
///
/// # Errors
///
/// Fails with [`LayoutError::Invalid`] carrying the problems of [`super::check_sql`] when it
/// has any, and with [`LayoutError::Port`] when the parser cannot be reached.
pub fn read_sql_step(
    path: &str,
    bytes: &[u8],
    known: &KnownTables,
    parser: &dyn SqlParser,
) -> Result<SqlStep, LayoutError> {
    let checked = walk_sql(path, bytes, known, parser)?;
    if !checked.problems.is_empty() {
        return Err(LayoutError::Invalid(checked.problems));
    }
    let text = String::from_utf8_lossy(bytes).into_owned();
    let sql = typed(&text, &checked.instants);
    Ok(SqlStep {
        path: path.to_owned(),
        text,
        sql,
        parameters: checked.parameters,
    })
}

/// Returns `text` with the parameter at each byte range of `instants` wrapped in
/// `CAST(… AS TIMESTAMPTZ)`.
fn typed(text: &str, instants: &[(usize, usize)]) -> String {
    let mut ranges = instants.to_vec();
    ranges.sort_unstable();
    ranges.dedup();
    let mut sql = String::with_capacity(text.len() + ranges.len() * 21);
    let mut copied = 0;
    for (start, end) in ranges {
        if let (Some(before), Some(parameter)) = (text.get(copied..start), text.get(start..end)) {
            sql.push_str(before);
            sql.push_str("CAST(");
            sql.push_str(parameter);
            sql.push_str(" AS TIMESTAMPTZ)");
            copied = end;
        }
    }
    sql.push_str(text.get(copied..).unwrap_or(""));
    sql
}

/// Returns the columns of the result of `step` (MG-REQ-0604). A new in-memory connection of
/// `database` holds one empty table per input of `inputs`, by reference (`<model>` or
/// `raw.<table>`) with its columns; DuckDB then describes the step with a typed placeholder
/// for each run parameter. No data exists, so none is read.
///
/// # Errors
///
/// Fails with [`LayoutError::Invalid`] carrying [`ProjectError::UnpreparableSql`] when DuckDB
/// refuses the step ([`PortError::Refused`]), and with [`LayoutError::Port`] when the
/// database fails otherwise.
pub fn sql_schema(
    database: &dyn Database,
    step: &SqlStep,
    inputs: &BTreeMap<String, Vec<ResultColumn>>,
) -> Result<Vec<ResultColumn>, LayoutError> {
    let mut connection = database.connect(&DatabaseLocation::InMemory)?;
    for (reference, columns) in inputs {
        let (schema, table) = reference
            .split_once('.')
            .map_or((None, reference.as_str()), |(schema, table)| {
                (Some(schema), table)
            });
        connection.create_empty_table(schema, table, columns)?;
    }
    let placeholders = step.bind(&RunParameters::default());
    match connection.describe(&step.sql, &placeholders) {
        Ok(columns) => Ok(columns),
        Err(PortError::Refused(message)) => {
            Err(LayoutError::Invalid(vec![ProjectError::UnpreparableSql {
                path: step.path.clone(),
                message,
            }]))
        }
        Err(other) => Err(LayoutError::Port(other)),
    }
}
