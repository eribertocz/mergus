//! Source care in the binary: attaches a database source through its replica when it has
//! one (MG-REQ-0426), so that `doctor --source` can check its indexes (MG-REQ-0431) and
//! `plan` can show what reading it costs (MG-REQ-0432).

use std::path::Path;

use mergus_project::Source;
use mergus_security::{ExposeSecret, SecretRef, resolve};
use mergus_source::{
    AttachedSource, ConnectError, SourceCare, attach_duckdb_file, attach_postgres,
};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::run::Ports;

/// The `engine` of a database source that is a DuckDB file.
const DUCKDB: &str = "duckdb";

/// The `engine` of a PostgreSQL source.
const POSTGRESQL: &str = "postgresql";

/// Attaches the database source `source` of the project at `root` read-only: PostgreSQL
/// with the value of [`SourceCare::extraction_reference`] and the budget of `care`, a
/// DuckDB file at its `location`. Returns `Ok(None)` for a source that is not a database
/// read by DuckDB.
///
/// # Errors
///
/// Fails with the diagnostic of a secret that does not resolve, of an engine that differs
/// from the declared one, or of a source that cannot be reached.
pub(crate) fn attach(
    ports: &Ports<'_>,
    root: &Path,
    source: &Source,
    care: &SourceCare,
) -> Result<Option<AttachedSource>, Box<Diagnostic>> {
    let text = |key: &str| source.file.value.get(key).and_then(|value| value.as_str());
    if text("kind") != Some("database") {
        return Ok(None);
    }
    let engine = text("engine");
    if engine != Some(POSTGRESQL) && engine != Some(DUCKDB) {
        return Ok(None);
    }
    let Some(database) = ports.database else {
        let message = format!(
            "source `{}` cannot be reached: no DuckDB is available",
            source.name
        );
        return Err(Box::new(Diagnostic::of_cli(&CliError::Internal(message))));
    };
    let attached = if engine == Some(DUCKDB) {
        let Some(location) = text("location") else {
            return Ok(None);
        };
        attach_duckdb_file(database, &source.name, &root.join(location))
    } else {
        let Some(reference) = care.extraction_reference().and_then(SecretRef::parse) else {
            return Ok(None);
        };
        let value = resolve(&reference, ports.environment, ports.keychain)
            .map_err(|error| Box::new(Diagnostic::of_secret(&error)))?;
        let postgres = care.postgres_source(&source.name, value.expose_secret().to_owned());
        attach_postgres(database, &source.name, &postgres)
    };
    attached
        .map(Some)
        .map_err(|error| Box::new(of_connect(&source.name, &error)))
}

/// The diagnostic of a source that could not be attached.
fn of_connect(source_name: &str, error: &ConnectError) -> Diagnostic {
    match error.code() {
        Some(code) => Diagnostic::of_code(code, error.to_string()),
        None => Diagnostic::of_cli(&CliError::Internal(format!(
            "source `{source_name}` cannot be reached: {error}"
        ))),
    }
}
