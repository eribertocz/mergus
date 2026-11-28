//! The work of a model run by DuckDB in one segment, through `mergus-engine-duckdb`.

use arrow::datatypes::SchemaRef;
use mergus_engine_duckdb::{Boundary, Segment, Table, Transport, run_segment};
use mergus_ports::{Database, FileSystem, SqlValue};

use super::{ModelWork, StepContext, StepFailure};

/// One input of a [`SqlWork`].
#[derive(Clone, Debug, PartialEq)]
pub struct SqlInput {
    /// The name the SQL reads it by.
    pub name: String,
    /// Its declared columns and their types.
    pub schema: SchemaRef,
    /// Where its rows are; `None` for an input that is a step of the plan, whose result
    /// file stage 5 gives as shared memory.
    pub boundary: Option<Boundary>,
}

/// A model of one DuckDB segment: its SQL, bound parameters, macros, inputs and declared
/// output, run by [`run_segment`] in its own session.
pub struct SqlWork<'a> {
    /// The database the segment's session opens on.
    pub database: &'a dyn Database,
    /// The file system of the boundaries.
    pub files: &'a dyn FileSystem,
    /// The step's SQL.
    pub sql: String,
    /// The values bound to its parameters, in order (MG-REQ-0007).
    pub params: Vec<SqlValue>,
    /// The project's macro texts, in the order they are loaded.
    pub macros: Vec<String>,
    /// Its inputs.
    pub inputs: Vec<SqlInput>,
    /// Its declared output columns and their types.
    pub output: SchemaRef,
}

impl ModelWork for SqlWork<'_> {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        let failure = |code, message| StepFailure {
            name: context.model.to_owned(),
            code,
            message,
        };
        let mut inputs = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let boundary = match &input.boundary {
                Some(boundary) => boundary.clone(),
                None => Boundary {
                    transport: Transport::SharedMemory,
                    path: context.inputs.get(&input.name).cloned().ok_or_else(|| {
                        failure(
                            None,
                            format!("internal error: no result for `{}`", input.name),
                        )
                    })?,
                },
            };
            inputs.push(Table {
                name: input.name.clone(),
                schema: input.schema.clone(),
                boundary,
            });
        }
        let segment = Segment {
            sql: self.sql.clone(),
            params: self.params.clone(),
            macros: self.macros.clone(),
            inputs,
            output: Table {
                name: context.model.to_owned(),
                schema: self.output.clone(),
                boundary: Boundary {
                    transport: Transport::SharedMemory,
                    path: context.output.to_path_buf(),
                },
            },
        };
        run_segment(self.database, self.files, &segment)
            .map_err(|error| failure(error.code(), error.to_string()))
    }
}
