//! Whether a new text of a schema keeps the major version of its contract (MG-REQ-0204).
//!
//! A change to a `title` or a `description` only is a patch change; so is a change in the
//! order of members or `enum` values, which changes no document. Adding an optional property
//! or a definition is a minor change. Every other change is major: removing or renaming a
//! property, changing a `type`, a `const`, an `enum`, a `$ref` or any other keyword, and
//! changing which properties are required. A loosening change, such as an added `enum` value
//! or a property that is no longer required, is major too: it changes the generated Rust
//! type that code matches on (MG-REQ-0202).

use super::version::Bump;
use crate::json::Json;

/// Keywords that do not change which documents a schema accepts.
const ANNOTATIONS: [&str; 2] = ["title", "description"];

/// Keywords compared member by member instead of as a whole.
const STRUCTURE: [&str; 4] = ["properties", "required", "items", "$defs"];

/// What a comparison of two schema texts finds.
#[derive(Default)]
struct Found {
    /// Every change that needs a new major version, with the JSON pointer of its node.
    breaking: Vec<String>,
    /// Whether an optional property or a definition is added.
    added: bool,
}

impl Found {
    fn push(&mut self, problem: String) {
        self.breaking.push(problem);
    }
}

/// Classifies the change from `old` to `new` (MG-REQ-0204): the part of the version it
/// raises, or every change that needs a new major version.
///
/// # Errors
///
/// Returns every change that needs a new major version, each with the JSON pointer of its
/// node.
pub(crate) fn classify(old: &Json, new: &Json) -> Result<Bump, Vec<String>> {
    let found = compare(old, new);
    if !found.breaking.is_empty() {
        Err(found.breaking)
    } else if found.added {
        Ok(Bump::Minor)
    } else if old == new {
        Ok(Bump::None)
    } else {
        Ok(Bump::Patch)
    }
}

fn compare(old: &Json, new: &Json) -> Found {
    let mut found = Found::default();
    node(old, new, "", &mut found);
    let old_defs = old.get("$defs");
    let new_defs = new.get("$defs");
    for (name, def) in old_defs.map(Json::members).unwrap_or_default() {
        let pointer = format!("/$defs/{name}");
        match new_defs.and_then(|defs| defs.get(name)) {
            Some(new_def) => node(def, new_def, &pointer, &mut found),
            None => found.push(format!("`{pointer}` is removed")),
        }
    }
    for (name, _) in new_defs.map(Json::members).unwrap_or_default() {
        if old_defs.and_then(|defs| defs.get(name)).is_none() {
            found.added = true;
        }
    }
    found
}

/// Compares one schema node: its keywords, its properties, then its `items`.
fn node(old: &Json, new: &Json, pointer: &str, found: &mut Found) {
    for keyword in keywords(old, new) {
        if !same(keyword, old.get(keyword), new.get(keyword)) {
            found.push(format!("{} changes `{keyword}`", shown(pointer)));
        }
    }
    properties(old, new, pointer, found);
    match (old.get("items"), new.get("items")) {
        (Some(old_items), Some(new_items)) => {
            node(old_items, new_items, &format!("{pointer}/items"), found);
        }
        (None, None) => {}
        _ => found.push(format!("{} changes `items`", shown(pointer))),
    }
}

/// Compares the properties of two object nodes and which of them are required.
fn properties(old: &Json, new: &Json, pointer: &str, found: &mut Found) {
    let old_required = required(old);
    let new_required = required(new);
    let new_properties = new.get("properties");
    for (name, old_child) in old.get("properties").map(Json::members).unwrap_or_default() {
        let at = format!("{pointer}/properties/{name}");
        let Some(new_child) = new_properties.and_then(|properties| properties.get(name)) else {
            found.push(format!("`{at}` is removed"));
            continue;
        };
        match (
            old_required.contains(&name.as_str()),
            new_required.contains(&name.as_str()),
        ) {
            (true, false) => found.push(format!("`{at}` is no longer required")),
            (false, true) => found.push(format!("`{at}` becomes required")),
            _ => {}
        }
        node(old_child, new_child, &at, found);
    }
    for (name, _) in new_properties.map(Json::members).unwrap_or_default() {
        let added = old.get("properties").and_then(|p| p.get(name)).is_none();
        if added && new_required.contains(&name.as_str()) {
            found.push(format!(
                "`{pointer}/properties/{name}` is added as a required property"
            ));
        } else if added {
            found.added = true;
        }
    }
}

/// The keywords of either node that are compared as a whole, in the order they appear.
fn keywords<'a>(old: &'a Json, new: &'a Json) -> Vec<&'a str> {
    let mut keywords: Vec<&str> = Vec::new();
    for (keyword, _) in old.members().iter().chain(new.members()) {
        let compared =
            !ANNOTATIONS.contains(&keyword.as_str()) && !STRUCTURE.contains(&keyword.as_str());
        if compared && !keywords.contains(&keyword.as_str()) {
            keywords.push(keyword);
        }
    }
    keywords
}

/// Whether a keyword has the same value in both nodes. The values of an `enum` are compared
/// as a set.
fn same(keyword: &str, old: Option<&Json>, new: Option<&Json>) -> bool {
    match (old, new) {
        (Some(old), Some(new)) if keyword == "enum" => sorted(old) == sorted(new),
        _ => old == new,
    }
}

fn sorted(list: &Json) -> Vec<String> {
    let mut values: Vec<String> = list
        .items()
        .iter()
        .map(|value| format!("{value:?}"))
        .collect();
    values.sort_unstable();
    values
}

/// The names a node lists in `required`.
fn required(node: &Json) -> Vec<&str> {
    node.get("required")
        .map(Json::items)
        .unwrap_or_default()
        .iter()
        .filter_map(Json::as_str)
        .collect()
}

/// A JSON pointer as findings show it.
fn shown(pointer: &str) -> String {
    if pointer.is_empty() {
        "the root".to_owned()
    } else {
        format!("`{pointer}`")
    }
}
