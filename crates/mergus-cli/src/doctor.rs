//! `mergus doctor` (MG-REQ-1004): for each secret reference of the sources, which link of the
//! resolution chain answers (MG-REQ-1001), and never its value.

use std::collections::BTreeSet;

use mergus_project::{Configuration, read_configuration};
use mergus_security::{SecretRef, origin};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::output::Output;
use crate::project;
use crate::run::Ports;
use crate::validate::collect;

/// The keys of a source that hold a secret reference (MG-REQ-1004.1).
const SECRET_KEYS: [&str; 2] = ["connection", "replica"];

/// Prints one line per secret reference the sources use, sorted by name: `<name>: <origin>`,
/// the origin being `environment <variable>`, `keychain mergus/<name>` or `missing`
/// (MG-REQ-1004.1). Each missing reference is returned as an `MG-1003` diagnostic, every one
/// in the same pass (MG-REQ-1004.3). It reads the project and the keychain and writes nothing.
///
/// # Errors
///
/// Returns the diagnostics of the configuration when it is not valid; fails when the project
/// cannot be found or read.
pub(crate) fn doctor(
    ports: &Ports<'_>,
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
    Ok(diagnostics)
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
