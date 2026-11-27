//! Parameter names wire the steps (MG-REQ-0614).

use std::collections::BTreeSet;

use mergus_ports::{PythonFunction, PythonParameter, PythonParameterKind};

use super::{InputSource, PythonStep, StepInput, to_usize};
use crate::names::closest_within;
use crate::{KnownTables, PythonModelError};

/// The farthest a known name may be from an unresolved one to be suggested (MG-REQ-0614.1).
const SUGGESTION_LIMIT: usize = 3;

/// Resolves the parameters of each step of `functions` into `steps`, which hold the same
/// steps in the same order, and adds to `problems` the variadic and unresolved parameters,
/// then the cycles between steps (MG-REQ-0614). `model` is the model name: a step's parameter
/// never resolves to the step itself, and the model is its result step.
pub(super) fn wire(
    path: &str,
    model: &str,
    functions: &[&PythonFunction],
    steps: &mut [PythonStep],
    known: &KnownTables,
    problems: &mut Vec<PythonModelError>,
) {
    let names: BTreeSet<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    let mut edges: Vec<Vec<usize>> = Vec::with_capacity(functions.len());
    for (function, step) in functions.iter().zip(steps.iter_mut()) {
        let mut reads = Vec::new();
        for parameter in &function.parameters {
            match input(path, model, &names, known, function, parameter) {
                Ok(input) => {
                    if let InputSource::Step(read) = &input.source
                        && let Some(index) = functions.iter().position(|f| f.name == *read)
                    {
                        reads.push(index);
                    }
                    step.inputs.push(input);
                }
                Err(problem) => problems.push(problem),
            }
        }
        edges.push(reads);
    }
    for cycle in cycles(&edges) {
        let steps: Vec<String> = cycle.iter().map(|&i| functions[i].name.clone()).collect();
        let line = cycle.first().map_or(0, |&i| to_usize(functions[i].line));
        problems.push(PythonModelError::StepCycle {
            path: path.to_owned(),
            steps,
            line,
        });
    }
}

/// The input of `parameter` of the step `function`, or why it has none (MG-REQ-0614.1,
/// MG-REQ-0614.2). `steps` names every step of the file.
fn input(
    path: &str,
    model: &str,
    steps: &BTreeSet<&str>,
    known: &KnownTables,
    function: &PythonFunction,
    parameter: &PythonParameter,
) -> Result<StepInput, PythonModelError> {
    let prefix = match parameter.kind {
        PythonParameterKind::VarPositional => "*",
        PythonParameterKind::VarKeyword => "**",
        _ => "",
    };
    let (line, column) = (to_usize(parameter.line), to_usize(parameter.column));
    if !prefix.is_empty() {
        return Err(PythonModelError::VariadicParameter {
            path: path.to_owned(),
            step: function.name.clone(),
            parameter: format!("{prefix}{}", parameter.name),
            line,
            column,
        });
    }
    let own = function.name.as_str();
    match resolve(&parameter.name, own, model, steps, known) {
        Some(source) => Ok(StepInput {
            parameter: parameter.name.clone(),
            source,
        }),
        None => Err(PythonModelError::UnresolvedParameter {
            path: path.to_owned(),
            step: function.name.clone(),
            parameter: parameter.name.clone(),
            line,
            column,
            suggestion: suggest(&parameter.name, own, model, steps, known),
        }),
    }
}

/// What `name`, a parameter of the step `own`, reads: a step of the file, a model, then a raw
/// table or seed (MG-REQ-0614).
fn resolve(
    name: &str,
    own: &str,
    model: &str,
    steps: &BTreeSet<&str>,
    known: &KnownTables,
) -> Option<InputSource> {
    if name != own && steps.contains(name) {
        Some(InputSource::Step(name.to_owned()))
    } else if name != model && known.models().contains(name) {
        Some(InputSource::Model(name.to_owned()))
    } else if known.raw_tables().contains(name) {
        Some(InputSource::Raw(name.to_owned()))
    } else {
        None
    }
}

/// The known name closest to `name`, within [`SUGGESTION_LIMIT`] edits (MG-REQ-0614.1).
fn suggest(
    name: &str,
    own: &str,
    model: &str,
    steps: &BTreeSet<&str>,
    known: &KnownTables,
) -> Option<String> {
    let models = known.models().iter().map(String::as_str);
    let raw = known.raw_tables().iter().map(String::as_str);
    let candidates: BTreeSet<&str> = steps
        .iter()
        .copied()
        .filter(|step| *step != own)
        .chain(models.filter(|known| *known != model))
        .chain(raw)
        .collect();
    closest_within(name, candidates, SUGGESTION_LIMIT).map(str::to_owned)
}

/// The cycles of the graph whose node `i` reads the nodes `edges[i]`, each from its node that
/// comes first, found by a depth-first walk in node order; each set of nodes once.
fn cycles(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut found: Vec<Vec<usize>> = Vec::new();
    let mut seen: BTreeSet<Vec<usize>> = BTreeSet::new();
    let mut done = vec![false; edges.len()];
    for start in 0..edges.len() {
        if done[start] {
            continue;
        }
        let mut path: Vec<usize> = vec![start];
        let mut pending: Vec<usize> = vec![0];
        while let (Some(&node), Some(next)) = (path.last(), pending.last_mut()) {
            let Some(&child) = edges[node].get(*next) else {
                done[node] = true;
                path.pop();
                pending.pop();
                continue;
            };
            *next += 1;
            if let Some(at) = path.iter().position(|&on| on == child) {
                let mut cycle = path[at..].to_vec();
                let first = cycle.iter().copied().min().unwrap_or(child);
                let shift = cycle.iter().position(|&n| n == first).unwrap_or(0);
                cycle.rotate_left(shift);
                let mut members = cycle.clone();
                members.sort_unstable();
                if seen.insert(members) {
                    found.push(cycle);
                }
            } else if !done[child] {
                path.push(child);
                pending.push(0);
            }
        }
    }
    found
}
