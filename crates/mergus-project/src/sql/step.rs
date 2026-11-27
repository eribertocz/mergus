//! A `.sql` model as a step: one query (MG-REQ-0601) whose run parameters are bound
//! (MG-REQ-0603) and whose schema DuckDB gives by preparing it (MG-REQ-0604).

use std::collections::BTreeMap;

use mergus_ports::{DatabaseConnection, PortError, ResultColumn, SqlParser, SqlValue};

use super::{KnownTables, walk_sql};
use crate::LayoutError;

/// A `.sql` model that passed the checks of MG-REQ-0601 and MG-REQ-0374.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlStep {
    /// The path of the model, relative to the project root.
    pub path: String,
    /// The text of the model, exactly as in its file: values are never written into it.
    pub text: String,
    /// The named parameters of the query (`$run_date` is `run_date`), in the order DuckDB
    /// numbers them, which is the order [`SqlStep::bind`] gives their values in.
    pub parameters: Vec<String>,
}

/// A run gives no value for a parameter its model names (MG-REQ-0603).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("`{path}`: the run gives no value for parameter `${name}`")]
pub struct MissingParameter {
    /// The path of the model.
    pub path: String,
    /// The parameter, without its `$`.
    pub name: String,
}

impl SqlStep {
    /// Returns the values of `values` for the parameters of the step, in the order of
    /// [`SqlStep::parameters`], to bind with [`SqlStep::text`]. Values the step does not name
    /// are ignored.
    ///
    /// # Errors
    ///
    /// Fails with [`MissingParameter`] for the first parameter that `values` has no value for.
    pub fn bind(
        &self,
        values: &BTreeMap<String, SqlValue>,
    ) -> Result<Vec<SqlValue>, MissingParameter> {
        self.parameters
            .iter()
            .map(|name| {
                values.get(name).cloned().ok_or_else(|| MissingParameter {
                    path: self.path.clone(),
                    name: name.clone(),
                })
            })
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
    let text = std::str::from_utf8(bytes).map_or_else(|_| String::new(), str::to_owned);
    Ok(SqlStep {
        path: path.to_owned(),
        text,
        parameters: checked.parameters,
    })
}

/// Returns the columns of the result of `step`, which `connection` prepares without running
/// it, so no data is read (MG-REQ-0604).
///
/// # Errors
///
/// Fails with the [`PortError`] of the connection when DuckDB cannot prepare the step.
pub fn sql_schema(
    connection: &mut dyn DatabaseConnection,
    step: &SqlStep,
) -> Result<Vec<ResultColumn>, PortError> {
    connection.prepare(&step.text)
}
