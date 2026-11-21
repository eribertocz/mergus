//! A JSON Schema 2020-12 document, read into the keywords this crate supports.
//!
//! A schema that uses any other keyword is refused when it is parsed, so no keyword is ever
//! silently ignored during validation.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use thiserror::Error;

/// The keywords a schema may use. The first four are annotations: they do not change what
/// validates.
pub const KEYWORDS: [&str; 13] = [
    "$schema",
    "$id",
    "title",
    "description",
    "$defs",
    "$ref",
    "type",
    "const",
    "enum",
    "properties",
    "required",
    "additionalProperties",
    "items",
];

/// The prefix of every `$ref`: only definitions of the same document are referenced.
const DEFS: &str = "#/$defs/";

/// Why a schema cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("the schema is not supported: {pointer}: {message}")]
pub struct SchemaError {
    /// The JSON pointer of the offending schema node, empty for the root.
    pub pointer: String,
    /// What is wrong with it.
    pub message: String,
}

/// The type a `type` keyword names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Null,
    Boolean,
    Object,
    Array,
    Number,
    Integer,
    String,
}

/// One schema node, reduced to the supported keywords.
#[derive(Debug, Default)]
pub(crate) struct Node {
    pub(crate) kind: Option<Kind>,
    pub(crate) constant: Option<Value>,
    pub(crate) choices: Option<Vec<Value>>,
    pub(crate) properties: BTreeMap<String, Node>,
    pub(crate) required: Vec<String>,
    pub(crate) closed: bool,
    pub(crate) items: Option<Box<Node>>,
    pub(crate) reference: Option<String>,
}

/// A parsed schema: its root node and its definitions.
#[derive(Debug)]
pub struct Schema {
    pub(crate) root: Node,
    pub(crate) defs: BTreeMap<String, Node>,
}

impl Schema {
    /// Parses the text of a schema.
    ///
    /// # Errors
    ///
    /// Fails if `text` is not JSON, or if the schema uses a keyword outside [`KEYWORDS`],
    /// a `$ref` that names no definition of the same document, or a keyword with a value of
    /// the wrong shape.
    pub fn parse(text: &str) -> Result<Self, SchemaError> {
        let document: Value = serde_json::from_str(text).map_err(|error| SchemaError {
            pointer: String::new(),
            message: format!("not JSON: {error}"),
        })?;
        let members = object(&document, "")?;
        let mut defs = BTreeMap::new();
        if let Some(value) = members.get("$defs") {
            for (name, child) in object(value, "/$defs")? {
                defs.insert(name.clone(), node(child, &format!("/$defs/{name}"), false)?);
            }
        }
        let schema = Self {
            root: node(&document, "", true)?,
            defs,
        };
        schema.check_references(&schema.root, "")?;
        for (name, def) in &schema.defs {
            schema.check_references(def, &format!("/$defs/{name}"))?;
        }
        Ok(schema)
    }

    fn check_references(&self, node: &Node, pointer: &str) -> Result<(), SchemaError> {
        if let Some(name) = &node.reference
            && !self.defs.contains_key(name)
        {
            return Err(error(
                pointer,
                format!("`$ref` names no definition `{name}`"),
            ));
        }
        for (name, child) in &node.properties {
            self.check_references(child, &format!("{pointer}/properties/{name}"))?;
        }
        if let Some(items) = &node.items {
            self.check_references(items, &format!("{pointer}/items"))?;
        }
        Ok(())
    }
}

fn error(pointer: &str, message: impl Into<String>) -> SchemaError {
    SchemaError {
        pointer: pointer.to_owned(),
        message: message.into(),
    }
}

fn object<'a>(value: &'a Value, pointer: &str) -> Result<&'a Map<String, Value>, SchemaError> {
    value
        .as_object()
        .ok_or_else(|| error(pointer, "expected an object"))
}

/// Reads one schema node. `root` allows the keywords that only the root may carry.
fn node(value: &Value, pointer: &str, root: bool) -> Result<Node, SchemaError> {
    let members = object(value, pointer)?;
    let mut node = Node::default();
    for (keyword, value) in members {
        let at = format!("{pointer}/{keyword}");
        match keyword.as_str() {
            "$schema" | "$id" | "$defs" if root => {}
            "title" | "description" => {
                value
                    .as_str()
                    .ok_or_else(|| error(&at, "expected a string"))?;
            }
            "$ref" => node.reference = Some(reference(value, &at)?),
            "type" => node.kind = Some(kind(value, &at)?),
            "const" => node.constant = Some(value.clone()),
            "enum" => node.choices = Some(choices(value, &at)?),
            "properties" => {
                for (name, child) in object(value, &at)? {
                    let child = node_at(child, &format!("{at}/{name}"))?;
                    node.properties.insert(name.clone(), child);
                }
            }
            "required" => node.required = names(value, &at)?,
            "additionalProperties" => node.closed = closed(value, &at)?,
            "items" => node.items = Some(Box::new(node_at(value, &at)?)),
            _ => return Err(error(pointer, format!("unsupported keyword `{keyword}`"))),
        }
    }
    Ok(node)
}

fn node_at(value: &Value, pointer: &str) -> Result<Node, SchemaError> {
    node(value, pointer, false)
}

fn reference(value: &Value, pointer: &str) -> Result<String, SchemaError> {
    value
        .as_str()
        .and_then(|text| text.strip_prefix(DEFS))
        .map(str::to_owned)
        .ok_or_else(|| error(pointer, format!("`$ref` must start with `{DEFS}`")))
}

fn kind(value: &Value, pointer: &str) -> Result<Kind, SchemaError> {
    match value.as_str() {
        Some("null") => Ok(Kind::Null),
        Some("boolean") => Ok(Kind::Boolean),
        Some("object") => Ok(Kind::Object),
        Some("array") => Ok(Kind::Array),
        Some("number") => Ok(Kind::Number),
        Some("integer") => Ok(Kind::Integer),
        Some("string") => Ok(Kind::String),
        _ => Err(error(pointer, "`type` must be one type name")),
    }
}

fn choices(value: &Value, pointer: &str) -> Result<Vec<Value>, SchemaError> {
    match value.as_array() {
        Some(items) if !items.is_empty() => Ok(items.clone()),
        _ => Err(error(pointer, "`enum` must be a non-empty array")),
    }
}

fn names(value: &Value, pointer: &str) -> Result<Vec<String>, SchemaError> {
    value
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .ok_or_else(|| error(pointer, "`required` must be an array of strings"))
}

/// Only closed objects are supported: `additionalProperties` must be `false`.
fn closed(value: &Value, pointer: &str) -> Result<bool, SchemaError> {
    match value {
        Value::Bool(false) => Ok(true),
        _ => Err(error(pointer, "`additionalProperties` must be `false`")),
    }
}
