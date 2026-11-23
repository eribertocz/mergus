//! `mergus validate`: every check of `mergus-project`, every problem in one pass, nothing
//! written (MG-REQ-1345).

use mergus_project::{LayoutError, read_configuration, read_layout, read_model_options};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::project;
use crate::run::Ports;

/// Runs stage 1 on the project found from the current directory and returns every
/// diagnostic: those of the layout, then of the configuration, then of the folder options.
/// The folder options are checked only when the layout is valid, since they name its models.
///
/// # Errors
///
/// Fails when the project cannot be found or read, or a bundled schema is broken.
pub(crate) fn validate(ports: &Ports<'_>) -> Result<Vec<Diagnostic>, CliError> {
    let root = project::find(ports.environment, ports.files)?;
    let mut diagnostics = Vec::new();
    let layout = collect(read_layout(ports.files, &root), &mut diagnostics)?;
    collect(read_configuration(ports.files, &root), &mut diagnostics)?;
    if let Some(layout) = layout {
        collect(
            read_model_options(ports.files, &root, &layout),
            &mut diagnostics,
        )?;
    }
    Ok(diagnostics)
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
