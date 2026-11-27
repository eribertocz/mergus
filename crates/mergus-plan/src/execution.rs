//! The execution fields of a step (MG-REQ-0747.1, MG-REQ-0747.4): its segments with their
//! Polars levels (MG-REQ-0635) and its boundaries with their transport (MG-REQ-0633), and the
//! conditions on them that the schema cannot express.

use mergus_contracts::generated::mergus_plan_v1::{
    Boundary, BoundaryTransport, Segment, SegmentEngine, SegmentPolarsLevel, SegmentPolarsReason,
    Step,
};

use crate::types::width;
use crate::{Crossing, Engine, PlanError, PlanModel, segment};

/// The largest boundary, in bytes, that crosses as Arrow (MG-REQ-0633).
const ARROW_LIMIT: u128 = 1 << 30;

/// The Polars levels, from highest to lowest (MG-REQ-0635.1).
const LEVELS: [SegmentPolarsLevel; 3] = [
    SegmentPolarsLevel::Native,
    SegmentPolarsLevel::InProcess,
    SegmentPolarsLevel::Subprocess,
];

/// The Polars levels of this build: `native` and `in_process` are not available yet
/// (MG-REQ-0635.3).
const AVAILABLE: [SegmentPolarsLevel; 1] = [SegmentPolarsLevel::Subprocess];

/// The `segments` and `boundaries` of the step of `model`, whose boundaries carry about
/// `rows` rows when the caller estimates them; both `None` when the model has no steps.
pub(crate) fn fields(
    model: &PlanModel,
    rows: Option<u64>,
) -> (Option<Vec<Segment>>, Option<Vec<Boundary>>) {
    if model.steps.is_empty() {
        return (None, None);
    }
    let planned = segment(&model.steps);
    let segments = planned
        .segments
        .iter()
        .map(|fused| {
            let (engine, level) = match fused.engine {
                Engine::Duckdb => (SegmentEngine::Duckdb, None),
                Engine::Polars => (SegmentEngine::Polars, Some(polars_level())),
            };
            Segment {
                index: index(fused.index),
                engine,
                steps: fused.steps.clone(),
                polars_level: level.map(|(level, _)| level),
                polars_reason: level.map(|(_, reason)| reason),
            }
        })
        .collect();
    let boundaries = planned
        .boundaries
        .iter()
        .map(|crossing| Boundary {
            from: index(crossing.from),
            to: index(crossing.to),
            columns: names(crossing),
            transport: transport(crossing, rows),
        })
        .collect();
    (Some(segments), Some(boundaries))
}

/// The level of a Polars segment and its reason (MG-REQ-0635): the highest level available,
/// `qualified` when it is the highest level, `not_available` when the higher levels are not
/// in this build. No step is refused a level in 0.4, so the lowest level its steps qualify
/// for is the highest available one (MG-REQ-0635.1, MG-REQ-0635.3).
fn polars_level() -> (SegmentPolarsLevel, SegmentPolarsReason) {
    let level = LEVELS
        .iter()
        .copied()
        .find(|level| AVAILABLE.contains(level))
        .unwrap_or(SegmentPolarsLevel::Subprocess);
    let reason = if LEVELS.first() == Some(&level) {
        SegmentPolarsReason::Qualified
    } else {
        SegmentPolarsReason::NotAvailable
    };
    (level, reason)
}

/// The transport of `crossing` (MG-REQ-0633): `arrow` when the estimated rows times the
/// widths of its columns is at most 1 GiB, `parquet` above that or with no estimate.
fn transport(crossing: &Crossing, rows: Option<u64>) -> BoundaryTransport {
    let Some(rows) = rows else {
        return BoundaryTransport::Parquet;
    };
    let row_width: u128 = crossing
        .columns
        .iter()
        .map(|column| u128::from(width(&column.data_type)))
        .sum();
    if u128::from(rows).saturating_mul(row_width) <= ARROW_LIMIT {
        BoundaryTransport::Arrow
    } else {
        BoundaryTransport::Parquet
    }
}

/// The names of the columns of `crossing`, each once, in their output order: two steps may
/// output columns of one name.
fn names(crossing: &Crossing) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for column in &crossing.columns {
        if !names.contains(&column.name) {
            names.push(column.name.clone());
        }
    }
    names
}

/// `index` as a contract integer.
fn index(index: usize) -> i64 {
    i64::try_from(index).unwrap_or(i64::MAX)
}

/// Checks the conditions of MG-REQ-0747.4 that the schema cannot express: `polars_level`
/// and `polars_reason` only on a Polars segment, and boundaries sorted by `from`, then `to`.
///
/// # Errors
///
/// Fails with [`PlanError::ExecutionFields`] on the first step that breaks one.
pub(crate) fn check(steps: &[Step]) -> Result<(), PlanError> {
    for step in steps {
        let polars_only = step.segments.iter().flatten().all(|segment| {
            segment.engine == SegmentEngine::Polars
                || (segment.polars_level.is_none() && segment.polars_reason.is_none())
        });
        if !polars_only {
            return Err(PlanError::ExecutionFields {
                model: step.model.clone(),
                broken: "a segment that is not Polars has a Polars level",
            });
        }
        let sorted = step.boundaries.as_ref().is_none_or(|boundaries| {
            boundaries
                .windows(2)
                .all(|pair| (pair[0].from, pair[0].to) < (pair[1].from, pair[1].to))
        });
        if !sorted {
            return Err(PlanError::ExecutionFields {
                model: step.model.clone(),
                broken: "its boundaries are not sorted by `from`, then `to`",
            });
        }
    }
    Ok(())
}
