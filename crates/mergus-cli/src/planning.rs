//! `mergus plan [<selection>]` (MG-REQ-0746): writes the plan of the selected models to
//! `.mergus/plans/<plan_id>.json` and executes nothing.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use mergus_contracts::generated::mergus_events_v1::Step as EventStep;
use mergus_contracts::generated::mergus_plan_v1::{StepAction, StepReason};
use mergus_plan::{Plan, PlanError, PlanModel, PlanRequest, RefreshError};
use mergus_ports::{FileSystem, PortError};
use mergus_project::{Configuration, Source, read_configuration};
use mergus_runtime::{raw_off_sources, raw_retention, show_retention};
use mergus_source::{ImpactUnknown, SourceCare, TableImpact, declared_tables, source_impact};

use crate::args::Invocation;
use crate::care::attach;
use crate::commands::{EXCLUDE, MAX_PARALLEL, SELECTION};
use crate::diagnostic::{Diagnostic, Location};
use crate::error::CliError;
use crate::models::{self, Project};
use crate::output::{ENVIRONMENT, Output};
use crate::run::Ports;
use crate::selection::select;
use crate::settings::max_parallel;
use crate::validate::collect;

/// The folder of the plans, relative to the project root.
const PLANS: &str = ".mergus/plans";

/// Plans the models `invocation` selects, writes the plan unless a file of that name exists,
/// then reports `plan_ready` and the steps. Returns the diagnostics of the project when it is
/// not valid.
///
/// # Errors
///
/// Fails when the project cannot be read, the selection or a setting is invalid, the plan
/// cannot be made, or its file cannot be written.
pub(crate) fn plan(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Vec<Diagnostic>, CliError> {
    let project = match models::read(ports)? {
        Ok(project) => project,
        Err(diagnostics) => return Ok(diagnostics),
    };
    let mut diagnostics = Vec::new();
    let Some(configuration) = collect(
        read_configuration(ports.files, &project.root),
        &mut diagnostics,
    )?
    else {
        return Ok(diagnostics);
    };
    let selected = select(
        &project,
        &invocation.values(SELECTION),
        invocation.value(EXCLUDE),
    )?;
    let flag = invocation
        .value(MAX_PARALLEL)
        .map(|value| (MAX_PARALLEL, value));
    let setting = max_parallel(flag, ports.environment, ports.files)?;
    let planned = planned_models(&project);
    let names: BTreeSet<String> = selected.into_keys().collect();
    let mut request = PlanRequest::new(env!("CARGO_PKG_VERSION"), ENVIRONMENT, names);
    request.max_parallel = setting.value;
    let plan = match mergus_plan::plan(&planned, &request) {
        Ok(plan) => plan,
        Err(PlanError::Refresh(errors)) => return Ok(refresh_diagnostics(&project, &errors)),
        Err(error) => return Err(CliError::Internal(error.to_string())),
    };
    let relative = write(ports.files, &project.root, &plan)?;
    output.plan_ready(steps(&plan))?;
    let mut text = summary(&plan, &relative);
    text.push_str(&raw_summary(&configuration));
    diagnostics.extend(impact_lines(
        ports,
        &project.root,
        &configuration,
        &mut text,
    ));
    output.report(&text)?;
    Ok(diagnostics)
}

/// One coded diagnostic per refresh error, in their order, located at the file of its model
/// (MG-REQ-0654.1, MG-REQ-0657.2, MG-REQ-0661.1).
pub(crate) fn refresh_diagnostics(project: &Project, errors: &[RefreshError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|error| {
            let mut diagnostic = Diagnostic::of_code(error.code(), error.to_string());
            diagnostic.location = project
                .models
                .iter()
                .find(|model| model.options.name == error.model())
                .map(|model| Location {
                    file: model.options.path.clone(),
                    line: None,
                    column: None,
                });
            diagnostic
        })
        .collect()
}

/// The models of `project` as `mergus-plan` plans them.
fn planned_models(project: &Project) -> Vec<PlanModel> {
    project
        .models
        .iter()
        .map(|model| {
            PlanModel::from_project(
                &model.options,
                &model.identifier,
                &model.code,
                &model.inputs,
            )
        })
        .collect()
}

/// Appends to `text` one line per table of each source of `configuration`, with what reading
/// it costs the source (MG-REQ-0432), and returns the warnings of the sources that could not
/// be reached or read (MG-REQ-0432.2).
fn impact_lines(
    ports: &Ports<'_>,
    root: &Path,
    configuration: &Configuration,
    text: &mut String,
) -> Vec<Diagnostic> {
    let mut warnings = Vec::new();
    for source in &configuration.sources {
        let (impacts, warning) = impacts(ports, root, source);
        for impact in impacts {
            // Writing to a `String` cannot fail.
            let _ = writeln!(text, "{impact}");
        }
        warnings.extend(warning);
    }
    warnings
}

/// What reading each table of `source` costs it (MG-REQ-0432). A source that cannot be
/// attached, or whose catalog cannot be read, shows `unknown` and gives the `MG-0409`
/// warning with the reason; `plan` fails for none (MG-REQ-0432.2). A `connect` or `live`
/// source is read by no extraction, and a source without tables has none to show: neither
/// is attached.
fn impacts(
    ports: &Ports<'_>,
    root: &Path,
    source: &Source,
) -> (Vec<TableImpact>, Option<Diagnostic>) {
    let mode = source.file.value.get("mode").and_then(|mode| mode.as_str());
    if mode.is_some_and(|mode| mode != "copy") || declared_tables(source).is_empty() {
        return (Vec::new(), None);
    }
    let care = SourceCare::of(source, ENVIRONMENT);
    let (mut attached, mut reason) = match attach(ports, root, source, &care) {
        Ok(attached) => (attached, None),
        Err(diagnostic) => (
            None,
            Some(format!("{} {}", diagnostic.code, diagnostic.message)),
        ),
    };
    let known = source_impact(attached.as_mut(), source, &care);
    if let Some(attached) = &mut attached {
        // Only the estimates are wanted; the detach is a best effort.
        attached.connection.detach(&attached.alias).ok();
    }
    let impacts = known.unwrap_or_else(|error| {
        reason = Some(error.to_string());
        source_impact(None, source, &care).unwrap_or_default()
    });
    let warning = reason.map(|reason| {
        let unknown = ImpactUnknown {
            source_name: source.name.clone(),
            reason,
        };
        Diagnostic::warning_of_code(unknown.code(), unknown.to_string())
    });
    (impacts, warning)
}

/// Writes `plan` to `.mergus/plans/<plan_id>.json` under `root`, unless a file of that name
/// exists, and returns that path relative to `root`.
fn write(files: &dyn FileSystem, root: &Path, plan: &Plan) -> Result<String, CliError> {
    let relative = format!("{PLANS}/{}.json", plan.plan_id());
    let path = root.join(&relative);
    let unwritable = |error: PortError| CliError::Unwritable {
        path: relative.clone(),
        message: error.to_string(),
    };
    if !files.exists(&path).map_err(unwritable)? {
        files
            .create_dir_all(&root.join(PLANS))
            .map_err(unwritable)?;
        files.write(&path, &plan.bytes()).map_err(unwritable)?;
    }
    Ok(relative)
}

/// The steps of `plan` as the `plan_ready` event carries them.
pub(crate) fn steps(plan: &Plan) -> Vec<EventStep> {
    plan.document()
        .steps
        .iter()
        .map(|step| EventStep {
            model: step.model.clone(),
            action: action(step.action).to_owned(),
            reason: reason(step.reason).to_owned(),
        })
        .collect()
}

/// What `mergus plan` prints for a person: the file written, one line per step, then each
/// stale model left out with the selector that includes it (MG-REQ-0717).
fn summary(plan: &Plan, relative: &str) -> String {
    let mut text = format!("plan {}: {relative}\n", plan.plan_id());
    for step in &plan.document().steps {
        // Writing to a `String` cannot fail.
        let _ = writeln!(
            text,
            "{} {} ({})",
            action(step.action),
            step.model,
            reason(step.reason)
        );
    }
    for stale in &plan.document().stale_upstream {
        let _ = writeln!(
            text,
            "stale upstream: {}; include it with `{}`",
            stale.model, stale.include
        );
    }
    text
}

/// What `mergus plan` prints about raw for a person: one line per source with
/// `raw = false` (MG-REQ-0517.2), then the retention raw is expired with, when a source keeps
/// raw (MG-REQ-0516.2).
fn raw_summary(configuration: &Configuration) -> String {
    let mut text = String::new();
    for source in raw_off_sources(configuration) {
        // Writing to a `String` cannot fail.
        let _ = writeln!(text, "source {source}: raw = false");
    }
    if let Some(retention) = raw_retention(configuration) {
        let _ = writeln!(text, "raw retention: {}", show_retention(retention));
    }
    text
}

fn action(action: StepAction) -> &'static str {
    match action {
        StepAction::Compute => "compute",
        StepAction::Skip => "skip",
    }
}

fn reason(reason: StepReason) -> &'static str {
    match reason {
        StepReason::NoResult => "no_result",
        StepReason::ResultAvailable => "result_available",
    }
}
