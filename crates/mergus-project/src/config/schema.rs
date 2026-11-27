//! Validation of a converted configuration file against its JSON Schema (MG-REQ-0316).

use jsonschema::ValidationError;
use jsonschema::error::ValidationErrorKind;
use jsonschema::paths::LocationSegment;
use serde_json::Value;

use super::ConfigKind;
use super::convert::{Converted, Segment, dotted};
use crate::{LayoutError, ProjectError, closest_name};

/// Validates `file` against the schema of `kind`, pushing every finding to `problems`.
///
/// # Errors
///
/// Fails with [`LayoutError::Schema`] when the bundled schema cannot be compiled.
pub(crate) fn validate(
    kind: ConfigKind,
    path: &str,
    file: &Converted,
    problems: &mut Vec<ProjectError>,
) -> Result<(), LayoutError> {
    let schema: Value =
        serde_json::from_str(kind.schema()).map_err(|_| LayoutError::Schema(kind.schema_path()))?;
    let validator = jsonschema::draft202012::new(&schema)
        .map_err(|_| LayoutError::Schema(kind.schema_path()))?;
    let report = Report {
        kind,
        path,
        file,
        schema: &schema,
    };
    for error in validator.iter_errors(&file.value) {
        report.push(&error, problems);
    }
    Ok(())
}

struct Report<'a> {
    kind: ConfigKind,
    path: &'a str,
    file: &'a Converted,
    schema: &'a Value,
}

impl Report<'_> {
    fn push(&self, error: &ValidationError<'_>, problems: &mut Vec<ProjectError>) {
        let at: Vec<Segment> = error
            .instance_path()
            .iter()
            .map(|segment| match segment {
                LocationSegment::Property(key) => Segment::Key(key.into_owned()),
                LocationSegment::Index(index) => Segment::Index(index),
            })
            .collect();
        match error.kind() {
            ValidationErrorKind::AdditionalProperties { unexpected } => {
                self.push_unknown(error, &at, unexpected, problems);
            }
            ValidationErrorKind::Required { property } => {
                let mut child = at.clone();
                child.push(Segment::Key(
                    property.as_str().unwrap_or_default().to_owned(),
                ));
                problems.push(self.invalid(&child, self.line(&at, false), error, None));
            }
            ValidationErrorKind::Enum { options } => {
                let given = self
                    .file
                    .value
                    .pointer(&pointer(&at))
                    .and_then(Value::as_str);
                let options = options.as_array().map(Vec::as_slice).unwrap_or_default();
                let suggestion = given.and_then(|given| {
                    closest_name(given, options.iter().filter_map(Value::as_str)).map(str::to_owned)
                });
                problems.push(self.invalid(&at, self.line(&at, false), error, suggestion));
            }
            ValidationErrorKind::OneOfNotValid { context } if self.is_object(&at) => {
                self.push_branch(&at, context, problems);
            }
            _ => problems.push(self.invalid(&at, self.line(&at, false), error, None)),
        }
    }

    /// Reports an object that matches no branch of a `oneOf`, such as a source of one kind
    /// (MG-REQ-0376). The branches tell themselves apart by the `const` of a key: a branch
    /// whose `const` key is wrong, or missing, is not the one meant. When every branch is
    /// ruled out, the key itself is missing or wrong, and the keywords beside the `oneOf`
    /// report it. Of the branches left, the one with the fewest errors, the first on a tie,
    /// is reported, error by error.
    fn push_branch(
        &self,
        at: &[Segment],
        branches: &[Vec<ValidationError<'static>>],
        problems: &mut Vec<ProjectError>,
    ) {
        let depth = at.len() + 1;
        let meant = branches
            .iter()
            .filter(|errors| !errors.iter().any(|error| self.rules_out(error, depth)))
            .min_by_key(|errors| errors.len());
        for error in meant.into_iter().flatten() {
            self.push(error, problems);
        }
    }

    /// Whether `error`, found in a branch, says the branch is not the one meant: the key at
    /// `depth` has the wrong `const`, or is missing where the branch gives it a `const`.
    fn rules_out(&self, error: &ValidationError<'_>, depth: usize) -> bool {
        match error.kind() {
            ValidationErrorKind::Constant { .. } => error.instance_path().iter().count() == depth,
            ValidationErrorKind::Required { property } => {
                let keyword = error.schema_path().as_str();
                let parent = keyword.strip_suffix("/required").unwrap_or_default();
                self.node_at(parent)
                    .and_then(|node| node.get("properties"))
                    .and_then(|properties| properties.get(property.as_str().unwrap_or_default()))
                    .is_some_and(|key| self.follow(key).get("const").is_some())
            }
            _ => false,
        }
    }

    /// Reports each key of `unexpected`, found at `at` by `error`, with the closest allowed
    /// key; a key of a `[seeds.<name>]` as [`ProjectError::UnknownSeedKey`] (MG-REQ-0645.2).
    fn push_unknown(
        &self,
        error: &ValidationError<'_>,
        at: &[Segment],
        unexpected: &[String],
        problems: &mut Vec<ProjectError>,
    ) {
        let allowed = self.allowed_keys(error.schema_path().as_str());
        for key in unexpected {
            let mut child = at.to_vec();
            child.push(Segment::Key(key.clone()));
            let path = self.path.to_owned();
            let line = self.line(&child, true);
            let suggestion =
                closest_name(key, allowed.iter().map(String::as_str)).map(str::to_owned);
            let key = dotted(&child);
            problems.push(if self.is_seed_key(&child) {
                ProjectError::UnknownSeedKey {
                    path,
                    line,
                    key,
                    suggestion,
                }
            } else {
                ProjectError::UnknownKey {
                    path,
                    line,
                    key,
                    suggestion,
                }
            });
        }
    }

    /// Whether `at` is a key of a `[seeds.<name>]` of `project.toml`, which MG-REQ-0645.2
    /// reports with its own code.
    fn is_seed_key(&self, at: &[Segment]) -> bool {
        self.kind == ConfigKind::Project
            && matches!(at, [Segment::Key(seeds), Segment::Key(_), Segment::Key(_)] if seeds == "seeds")
    }

    /// Whether the value at `at` is an object.
    fn is_object(&self, at: &[Segment]) -> bool {
        self.file
            .value
            .pointer(&pointer(at))
            .is_some_and(Value::is_object)
    }

    fn invalid(
        &self,
        at: &[Segment],
        line: usize,
        error: &ValidationError<'_>,
        suggestion: Option<String>,
    ) -> ProjectError {
        ProjectError::InvalidValue {
            path: self.path.to_owned(),
            line,
            key: dotted(at),
            message: error.to_string(),
            suggestion,
        }
    }

    /// The line of the key (or of the value) at `at`; line 1 for the root.
    fn line(&self, at: &[Segment], key: bool) -> usize {
        self.file
            .lines
            .get(at)
            .map_or(1, |lines| if key { lines.key } else { lines.value })
    }

    /// The keys allowed by the object schema whose `additionalProperties` is at `keyword`, a
    /// schema path that does not show the `$ref` it went through.
    fn allowed_keys(&self, keyword: &str) -> Vec<String> {
        let parent = keyword
            .strip_suffix("/additionalProperties")
            .unwrap_or_default();
        self.node_at(parent)
            .and_then(|value| value.get("properties"))
            .and_then(Value::as_object)
            .map(|properties| properties.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The schema node at `location`, a schema path that does not show the `$ref` it went
    /// through, with its own `$ref` followed.
    fn node_at(&self, location: &str) -> Option<&Value> {
        let mut node = Some(self.schema);
        for step in location.split('/').filter(|step| !step.is_empty()) {
            node = node.and_then(|value| {
                let value = self.follow(value);
                match step.parse::<usize>() {
                    Ok(index) if value.is_array() => value.get(index),
                    _ => value.get(step),
                }
            });
        }
        node.map(|value| self.follow(value))
    }

    /// Follows the `$ref` of a schema node to a `#/$defs/<name>` of the schema, if it has one.
    fn follow<'s>(&'s self, node: &'s Value) -> &'s Value {
        node.get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix('#'))
            .and_then(|target| self.schema.pointer(target))
            .unwrap_or(node)
    }
}

/// Writes a path as a JSON Pointer.
fn pointer(at: &[Segment]) -> String {
    at.iter()
        .map(|segment| match segment {
            Segment::Key(key) => format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            Segment::Index(index) => format!("/{index}"),
        })
        .collect()
}
