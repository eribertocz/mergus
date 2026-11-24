//! Selections (MG-REQ-1305): which models a command works on, and why each one is selected.
//!
//! The words of a selection form a union; the selectors of one word, joined by commas, an
//! intersection; `--exclude` removes what its own selection selects.

use std::collections::{BTreeMap, BTreeSet};

use mergus_project::closest_name;

use crate::error::CliError;
use crate::models::Project;

/// The selected models, by name, each with the reasons it is selected, sorted.
pub(crate) type Selected = BTreeMap<String, BTreeSet<String>>;

/// What one selector matches, before `+`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Method {
    Model(String),
    Tag(String),
    Source(String),
    Path(String),
    Changed,
}

/// One selector: a method, with or without its upstream and downstream models.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Selector {
    upstream: bool,
    method: Method,
    downstream: bool,
}

/// Selects the models of `project` that `words` select, without those `exclude` selects.
/// No word selects every model.
///
/// # Errors
///
/// Fails with [`CliError::InvalidSelector`] for a word that is not a selector, and with
/// [`CliError::UnknownName`] for a model or a source the project does not have.
pub(crate) fn select(
    project: &Project,
    words: &[&str],
    exclude: Option<&str>,
) -> Result<Selected, CliError> {
    let words = split(words);
    let mut selected = if words.is_empty() {
        project
            .models
            .iter()
            .map(|model| {
                let reason = BTreeSet::from(["no selection: every model".to_owned()]);
                (model.name().to_owned(), reason)
            })
            .collect()
    } else {
        union(project, &words)?
    };
    if let Some(exclude) = exclude {
        for name in union(project, &split(&[exclude]))?.keys() {
            selected.remove(name);
        }
    }
    Ok(selected)
}

/// The text `mergus ls` prints: one line per selected model, `<model>: <reasons>`.
pub(crate) fn listing(selected: &Selected) -> String {
    selected
        .iter()
        .map(|(name, reasons)| {
            let reasons: Vec<&str> = reasons.iter().map(String::as_str).collect();
            format!("{name}: {}\n", reasons.join("; "))
        })
        .collect()
}

/// Every word of `words`, a quoted word split at its spaces.
fn split<'a>(words: &[&'a str]) -> Vec<&'a str> {
    words
        .iter()
        .flat_map(|word| word.split_whitespace())
        .collect()
}

/// The union of what each word selects.
fn union(project: &Project, words: &[&str]) -> Result<Selected, CliError> {
    let mut selected = Selected::new();
    for word in words {
        for (name, reasons) in intersection(project, word)? {
            selected.entry(name).or_default().extend(reasons);
        }
    }
    Ok(selected)
}

/// The intersection of the selectors of `word`, joined by commas.
fn intersection(project: &Project, word: &str) -> Result<Selected, CliError> {
    let mut parts = word.split(',');
    let first = parts.next().unwrap_or_default();
    let mut selected = matches(project, &parse(word, first)?)?;
    for part in parts {
        let other = matches(project, &parse(word, part)?)?;
        selected.retain(|name, _| other.contains_key(name));
        for (name, reasons) in &mut selected {
            reasons.extend(other.get(name).into_iter().flatten().cloned());
        }
    }
    Ok(selected)
}

/// Parses one selector of `word`.
fn parse(word: &str, text: &str) -> Result<Selector, CliError> {
    let invalid = |reason| CliError::InvalidSelector {
        given: word.to_owned(),
        reason,
    };
    let (upstream, rest) = match text.strip_prefix('+') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (downstream, core) = match rest.strip_suffix('+') {
        Some(core) => (true, core),
        None => (false, rest),
    };
    if core.is_empty() {
        return Err(invalid("it names no model"));
    }
    if core.contains('+') {
        return Err(invalid("`+` comes only first or last"));
    }
    let method = match core.split_once(':') {
        None if core == "changed" => Method::Changed,
        None => Method::Model(core.to_owned()),
        Some((_, "")) => return Err(invalid("the method has no value")),
        Some(("tag", value)) => Method::Tag(value.to_owned()),
        Some(("source", value)) => Method::Source(value.to_owned()),
        Some(("path", value)) => Method::Path(value.trim_end_matches('/').to_owned()),
        Some(_) => return Err(invalid("the methods are `tag:`, `source:` and `path:`")),
    };
    Ok(Selector {
        upstream,
        method,
        downstream,
    })
}

/// The models `selector` selects, with their reasons.
fn matches(project: &Project, selector: &Selector) -> Result<Selected, CliError> {
    let base = base(project, &selector.method)?;
    let mut selected: Selected = base
        .iter()
        .map(|(name, reason)| (name.clone(), BTreeSet::from([reason.clone()])))
        .collect();
    for name in base.keys() {
        if selector.upstream {
            for found in walk(project, name, Direction::Up) {
                let reason = format!("upstream of `{name}`");
                selected.entry(found).or_default().insert(reason);
            }
        }
        if selector.downstream {
            for found in walk(project, name, Direction::Down) {
                let reason = format!("downstream of `{name}`");
                selected.entry(found).or_default().insert(reason);
            }
        }
    }
    Ok(selected)
}

/// The models `method` matches, each with its reason.
fn base(project: &Project, method: &Method) -> Result<BTreeMap<String, String>, CliError> {
    let names = project.models.iter().map(|model| model.name());
    let found: BTreeMap<String, String> = match method {
        Method::Model(name) => {
            if project.model(name).is_none() {
                return Err(unknown("model", name, names));
            }
            BTreeMap::from([(name.clone(), "named".to_owned())])
        }
        Method::Tag(tag) => project
            .models
            .iter()
            .filter(|model| model.tags().contains(tag))
            .map(|model| (model.name().to_owned(), format!("has tag `{tag}`")))
            .collect(),
        Method::Source(source) => {
            if !project.sources.contains(source) {
                return Err(unknown(
                    "source",
                    source,
                    project.sources.iter().map(String::as_str),
                ));
            }
            let prefix = format!("raw.{source}__");
            project
                .models
                .iter()
                .filter(|model| model.inputs.iter().any(|input| input.starts_with(&prefix)))
                .map(|model| (model.name().to_owned(), format!("reads source `{source}`")))
                .collect()
        }
        Method::Path(path) => project
            .models
            .iter()
            .filter(|model| model.path() == path || model.path().starts_with(&format!("{path}/")))
            .map(|model| (model.name().to_owned(), format!("is under `{path}`")))
            .collect(),
        Method::Changed => project
            .models
            .iter()
            .map(|model| {
                let reason = "changed: no result of `prod` is available".to_owned();
                (model.name().to_owned(), reason)
            })
            .collect(),
    };
    Ok(found)
}

/// The error for a `kind` named `given` that is not among `names`.
fn unknown<'a>(kind: &'static str, given: &str, names: impl Iterator<Item = &'a str>) -> CliError {
    CliError::UnknownName {
        kind,
        given: given.to_owned(),
        closest: closest_name(given, names).map(str::to_owned),
    }
}

/// Which way [`walk`] follows the models.
#[derive(Clone, Copy)]
enum Direction {
    /// To the models a model reads.
    Up,
    /// To the models that read it.
    Down,
}

/// Every model reached from the model `start` going `direction`, without `start`.
fn walk(project: &Project, start: &str, direction: Direction) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![start.to_owned()];
    while let Some(name) = pending.pop() {
        let next: Vec<String> = match direction {
            Direction::Up => project
                .model(&name)
                .map(|model| {
                    model
                        .inputs
                        .iter()
                        .filter(|input| project.model(input).is_some())
                        .cloned()
                        .collect()
                })
                .unwrap_or_default(),
            Direction::Down => project
                .readers(&name)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        for model in next {
            if model != start && found.insert(model.clone()) {
                pending.push(model);
            }
        }
    }
    found
}
