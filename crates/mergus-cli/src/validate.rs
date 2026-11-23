//! `mergus validate`: every check of `mergus-project`, every problem in one pass, nothing
//! written (MG-REQ-1345).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    LayoutError, check_sql_models, read_configuration, read_layout, read_model_options,
};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::project;
use crate::run::Ports;

/// Runs stage 1 on the project found from the current directory and returns every
/// diagnostic: those of the layout, then of the configuration, then of the folder options,
/// then of the `.sql` models. The folder options are checked only when the layout is valid,
/// since they name its models; the `.sql` models only when the configuration is valid too,
/// since they name its source tables and seeds.
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
    if let (Some(layout), Some(configuration)) = (&layout, &configuration) {
        let checked = check_sql_models(ports.files, &root, layout, configuration, ports.sql);
        collect(checked, &mut diagnostics)?;
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
fn collect<T>(
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
