//! The segments of a model and the columns crossing between them (MG-REQ-0631,
//! MG-REQ-0632).

use std::collections::{BTreeMap, BTreeSet};

use crate::Detected;

/// The engine that runs a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Engine {
    /// DuckDB.
    Duckdb,
    /// Polars.
    Polars,
}

/// A column a step outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputColumn {
    /// The column name.
    pub name: String,
    /// Its DuckDB type name, as `DESCRIBE` writes it; for a Polars step, the type of its
    /// empty frame mapped to a DuckDB name (MG-REQ-0634.1).
    pub data_type: String,
}

/// A column a step reads from an earlier step of the same model.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnRead {
    /// The step that outputs the column.
    pub step: String,
    /// The column.
    pub column: String,
}

/// One step of a model, in execution order, as the planner needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelStep {
    /// The step name.
    pub name: String,
    /// The engine that runs it.
    pub engine: Engine,
    /// The columns it outputs, in their output order.
    pub columns: Vec<OutputColumn>,
    /// The columns it reads from earlier steps of the model.
    pub reads: Vec<ColumnRead>,
    /// The non-deterministic operations found in its code (MG-REQ-0637).
    pub detected: Vec<Detected>,
}

/// Consecutive steps of one engine, fused (MG-REQ-0631).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fused {
    /// The position of the segment in execution order, from 0.
    pub index: usize,
    /// The engine that runs every step of the segment.
    pub engine: Engine,
    /// The names of the steps, in order.
    pub steps: Vec<String>,
}

/// One column crossing a boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrossingColumn {
    /// The step that outputs it.
    pub step: String,
    /// The column name.
    pub name: String,
    /// Its DuckDB type name.
    pub data_type: String,
}

/// The columns that cross from one segment to a later one (MG-REQ-0632).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crossing {
    /// The index of the segment the data leaves.
    pub from: usize,
    /// The index of the segment the data enters.
    pub to: usize,
    /// The columns the entering segment uses, each once, in their output order.
    pub columns: Vec<CrossingColumn>,
}

/// The segments of a model and the crossings between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segments {
    /// The segments, in execution order.
    pub segments: Vec<Fused>,
    /// The crossings, sorted by `from`, then `to`.
    pub boundaries: Vec<Crossing>,
}

impl Segments {
    /// The engine of the segment `index`.
    pub(crate) fn engine(&self, index: usize) -> Option<Engine> {
        self.segments.get(index).map(|segment| segment.engine)
    }
}

/// Fuses consecutive steps of the same engine into one segment (MG-REQ-0631) and finds, for
/// each pair of segments, the columns the later one reads from the earlier one, and only
/// those (MG-REQ-0632). A read of a step that is not an earlier step of `steps` crosses
/// nothing.
#[must_use]
pub fn segment(steps: &[ModelStep]) -> Segments {
    let mut segments: Vec<Fused> = Vec::new();
    let mut segment_of: BTreeMap<&str, usize> = BTreeMap::new();
    for step in steps {
        match segments.last_mut() {
            Some(last) if last.engine == step.engine => last.steps.push(step.name.clone()),
            _ => segments.push(Fused {
                index: segments.len(),
                engine: step.engine,
                steps: vec![step.name.clone()],
            }),
        }
        segment_of.insert(&step.name, segments.len().saturating_sub(1));
    }
    Segments {
        boundaries: crossings(steps, &segment_of),
        segments,
    }
}

/// The crossings of `steps`, whose segments are `segment_of`, sorted by `from`, then `to`.
fn crossings(steps: &[ModelStep], segment_of: &BTreeMap<&str, usize>) -> Vec<Crossing> {
    let mut used: BTreeMap<(usize, usize), BTreeSet<(&str, &str)>> = BTreeMap::new();
    for step in steps {
        let Some(&to) = segment_of.get(step.name.as_str()) else {
            continue;
        };
        for read in &step.reads {
            match segment_of.get(read.step.as_str()) {
                Some(&from) if from < to => {
                    used.entry((from, to))
                        .or_default()
                        .insert((&read.step, &read.column));
                }
                _ => {}
            }
        }
    }
    used.into_iter()
        .map(|((from, to), reads)| Crossing {
            from,
            to,
            columns: in_output_order(steps, &reads),
        })
        .collect()
}

/// The columns of `reads` that their steps output, each name once, in the output order of
/// the steps.
fn in_output_order(steps: &[ModelStep], reads: &BTreeSet<(&str, &str)>) -> Vec<CrossingColumn> {
    let mut columns: Vec<CrossingColumn> = Vec::new();
    for step in steps {
        for column in &step.columns {
            let read = reads.contains(&(step.name.as_str(), column.name.as_str()));
            if read && columns.iter().all(|seen| seen.name != column.name) {
                columns.push(CrossingColumn {
                    step: step.name.clone(),
                    name: column.name.clone(),
                    data_type: column.data_type.clone(),
                });
            }
        }
    }
    columns
}
