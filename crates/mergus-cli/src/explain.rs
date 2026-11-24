//! `mergus explain <argument>` (MG-REQ-1310, MG-REQ-1352): an error code when the argument is
//! `MG-` and four digits, a model otherwise.

use std::fmt::Write as _;

use mergus_project::{Effective, Origin, Setting, closest_name};

use crate::catalog;
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::models::{self, Project, ProjectModel};
use crate::run::Ports;

/// Whether `argument` is an error code: `MG-` and exactly four ASCII digits.
pub(crate) fn is_code(argument: &str) -> bool {
    argument
        .strip_prefix("MG-")
        .is_some_and(|digits| digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// The text `mergus explain <argument>` prints, or the diagnostics of the project when it
/// must be read and is not valid.
///
/// # Errors
///
/// Fails with [`CliError::UnknownCode`] or [`CliError::UnknownName`], naming the closest
/// code or model (MG-REQ-1352.3), and when the project cannot be found or read.
pub(crate) fn explain(
    ports: &Ports<'_>,
    argument: &str,
) -> Result<Result<String, Vec<Diagnostic>>, CliError> {
    if is_code(argument) {
        return catalog::file(argument)
            .map(|text| Ok(text.to_owned()))
            .ok_or_else(|| CliError::UnknownCode {
                given: argument.to_owned(),
                closest: closest_name(argument, catalog::codes()),
            });
    }
    let project = match models::read(ports)? {
        Ok(project) => project,
        Err(diagnostics) => return Ok(Err(diagnostics)),
    };
    let model = project
        .model(argument)
        .ok_or_else(|| CliError::UnknownName {
            kind: "model",
            given: argument.to_owned(),
            closest: closest_name(argument, project.models.iter().map(ProjectModel::name))
                .map(str::to_owned),
        })?;
    Ok(Ok(describe(&project, model)))
}

/// The name, identifier, file, inputs and readers of `model`, then each effective option
/// with its value and origin (MG-REQ-1352.2).
fn describe(project: &Project, model: &ProjectModel) -> String {
    let list = |items: Vec<&str>| {
        if items.is_empty() {
            "none".to_owned()
        } else {
            items.join(", ")
        }
    };
    let mut text = format!(
        "model: {}\nidentifier: {}\nfile: {}\ninputs: {}\nread by: {}\noptions:\n",
        model.name(),
        model.identifier.as_str(),
        model.path(),
        list(model.inputs.iter().map(String::as_str).collect()),
        list(project.readers(model.name())),
    );
    for (name, effective) in &model.options.options {
        let value = match effective {
            Effective::Single(setting) => setting_text(setting),
            Effective::List(items) if items.is_empty() => "[] (default)".to_owned(),
            Effective::List(items) => {
                let items: Vec<String> = items.iter().map(setting_text).collect();
                format!("[{}]", items.join(", "))
            }
        };
        // Writing to a `String` cannot fail.
        let _ = writeln!(text, "  {name} = {value}");
    }
    text
}

/// A value and its origin: `<value> (<origin>)`.
fn setting_text(setting: &Setting) -> String {
    let origin = match &setting.origin {
        Origin::File { path, line } => format!("{path}:{line}"),
        Origin::Convention => "convention".to_owned(),
        Origin::Default => "default".to_owned(),
    };
    format!("{} ({origin})", setting.value)
}
