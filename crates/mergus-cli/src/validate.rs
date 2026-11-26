//! `mergus validate`: every check of `mergus-project`, every problem in one pass, nothing
//! written (MG-REQ-1345).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    LayoutError, check_sql_models, read_configuration, read_layout, read_model_options,
};
use mergus_runtime::check_storage;
use mergus_source::check_sources;

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::project;
use crate::run::Ports;

/// Runs stage 1 on the project found from the current directory and returns every
/// diagnostic: those of the layout, then of the configuration, then of the folder options,
/// then of the storage of the zones by `mergus-runtime` (MG-REQ-0501), then of the sources by
/// `mergus-source` (MG-REQ-0409, MG-REQ-0414, MG-REQ-0416), then of the
/// `.sql` models, parsed by DuckDB through `ports.sql_parser` (MG-REQ-0193). The folder options
/// are checked only when the layout is valid, since they name its models; the storage and the
/// sources only when the configuration is valid; the `.sql` models only when both are, since the
/// configuration declares the tables they may read.
/// Each diagnostic with a line carries the text of that line when its file can be read.
///
/// # Errors
///
/// Fails when the project cannot be found or read, or a bundled schema is broken.
pub(crate) fn validate(ports: &Ports<'_>) -> Result<Vec<Diagnostic>, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let mut diagnostics = Vec::new();
    let layout = collect(read_layout(ports.files, &root), &mut diagnostics)?;
    let configuration = collect(read_configuration(ports.files, &root), &mut diagnostics)?;
    if let Some(layout) = &layout {
        collect(
            read_model_options(ports.files, &root, layout),
            &mut diagnostics,
        )?;
    }
    if let Some(configuration) = &configuration {
        let problems = check_storage(ports.files, &root, configuration)
            .map_err(|error| CliError::Unreadable(error.to_string()))?;
        diagnostics.extend(problems.iter().map(Diagnostic::of_storage));
        let problems = check_sources(ports.files, &root, configuration)
            .map_err(|error| CliError::Unreadable(error.to_string()))?;
        diagnostics.extend(problems.iter().map(Diagnostic::of_source));
    }
    if let (Some(layout), Some(configuration)) = (&layout, &configuration) {
        collect(
            check_sql_models(ports.files, &root, layout, configuration, ports.sql_parser),
            &mut diagnostics,
        )?;
    }
    for diagnostic in &mut diagnostics {
        diagnostic.source = source_line(ports.files, &root, diagnostic);
    }
    Ok(diagnostics)
}

/// The text of the line of the location of `diagnostic`, without its line ending; `None`
/// when it has no line or its file cannot be read as UTF-8.
fn source_line(files: &dyn FileSystem, root: &Path, diagnostic: &Diagnostic) -> Option<String> {
    let at = diagnostic.location.as_ref()?;
    let line = at.line?;
    let bytes = files.read(&root.join(&at.file)).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    text.lines()
        .nth(line.checked_sub(1)?)
        .map(|line| line.trim_end_matches('\r').to_owned())
}

/// Returns the value of a check, or adds its problems to `diagnostics`.
pub(crate) fn collect<T>(
    result: Result<T, LayoutError>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<T>, CliError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(LayoutError::Invalid(problems)) => {
            diagnostics.extend(problems.iter().map(Diagnostic::of_project));
            Ok(None)
        }
        Err(LayoutError::Port(error)) => Err(CliError::Unreadable(error.to_string())),
        Err(error @ LayoutError::Schema(_)) => Err(CliError::Internal(error.to_string())),
    }
}
