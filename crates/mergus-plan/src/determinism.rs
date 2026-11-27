//! The non-determinism detector (MG-REQ-0637): operations that can return different results
//! on the same data, found on the tree of the `SqlParser` port, and the `Determinism` gate.

use std::fmt;

use serde_json::{Map, Value};

use crate::{Gate, GateError, PlanModel};

/// The code of an undeclared non-deterministic operation of a published model
/// (MG-REQ-0637.4).
const UNDECLARED: &str = "MG-0703";

/// The option that declares a model's non-deterministic operations (MG-REQ-0637).
const DECLARED: &str = "nondeterministic";

/// An operation that can return different results on the same data (MG-REQ-0637.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    /// A `LIMIT` in a `SELECT` with no `ORDER BY` at the same level.
    LimitWithoutOrder,
    /// A call of `any_value`.
    AnyValue,
    /// A call of `random`.
    Random,
    /// A call of `now`.
    Now,
    /// A Polars `.unique(` call without `keep`.
    UniqueWithoutKeep,
    /// A Polars `.first(` call with no `.sort(` earlier in the same chain.
    FirstWithoutSort,
}

impl Operation {
    /// The name of the operation, as `nondeterministic = [...]` declares it (MG-REQ-0637.1).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::LimitWithoutOrder => "limit_without_order",
            Self::AnyValue => "any_value",
            Self::Random => "random",
            Self::Now => "now",
            Self::UniqueWithoutKeep => "unique_without_keep",
            Self::FirstWithoutSort => "first_without_sort",
        }
    }
}

/// Shows the name of the operation.
impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// A non-deterministic operation found in the code of a step, and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Detected {
    /// Its line in the code, from 1.
    pub line: usize,
    /// Its column in the code, from 1, counted in Unicode scalar values.
    pub column: usize,
    /// The operation.
    pub operation: Operation,
}

/// The SQL functions that are non-deterministic, matched without case (MG-REQ-0637.2).
const FUNCTIONS: [(&str, Operation); 3] = [
    ("any_value", Operation::AnyValue),
    ("random", Operation::Random),
    ("now", Operation::Now),
];

/// Returns the non-deterministic operations of the SQL `text`, found on `tree`, its tree from
/// the `SqlParser` port (`json_serialize_sql`), never on the text (MG-REQ-0637.2): a
/// `LIMIT` modifier with no `ORDER BY` modifier at the same level, and each call of
/// `any_value`, `random` or `now`. They are sorted by line, column and operation.
#[must_use]
pub fn detect_sql(text: &str, tree: &Value) -> Vec<Detected> {
    let mut found = Vec::new();
    walk(text, tree, &mut found);
    found.sort();
    found.dedup();
    found
}

/// Adds to `found` the operations of `value` and of the nodes under it.
fn walk(text: &str, value: &Value, found: &mut Vec<Detected>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| walk(text, item, found)),
        Value::Object(node) => {
            if let Some(at) = limit_without_order(node) {
                found.push(detected(text, Operation::LimitWithoutOrder, at));
            }
            if node.get("class").and_then(Value::as_str) == Some("FUNCTION") {
                let name = node
                    .get("function_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                if let Some((_, operation)) = FUNCTIONS.iter().find(|(known, _)| *known == name) {
                    found.push(detected(text, *operation, location(node)));
                }
            }
            node.values().for_each(|child| walk(text, child, found));
        }
        _ => {}
    }
}

/// The location of the `LIMIT` of `node` when its modifiers hold a `LIMIT` and no `ORDER BY`
/// with an order; `None` otherwise. The limit's own location, or the node's, or the start.
fn limit_without_order(node: &Map<String, Value>) -> Option<Option<usize>> {
    let modifiers = node.get("modifiers").and_then(Value::as_array)?;
    let limit = modifiers
        .iter()
        .find(|modifier| kind(modifier) == "LIMIT_MODIFIER")?;
    let ordered = modifiers.iter().any(|modifier| {
        kind(modifier) == "ORDER_MODIFIER"
            && modifier
                .get("orders")
                .and_then(Value::as_array)
                .is_some_and(|orders| !orders.is_empty())
    });
    if ordered {
        return None;
    }
    let at = limit
        .as_object()
        .and_then(|limit| {
            ["limit", "offset"]
                .iter()
                .filter_map(|key| limit.get(*key).and_then(Value::as_object))
                .find_map(location)
        })
        .or_else(|| location(node));
    Some(at)
}

/// The `type` of the modifier `modifier`, empty when it has none.
fn kind(modifier: &Value) -> &str {
    modifier.get("type").and_then(Value::as_str).unwrap_or("")
}

/// The byte offset of `node` in the text, when the tree gives one.
fn location(node: &Map<String, Value>) -> Option<usize> {
    let offset = node.get("query_location").and_then(Value::as_u64)?;
    usize::try_from(offset).ok()
}

/// `operation` at the byte offset `at` of `text`, or at its start when `at` is not in it.
fn detected(text: &str, operation: Operation, at: Option<usize>) -> Detected {
    let before = at.and_then(|at| text.get(..at)).unwrap_or("");
    let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
    Detected {
        operation,
        line: before.matches('\n').count() + 1,
        column: before.get(line_start..).unwrap_or("").chars().count() + 1,
    }
}

/// The errors of the `Determinism` gate (MG-REQ-0637.4): one `MG-0703` per operation found in
/// a step of a published model that the model does not declare in `nondeterministic`, in the
/// order of `models`, then of their steps and operations. A model that is not published has
/// none.
#[must_use]
pub fn determinism(models: &[PlanModel]) -> Vec<GateError> {
    let mut errors = Vec::new();
    for model in models.iter().filter(|model| model.published) {
        let declared: Vec<&str> = model
            .config
            .get(DECLARED)
            .and_then(Value::as_array)
            .map(|names| names.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for step in &model.steps {
            for found in &step.detected {
                let name = found.operation.name();
                if !declared.contains(&name) {
                    errors.push(GateError {
                        gate: Gate::Determinism,
                        code: UNDECLARED,
                        model: Some(model.name.clone()),
                        step: Some(step.name.clone()),
                        message: format!(
                            "`{name}` at line {}, column {} can return different results on \
                             the same data",
                            found.line, found.column
                        ),
                        fix: format!(
                            "make it deterministic, or declare it with \
                             `nondeterministic = [\"{name}\"]`"
                        ),
                    });
                }
            }
        }
    }
    errors
}
