//! The type passport (MG-REQ-0634): before execution, every column that crosses from a DuckDB
//! segment to a Polars segment is checked, and a lossy one is an error of the
//! `Type passport` gate.

use crate::types::lossy;
use crate::{Engine, Gate, GateError, PlanModel, segment};

/// The code of a column that loses information at a boundary (MG-REQ-0634.3).
const LOSSY_BOUNDARY: &str = "MG-0702";

/// The errors of the `Type passport` gate (MG-REQ-0634.3): one `MG-0702` per column that
/// crosses from a DuckDB segment to a Polars segment with a type that loses information
/// (MG-REQ-0634.2), naming the model, the step that outputs it, the column and its type. In
/// the order of `models`, then of their boundaries and columns.
#[must_use]
pub fn type_passport(models: &[PlanModel]) -> Vec<GateError> {
    let mut errors = Vec::new();
    for model in models {
        let planned = segment(&model.steps);
        for crossing in &planned.boundaries {
            let into_polars = planned.engine(crossing.from) == Some(Engine::Duckdb)
                && planned.engine(crossing.to) == Some(Engine::Polars);
            if !into_polars {
                continue;
            }
            for column in crossing
                .columns
                .iter()
                .filter(|column| lossy(&column.data_type))
            {
                errors.push(GateError {
                    gate: Gate::TypePassport,
                    code: LOSSY_BOUNDARY,
                    model: Some(model.name.clone()),
                    step: Some(column.step.clone()),
                    message: format!(
                        "the column `{}` of type `{}` loses information from DuckDB to Polars \
                         at the boundary from segment {} to segment {}",
                        column.name, column.data_type, crossing.from, crossing.to
                    ),
                    fix: format!(
                        "cast `{}` in the step `{}` to a type Polars keeps, for example \
                         `VARCHAR` or `DECIMAL(38, <scale>)`",
                        column.name, column.step
                    ),
                });
            }
        }
    }
    errors
}
