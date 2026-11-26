//! `mergus doctor` (MG-REQ-1004): for each secret reference of the sources, which link of the
//! resolution chain answers (MG-REQ-1001), and never its value. With `--source <name>`, it
//! also warns for each key or incremental column of that source without an index
//! (MG-REQ-0431).

use std::collections::BTreeSet;
use std::path::Path;

use mergus_project::{Configuration, closest_name, read_configuration};
use mergus_security::{SecretRef, origin};
use mergus_source::{SourceCare, check_indexes};

use crate::args::Invocation;
use crate::care::attach;
use crate::commands::SOURCE;
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::output::{ENVIRONMENT, Output};
use crate::project;
use crate::run::Ports;
use crate::validate::collect;

/// The keys of a source that hold a secret reference (MG-REQ-1004.1).
const SECRET_KEYS: [&str; 2] = ["connection", "replica"];

/// Prints one line per secret reference the sources use, sorted by name: `<name>: <origin>`,
/// the origin being `environment <variable>`, `keychain mergus/<name>` or `missing`
/// (MG-REQ-1004.1). Each missing reference is returned as an `MG-1003` diagnostic, every one
/// in the same pass (MG-REQ-1004.3). With `--source`, the index warnings of that source
/// follow (MG-REQ-0431). It reads the project, the keychain and the source, and writes
/// nothing.
///
/// # Errors
///
/// Returns the diagnostics of the configuration when it is not valid; fails when the project
/// cannot be found or read, or `--source` names no source of the project.
pub(crate) fn doctor(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Vec<Diagnostic>, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let mut diagnostics = Vec::new();
    let Some(configuration) = collect(read_configuration(ports.files, &root), &mut diagnostics)?
    else {
        return Ok(diagnostics);
    };
    let mut report = String::new();
    for reference in references(&configuration) {
        let shown = match origin(&reference, ports.environment, ports.keychain) {
            Ok(origin) => origin.to_string(),
            Err(error) => {
                diagnostics.push(Diagnostic::of_secret(&error));
                "missing".to_owned()
            }
        };
        report.push_str(&format!("{}: {shown}\n", reference.name()));
    }
    output.report(&report)?;
    if let Some(name) = invocation.value(SOURCE) {
        diagnostics.extend(indexes(ports, &root, &configuration, name)?);
    }
    Ok(diagnostics)
}

/// The index warnings of the source `name` of `configuration` (MG-REQ-0431), or the
/// diagnostic of why it could not be checked.
fn indexes(
    ports: &Ports<'_>,
    root: &Path,
    configuration: &Configuration,
    name: &str,
) -> Result<Vec<Diagnostic>, CliError> {
    let source = configuration
        .sources
        .iter()
        .find(|source| source.name == name)
        .ok_or_else(|| CliError::UnknownName {
            kind: "source",
            given: name.to_owned(),
            closest: closest_name(
                name,
                configuration
                    .sources
                    .iter()
                    .map(|source| source.name.as_str()),
            )
            .map(str::to_owned),
        })?;
    let care = SourceCare::of(source, ENVIRONMENT);
    let mut attached = match attach(ports, root, source, &care) {
        Ok(Some(attached)) => attached,
        Ok(None) => return Ok(Vec::new()),
        Err(diagnostic) => return Ok(vec![*diagnostic]),
    };
    let checked = check_indexes(&mut attached, source);
    // The warnings are what doctor reports; the detach is a best effort.
    attached.connection.detach(&attached.alias).ok();
    match checked {
        Ok(warnings) => Ok(warnings
            .iter()
            .map(|warning| Diagnostic::of_index(warning, &source.file.path))
            .collect()),
        Err(error) => Ok(vec![Diagnostic::of_cli(&CliError::Internal(format!(
            "the indexes of source `{name}` cannot be read: {error}"
        )))]),
    }
}

/// Every secret reference the sources of `configuration` use, once each, sorted by name.
fn references(configuration: &Configuration) -> BTreeSet<SecretRef> {
    configuration
        .sources
        .iter()
        .flat_map(|source| {
            SECRET_KEYS
                .iter()
                .filter_map(|key| source.file.value.get(*key)?.as_str())
        })
        .filter_map(SecretRef::parse)
        .collect()
}
