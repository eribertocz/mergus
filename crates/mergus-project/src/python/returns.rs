//! The return decides the engine (MG-REQ-0613).

use mergus_ports::{PythonFunction, PythonReturn, PythonReturned, PythonSegment, PythonTree};

use super::{StepEngine, to_usize};
use crate::PythonModelError;

/// The fix every invalid return shares.
const FIX: &str = "return SQL as one string literal, or a Polars LazyFrame";

/// The kind of one valid return.
#[derive(PartialEq, Eq)]
enum Kind {
    /// A string literal: SQL.
    Sql,
    /// Any other expression: Polars.
    Polars,
}

/// Returns the engine of the step `function` of `tree`, or `None` after adding to `problems`
/// each invalid return, a missing return or returns of both kinds (MG-REQ-0613.2).
pub(super) fn engine(
    path: &str,
    function: &PythonFunction,
    tree: &PythonTree,
    problems: &mut Vec<PythonModelError>,
) -> Option<StepEngine> {
    let problem = |line: u32, problem: String| PythonModelError::InvalidReturn {
        path: path.to_owned(),
        step: function.name.clone(),
        line: to_usize(line),
        problem,
    };
    if function.returns.is_empty() {
        problems.push(problem(function.line, format!("has no `return`; {FIX}")));
        return None;
    }
    let mut kinds = Vec::new();
    let before = problems.len();
    for each in &function.returns {
        match kind(each, tree) {
            Ok(kind) => kinds.push((kind, each)),
            Err(what) => problems.push(problem(each.line, format!("returns {what}; {FIX}"))),
        }
    }
    if problems.len() > before {
        return None;
    }
    let (first, _) = kinds.first()?;
    if let Some((_, other)) = kinds.iter().find(|(kind, _)| kind != first) {
        let what = "returns both SQL and a Polars expression; every return of a step has one kind";
        problems.push(problem(other.line, what.to_owned()));
        return None;
    }
    Some(match first {
        Kind::Sql => StepEngine::DuckDb {
            sql: function
                .returns
                .iter()
                .filter_map(|each| match &each.value {
                    PythonReturned::String { value } => Some(value.clone()),
                    _ => None,
                })
                .collect(),
        },
        Kind::Polars => StepEngine::Polars,
    })
}

/// The kind of `returned`, or what makes it invalid (MG-REQ-0613.1, MG-REQ-0613.2).
fn kind(returned: &PythonReturn, tree: &PythonTree) -> Result<Kind, &'static str> {
    match &returned.value {
        PythonReturned::String { .. } => Ok(Kind::Sql),
        PythonReturned::None => Err("`None`"),
        PythonReturned::Expression { node } => match node.as_str() {
            "JoinedStr" => Err("an f-string"),
            "BinOp" => Err("a binary operation"),
            "Name" => Err("a bare variable"),
            "Call" if formats_a_literal(returned.line, tree) => {
                Err("`.format(...)` of a string literal")
            }
            _ => Ok(Kind::Polars),
        },
    }
}

/// Whether a call named `format` whose receiver is a literal is on `line`.
fn formats_a_literal(line: u32, tree: &PythonTree) -> bool {
    tree.calls.iter().any(|call| {
        call.line == line
            && call.name.as_deref() == Some("format")
            && matches!(
                call.receiver.as_slice(),
                [PythonSegment::Other { node }] if node == "Constant"
            )
    })
}
