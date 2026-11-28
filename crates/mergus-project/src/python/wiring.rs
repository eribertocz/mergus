//! Parameter names wire the steps (MG-REQ-0614).

use std::collections::BTreeSet;

use mergus_ports::{PythonFunction, PythonParameter, PythonParameterKind};

use super::{InputSource, PythonStep, StepInput, to_usize};
use crate::names::closest_within;
use crate::{KnownTables, PythonModelError};

/// The farthest a known name may be from an unresolved one to be suggested (MG-REQ-0614.1).
const SUGGESTION_LIMIT: usize = 3;

/// The names a parameter of a file may read, besides its steps.
pub(super) struct Names<'a> {
    /// The model name. The model is its result step, so no step reads it as a model.
    pub(super) model: &'a str,
    /// The public `async def` functions: rejected steps, already reported, so a parameter
    /// naming one is neither an input nor a problem.
    pub(super) rejected: &'a BTreeSet<&'a str>,
    /// The models, raw tables and seeds of the project.
    pub(super) known: &'a KnownTables,
}

/// Resolves the parameters of each step of `functions` into `steps`, which hold the same
/// steps in the same order, and adds to `problems` the variadic, positional-only and
/// unresolved parameters, then the cycles between steps (MG-REQ-0614). A step's parameter
/// never resolves to the step itself.
pub(super) fn wire(
    path: &str,
    names: &Names<'_>,
    functions: &[&PythonFunction],
    steps: &mut [PythonStep],
    problems: &mut Vec<PythonModelError>,
) {
    let all: BTreeSet<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    let mut edges: Vec<Vec<usize>> = Vec::with_capacity(functions.len());
    for (function, step) in functions.iter().zip(steps.iter_mut()) {
        let mut reads = Vec::new();
        for parameter in &function.parameters {
            match input(path, names, &all, function, parameter) {
                Ok(Some(input)) => {
                    if let InputSource::Step(read) = &input.source
                        && let Some(index) = functions.iter().position(|f| f.name == *read)
                    {
                        reads.push(index);
                    }
                    step.inputs.push(input);
                }
                Ok(None) => {}
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

/// The input of `parameter` of the step `function`, none when it names a rejected step, or
/// why it has none (MG-REQ-0614.1, MG-REQ-0614.2). `steps` names every step of the file.
fn input(
    path: &str,
    names: &Names<'_>,
    steps: &BTreeSet<&str>,
    function: &PythonFunction,
    parameter: &PythonParameter,
) -> Result<Option<StepInput>, PythonModelError> {
    if let Some(problem) = kind_problem(path, function, parameter) {
        return Err(problem);
    }
    if names.rejected.contains(parameter.name.as_str()) {
        return Ok(None);
    }
    let candidates = candidates(names, steps, &function.name);
    if let Some(source) = resolve(&parameter.name, &candidates) {
        let parameter = parameter.name.clone();
        return Ok(Some(StepInput { parameter, source }));
    }
    let suggestion = closest_within(&parameter.name, candidates.all(), SUGGESTION_LIMIT);
    Err(PythonModelError::UnresolvedParameter {
        path: path.to_owned(),
        step: function.name.clone(),
        parameter: parameter.name.clone(),
        line: to_usize(parameter.line),
        column: to_usize(parameter.column),
        suggestion: suggestion.map(str::to_owned),
    })
}

/// The problem of `parameter` of the step `function` when its kind takes no input by
/// keyword: `*args`, `**kwargs` or positional-only (MG-REQ-0614.2).
fn kind_problem(
    path: &str,
    function: &PythonFunction,
    parameter: &PythonParameter,
) -> Option<PythonModelError> {
    let (line, column) = (to_usize(parameter.line), to_usize(parameter.column));
    let (path, step) = (path.to_owned(), function.name.clone());
    let prefix = match parameter.kind {
        PythonParameterKind::VarPositional => "*",
        PythonParameterKind::VarKeyword => "**",
        PythonParameterKind::PositionalOnly => {
            return Some(PythonModelError::PositionalOnlyParameter {
                path,
                step,
                parameter: parameter.name.clone(),
                line,
                column,
            });
        }
        PythonParameterKind::Positional | PythonParameterKind::KeywordOnly => return None,
    };
    Some(PythonModelError::VariadicParameter {
        path,
        step,
        parameter: format!("{prefix}{}", parameter.name),
        line,
        column,
    })
}

/// What a parameter of one step may name, by kind of input.
struct Candidates<'a> {
    /// The other steps of the file.
    steps: BTreeSet<&'a str>,
    /// The models, the file's own excepted.
    models: BTreeSet<&'a str>,
    /// The raw tables and seeds, without their schema.
    raw: BTreeSet<&'a str>,
}

impl<'a> Candidates<'a> {
    /// Every candidate, sorted, each once.
    fn all(&self) -> BTreeSet<&'a str> {
        let all = self.steps.iter().chain(&self.models).chain(&self.raw);
        all.copied().collect()
    }
}

/// The candidates of a parameter of the step `own`.
fn candidates<'a>(names: &Names<'a>, steps: &BTreeSet<&'a str>, own: &str) -> Candidates<'a> {
    let models = names.known.models().iter().map(String::as_str);
    let raw = names.known.raw_tables().iter().map(String::as_str);
    Candidates {
        steps: steps.iter().copied().filter(|step| *step != own).collect(),
        models: models.filter(|model| *model != names.model).collect(),
        raw: raw.collect(),
    }
}

/// What `name` reads: a step of the file, a model, then a raw table or seed (MG-REQ-0614).
fn resolve(name: &str, candidates: &Candidates<'_>) -> Option<InputSource> {
    if candidates.steps.contains(name) {
        Some(InputSource::Step(name.to_owned()))
    } else if candidates.models.contains(name) {
        Some(InputSource::Model(name.to_owned()))
    } else if candidates.raw.contains(name) {
        Some(InputSource::Raw(name.to_owned()))
    } else {
        None
    }
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
