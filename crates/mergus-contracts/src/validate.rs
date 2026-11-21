//! Validation of a JSON value against a [`Schema`], following JSON Schema 2020-12 for the
//! supported keywords.

use std::fmt;

use serde_json::Value;

use crate::schema::{Kind, Node, Schema};

/// One way a document breaks its schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The JSON pointer of the offending value, empty for the document itself.
    pub pointer: String,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.pointer, self.message)
    }
}

impl Schema {
    /// Returns every violation of `value` against this schema, in document order; none when
    /// it is valid.
    pub fn validate(&self, value: &Value) -> Vec<Violation> {
        let mut violations = Vec::new();
        self.check(&self.root, value, "", &mut violations);
        violations
    }

    fn check(&self, node: &Node, value: &Value, pointer: &str, out: &mut Vec<Violation>) {
        let mut report = |message: String| {
            out.push(Violation {
                pointer: pointer.to_owned(),
                message,
            });
        };
        if let Some(kind) = node.kind
            && !has_kind(value, kind)
        {
            report(format!("the value is not of type `{}`", kind_name(kind)));
        }
        if let Some(constant) = &node.constant
            && !same(value, constant)
        {
            report(format!("the value is not the constant {constant}"));
        }
        if let Some(choices) = &node.choices
            && !choices.iter().any(|choice| same(value, choice))
        {
            report("the value is not one of the values of `enum`".to_owned());
        }
        if let Some(name) = &node.reference
            && let Some(def) = self.defs.get(name)
        {
            self.check(def, value, pointer, out);
        }
        if let Value::Object(members) = value {
            self.check_object(node, members, pointer, out);
        }
        if let (Value::Array(items), Some(schema)) = (value, &node.items) {
            for (index, item) in items.iter().enumerate() {
                self.check(schema, item, &format!("{pointer}/{index}"), out);
            }
        }
    }

    fn check_object(
        &self,
        node: &Node,
        members: &serde_json::Map<String, Value>,
        pointer: &str,
        out: &mut Vec<Violation>,
    ) {
        for name in &node.required {
            if !members.contains_key(name) {
                out.push(Violation {
                    pointer: pointer.to_owned(),
                    message: format!("the required property `{name}` is missing"),
                });
            }
        }
        for (name, member) in members {
            let at = format!("{pointer}/{}", escape(name));
            match node.properties.get(name) {
                Some(schema) => self.check(schema, member, &at, out),
                None if node.closed => out.push(Violation {
                    pointer: at,
                    message: "the property is not allowed".to_owned(),
                }),
                None => {}
            }
        }
    }
}

/// Escapes one reference token of a JSON pointer (RFC 6901).
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn has_kind(value: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Null => value.is_null(),
        Kind::Boolean => value.is_boolean(),
        Kind::Object => value.is_object(),
        Kind::Array => value.is_array(),
        Kind::Number => value.is_number(),
        Kind::Integer => is_integer(value),
        Kind::String => value.is_string(),
    }
}

/// An integer is a number with no fractional part, such as `1` or `1.0`.
fn is_integer(value: &Value) -> bool {
    match value {
        Value::Number(number) => {
            number.is_i64() || number.is_u64() || number.as_f64().is_some_and(|n| n.fract() == 0.0)
        }
        _ => false,
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Null => "null",
        Kind::Boolean => "boolean",
        Kind::Object => "object",
        Kind::Array => "array",
        Kind::Number => "number",
        Kind::Integer => "integer",
        Kind::String => "string",
    }
}

/// JSON equality: numbers are equal when their values are, whatever their notation.
fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => match (left.as_i64(), right.as_i64()) {
            (Some(left), Some(right)) => left == right,
            _ => match (left.as_u64(), right.as_u64()) {
                (Some(left), Some(right)) => left == right,
                _ => left.as_f64() == right.as_f64(),
            },
        },
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(l, r)| same(l, r))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, l)| right.get(key).is_some_and(|r| same(l, r)))
        }
        _ => left == right,
    }
}
