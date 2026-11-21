//! Reads a contract schema into the Rust items generated from it (MG-REQ-0202).
//!
//! The generator accepts the keywords that `mergus-contracts` validates and nothing else, so
//! every generated type has a schema the validator can check.

use crate::json::Json;

/// The keywords any schema node may carry.
const KEYWORDS: [&str; 10] = [
    "title",
    "description",
    "$ref",
    "type",
    "const",
    "enum",
    "properties",
    "required",
    "additionalProperties",
    "items",
];

/// The keywords only the root node may carry.
const ROOT_KEYWORDS: [&str; 3] = ["$schema", "$id", "$defs"];

/// The prefix of every `$ref`.
const DEFS: &str = "#/$defs/";

/// The Rust type of a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Type {
    String,
    Integer,
    Number,
    Boolean,
    List(Box<Type>),
    Named(String),
}

/// One field of a generated struct.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Field {
    /// The property name, which is also the field name.
    pub(crate) name: String,
    pub(crate) doc: String,
    pub(crate) ty: Type,
    pub(crate) required: bool,
}

/// One generated item.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Item {
    /// A struct, from an object schema.
    Struct {
        name: String,
        doc: String,
        fields: Vec<Field>,
    },
    /// A unit-only enum, from an `enum` of strings: each value and its variant name.
    Enum {
        name: String,
        doc: String,
        values: Vec<(String, String)>,
    },
}

impl Item {
    fn name(&self) -> &str {
        match self {
            Self::Struct { name, .. } | Self::Enum { name, .. } => name,
        }
    }
}

/// Returns the items of the contract `contract`: the root struct, named after the contract,
/// then the items it needs, then those of `$defs`, each in document order.
///
/// # Errors
///
/// Fails with a message that starts with the JSON pointer of the offending node.
pub(crate) fn items(contract: &str, schema: &Json) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let root = pascal(contract).ok_or(": the contract name is not snake_case")?;
    object(&root, schema, "", &mut items)?;
    if let Some(defs) = schema.get("$defs") {
        for (name, def) in defs.members() {
            let pointer = format!("/$defs/{name}");
            let rust = pascal(name).ok_or_else(|| format!("{pointer}: name is not snake_case"))?;
            if !matches!(type_of(&rust, def, &pointer, &mut items)?, Type::Named(_)) {
                return Err(format!(
                    "{pointer}: a definition must be an object or an enum"
                ));
            }
        }
    }
    let mut names: Vec<&str> = items.iter().map(Item::name).collect();
    names.sort_unstable();
    if let Some(pair) = names.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(format!(": two generated types are named `{}`", pair[0]));
    }
    Ok(items)
}

/// Checks the keywords and the description of a node, and returns the description.
fn description(node: &Json, pointer: &str) -> Result<String, String> {
    let Json::Object(members) = node else {
        return Err(format!("{pointer}: a schema must be an object"));
    };
    for (keyword, _) in members {
        let root = pointer.is_empty() && ROOT_KEYWORDS.contains(&keyword.as_str());
        if !root && !KEYWORDS.contains(&keyword.as_str()) {
            return Err(format!("{pointer}: unsupported keyword `{keyword}`"));
        }
    }
    node.get("description")
        .and_then(Json::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{pointer}: `description` is missing"))
}

/// Returns the type of a node, pushing the items it needs. `name` names the item the node
/// generates, if it generates one.
fn type_of(name: &str, node: &Json, pointer: &str, items: &mut Vec<Item>) -> Result<Type, String> {
    let doc = description(node, pointer)?;
    if let Some(reference) = node.get("$ref") {
        return reference
            .as_str()
            .and_then(|text| text.strip_prefix(DEFS))
            .and_then(pascal)
            .map(Type::Named)
            .ok_or_else(|| format!("{pointer}: `$ref` must be `{DEFS}<snake_case name>`"));
    }
    let kind = node.get("type").and_then(Json::as_str);
    if kind == Some("object") || node.get("properties").is_some() {
        return object(name, node, pointer, items);
    }
    if let Some(choices) = node.get("enum") {
        if kind.is_some_and(|kind| kind != "string") {
            return Err(format!("{pointer}: `enum` must have `type` string"));
        }
        let values = variants(choices, pointer)?;
        items.push(Item::Enum {
            name: name.to_owned(),
            doc,
            values,
        });
        return Ok(Type::Named(name.to_owned()));
    }
    scalar(name, node, kind, pointer, items)
}

/// Returns each value of an `enum` and its variant name.
fn variants(choices: &Json, pointer: &str) -> Result<Vec<(String, String)>, String> {
    strings(choices)
        .ok_or_else(|| format!("{pointer}: `enum` must list strings"))?
        .into_iter()
        .map(|value| pascal(&value).map(|variant| (value, variant)))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("{pointer}: every value of `enum` must be snake_case"))
}

/// Returns the type of a node that is neither an object, an enum nor a reference: from its
/// `type`, or from the value of its `const`.
fn scalar(
    name: &str,
    node: &Json,
    kind: Option<&str>,
    pointer: &str,
    items: &mut Vec<Item>,
) -> Result<Type, String> {
    let kind = match (kind, node.get("const")) {
        (Some(kind), _) => kind,
        (None, Some(Json::String(_))) => "string",
        (None, Some(Json::Bool(_))) => "boolean",
        (None, Some(Json::Number(text))) if !text.contains(['.', 'e', 'E']) => "integer",
        (None, Some(Json::Number(_))) => "number",
        _ => return Err(format!("{pointer}: no Rust type for this schema")),
    };
    match kind {
        "string" => Ok(Type::String),
        "integer" => Ok(Type::Integer),
        "number" => Ok(Type::Number),
        "boolean" => Ok(Type::Boolean),
        "array" => {
            let items_node = node
                .get("items")
                .ok_or_else(|| format!("{pointer}: an array needs `items`"))?;
            let at = format!("{pointer}/items");
            let item = type_of(&format!("{name}Item"), items_node, &at, items)?;
            Ok(Type::List(Box::new(item)))
        }
        _ => Err(format!("{pointer}: unsupported type `{kind}`")),
    }
}

/// Pushes the struct of a closed object schema and returns its type.
fn object(name: &str, node: &Json, pointer: &str, items: &mut Vec<Item>) -> Result<Type, String> {
    let doc = description(node, pointer)?;
    if !matches!(node.get("additionalProperties"), Some(Json::Bool(false))) {
        return Err(format!(
            "{pointer}: objects must set `additionalProperties: false`"
        ));
    }
    let required = match node.get("required") {
        Some(list) => strings(list).ok_or_else(|| format!("{pointer}: bad `required`"))?,
        None => Vec::new(),
    };
    let index = items.len();
    let mut fields = Vec::new();
    for (property, child) in node
        .get("properties")
        .map(Json::members)
        .unwrap_or_default()
    {
        let at = format!("{pointer}/properties/{property}");
        let field_type = pascal(property)
            .filter(|_| !property.contains('-'))
            .ok_or_else(|| format!("{at}: name is not snake_case"))?;
        fields.push(Field {
            name: property.clone(),
            doc: description(child, &at)?,
            ty: type_of(&format!("{name}{field_type}"), child, &at, items)?,
            required: required.contains(property),
        });
    }
    if let Some(missing) = required
        .iter()
        .find(|name| !fields.iter().any(|f| &f.name == *name))
    {
        return Err(format!(
            "{pointer}: `required` names no property `{missing}`"
        ));
    }
    let item = Item::Struct {
        name: name.to_owned(),
        doc,
        fields,
    };
    items.insert(index, item);
    Ok(Type::Named(name.to_owned()))
}

fn strings(list: &Json) -> Option<Vec<String>> {
    let Json::Array(values) = list else {
        return None;
    };
    values
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

/// Converts a snake_case name, such as `catalog_schema` or `view-2`, to PascalCase, such as
/// `CatalogSchema` or `View2`. Returns `None` for any other name.
pub(crate) fn pascal(name: &str) -> Option<String> {
    let valid = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !valid {
        return None;
    }
    let mut out = String::new();
    for word in name.split(['_', '-']).filter(|word| !word.is_empty()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    Some(out)
}
