//! The parts of the messages of [`super::ProjectError`].

/// Formats the models of a cycle, back to the first.
pub(super) fn cycle_names(models: &[String]) -> String {
    models
        .iter()
        .chain(models.first())
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Formats each reference of a cycle with the model it reads.
pub(super) fn cycle_references(models: &[String], references: &[(String, usize)]) -> String {
    let next = models.iter().skip(1).chain(models.first());
    references
        .iter()
        .zip(next)
        .map(|((path, line), name)| format!("`{path}` line {line} reads `{name}`"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Formats the key of an error, when there is one.
pub(super) fn key_prefix(key: Option<&str>) -> String {
    key.map_or_else(String::new, |key| format!("key `{key}` "))
}

/// Formats the closest valid name of an error, when there is one.
pub(super) fn hint(suggestion: Option<&str>) -> String {
    suggestion.map_or_else(String::new, |name| format!("; did you mean `{name}`?"))
}
