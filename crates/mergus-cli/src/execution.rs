//! `mergus run [<selection>]` and `mergus backfill <model> --from <date> [--to <date>]`
//! (MG-REQ-0710): stages 1 to 4 as `plan` does them, then stage 5 into candidates in work,
//! through the run cycle of `mergus-runtime`. Nothing is published until a phase adds
//! stage 6 (MG-REQ-0710.3).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use mergus_contracts::generated::mergus_plan_v1::StepAction;
use mergus_plan::{
    Backfill, Cleared, Gate, GateCheck, GateInput, Instant, Plan, PlanError, PlanRequest,
    determinism, evaluate, type_passport,
};
use mergus_ports::Database;
use mergus_project::RunParameters;
use mergus_runtime::{
    Executed, ReadError, ReadPorts, ReadProject, Run, RunError, RunReport, plan_models,
    plan_project, read_project,
};

use crate::args::Invocation;
use crate::commands::{Command, EXCLUDE, FROM, MAX_PARALLEL, MODEL, SELECTION, TO};
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::exit;
use crate::limits;
use crate::models::{self, Project};
use crate::output::{ENVIRONMENT, Output};
use crate::planning::{refresh_diagnostics, steps};
use crate::run::Ports;
use crate::selection::select;
use crate::settings::max_parallel;
use crate::work::{Context, work};

/// The phase whose checks the gates this phase does not check wait for.
const LATER: &str = "0.5";

const SECONDS_PER_DAY: i64 = 86_400;

/// Runs stages 1 to 5 for `invocation` and returns the exit code the run itself ends with,
/// 7 when cancelled, with its diagnostics. An error in stages 1 to 4 executes nothing
/// (MG-REQ-0710.1).
///
/// # Errors
///
/// Fails when the project cannot be found or read, an argument is invalid, no DuckDB is
/// available, or the run cannot be reported.
pub(crate) fn run(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<(u8, Vec<Diagnostic>), CliError> {
    let project = match models::read(ports)? {
        Ok(project) => project,
        Err(diagnostics) => return Ok((exit::SUCCESS, diagnostics)),
    };
    let request = request(ports, invocation, &project)?;
    let database = ports.database.ok_or(CliError::NoDatabase)?;
    let read = match read_stage(ports, database, &project.root)? {
        Ok(read) => read,
        Err(diagnostics) => return Ok((exit::SUCCESS, diagnostics)),
    };
    let cycle = Run::begin_with_limits(
        ports.files,
        &project.root,
        output.run_id(),
        ports.cancel.clone(),
        limits::of(invocation, limits::of_project(&read.configuration)),
        ports.clock,
    );
    let cycle = match cycle {
        Ok(cycle) => cycle,
        Err(error) => return Ok((exit::SUCCESS, vec![of_run(&error)?])),
    };
    let context = Context {
        ports,
        database,
        parameters: parameters(ports),
    };
    // A run that ends before stage 5 is finalized when `cycle` is dropped (MG-REQ-0709.1).
    let (plan, outcome) = match stages(&context, &project, &read, &request, &cycle, output)? {
        Staged::Refused(diagnostics) => return Ok((exit::SUCCESS, diagnostics)),
        Staged::Ran { plan, outcome } => (plan, outcome),
    };
    ended(&cycle.finish(outcome), &plan, output)
}

/// The exit code and the diagnostics of the run `report` of `plan`, after its human summary
/// (MG-REQ-0710.3); the finalizer's failures are warnings after the run's own error
/// (MG-REQ-0709.2).
fn ended(
    report: &RunReport,
    plan: &Plan,
    output: &mut Output<'_>,
) -> Result<(u8, Vec<Diagnostic>), CliError> {
    let (code, mut diagnostics) = match &report.outcome {
        Ok(Executed::Finished { .. }) => {
            output.report(&summary(plan))?;
            (exit::SUCCESS, Vec::new())
        }
        Ok(Executed::Cancelled) => {
            output.report("run cancelled; nothing was published\n")?;
            (exit::CANCELLED, Vec::new())
        }
        Err(error) => (exit::SUCCESS, vec![of_run(error)?]),
    };
    for error in &report.warnings {
        let mut warning = of_run(error)?;
        warning.warning = true;
        diagnostics.push(warning);
    }
    Ok((code, diagnostics))
}

/// How stages 2 to 5 ended.
enum Staged {
    /// Stages 2 to 4 refused the run, with these diagnostics; nothing was executed.
    Refused(Vec<Diagnostic>),
    /// Stage 5 ran the plan, with this outcome.
    Ran {
        plan: Box<Plan>,
        outcome: Result<Executed, RunError>,
    },
}

/// Stages 2 to 5 inside the run cycle `cycle`, which the caller finishes.
fn stages(
    context: &Context<'_>,
    project: &Project,
    read: &ReadProject,
    request: &PlanRequest,
    cycle: &Run<'_>,
    output: &mut Output<'_>,
) -> Result<Staged, CliError> {
    let plan = match plan_project(read, request, cycle.available()) {
        Ok(plan) => plan,
        Err(PlanError::Refresh(errors)) => {
            return Ok(Staged::Refused(refresh_diagnostics(project, &errors)));
        }
        Err(error) => return Err(CliError::Internal(error.to_string())),
    };
    let cleared = match gates(read)? {
        Ok(cleared) => cleared,
        Err(diagnostics) => return Ok(Staged::Refused(diagnostics)),
    };
    let mut work = match work(context, read, &plan)? {
        Ok(work) => work,
        Err(diagnostics) => return Ok(Staged::Refused(diagnostics)),
    };
    let adopted = match cycle.adopt(&plan) {
        Ok(adopted) => adopted,
        Err(error) => return Ok(Staged::Refused(vec![of_run(&error)?])),
    };
    output.plan_ready(steps(&plan))?;
    let outcome = cycle.execute(&adopted, &cleared, &mut work);
    Ok(Staged::Ran {
        plan: Box::new(plan),
        outcome,
    })
}

/// The plan request of `invocation`: the selection, or the backfilled model and its range
/// (MG-REQ-0657), and `max_parallel`.
fn request(
    ports: &Ports<'_>,
    invocation: &Invocation,
    project: &Project,
) -> Result<PlanRequest, CliError> {
    let backfill = invocation.command == Command::Backfill;
    let words = match invocation.value(MODEL) {
        Some(model) if backfill => vec![model],
        _ => invocation.values(SELECTION),
    };
    let selected = select(project, &words, invocation.value(EXCLUDE))?;
    let names: BTreeSet<String> = selected.into_keys().collect();
    let flag = invocation
        .value(MAX_PARALLEL)
        .map(|value| (MAX_PARALLEL, value));
    let setting = max_parallel(flag, ports.environment, ports.files)?;
    let mut request = PlanRequest::new(env!("CARGO_PKG_VERSION"), ENVIRONMENT, names);
    request.max_parallel = setting.value;
    if let (true, Some(model)) = (backfill, invocation.value(MODEL)) {
        request
            .backfill
            .insert(model.to_owned(), range(ports, invocation)?);
    }
    Ok(request)
}

/// The range of a backfill: `--from`, and `--to` or the run's start (MG-REQ-0657.1).
fn range(ports: &Ports<'_>, invocation: &Invocation) -> Result<Backfill, CliError> {
    let from_text = invocation.value(FROM).unwrap_or_default();
    let from = date(FROM, from_text)?;
    let to = match invocation.value(TO) {
        Some(text) => date(TO, text)?,
        None => Instant::from_unix_seconds(now_seconds(ports))
            .ok_or_else(|| CliError::Internal("the clock is out of range".to_owned()))?,
    };
    if to < from {
        return Err(CliError::InvertedBackfill {
            from: from_text.to_owned(),
            to: invocation.value(TO).unwrap_or_default().to_owned(),
        });
    }
    Ok(Backfill { from, to })
}

/// The date `YYYY-MM-DD` given to `option`.
fn date(option: &'static str, given: &str) -> Result<Instant, CliError> {
    let invalid = || CliError::InvalidDate {
        option,
        given: given.to_owned(),
    };
    let mut parts = given.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid());
    };
    let digits = |text: &str, width: usize| {
        (text.len() == width && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse::<u32>().ok())
            .flatten()
    };
    let (Some(year), Some(month), Some(day)) = (digits(year, 4), digits(month, 2), digits(day, 2))
    else {
        return Err(invalid());
    };
    let year = i32::try_from(year).map_err(|_| invalid())?;
    Instant::from_date(year, month, day).ok_or_else(invalid)
}

/// Stage 1 through the runtime, every error at once (MG-REQ-0702).
fn read_stage(
    ports: &Ports<'_>,
    database: &dyn Database,
    root: &Path,
) -> Result<Result<ReadProject, Vec<Diagnostic>>, CliError> {
    let read_ports = ReadPorts {
        files: ports.files,
        parser: ports.sql_parser,
        python: None,
        database: Some(database),
    };
    match read_project(&read_ports, root) {
        Ok(read) => Ok(Ok(read)),
        Err(ReadError::Invalid(files)) => Ok(Err(files
            .iter()
            .flat_map(|file| file.errors.iter().map(Diagnostic::of_project))
            .collect())),
        Err(ReadError::Unreadable(error)) => Err(CliError::Unreadable(error.to_string())),
    }
}

/// Stage 4: the gates this phase checks, the others waiting for a later phase.
fn gates(read: &ReadProject) -> Result<Result<Cleared, Vec<Diagnostic>>, CliError> {
    let models = plan_models(read);
    let checks = Gate::ALL
        .into_iter()
        .filter(|gate| *gate != Gate::Credentials)
        .map(|gate| {
            let check = match gate {
                Gate::TypePassport => GateCheck::Checked(type_passport(&models)),
                Gate::Determinism => GateCheck::Checked(determinism(&models)),
                _ => GateCheck::NotApplicable {
                    phase: LATER.to_owned(),
                },
            };
            (gate, check)
        })
        .collect();
    let input = GateInput {
        checks,
        ..GateInput::default()
    };
    let report = evaluate(&input).map_err(|error| CliError::Internal(error.to_string()))?;
    Ok(report.clear().map_err(|blocked| {
        blocked
            .errors()
            .iter()
            .map(|error| Diagnostic::of_code(error.code, error.to_string()))
            .collect()
    }))
}

/// The diagnostic of a failed run cycle; an error without a code is a defect of Mergus.
fn of_run(error: &RunError) -> Result<Diagnostic, CliError> {
    error
        .code()
        .map(|code| Diagnostic::of_code(code, error.to_string()))
        .ok_or_else(|| CliError::Internal(error.to_string()))
}

/// What a finished run prints for a person (MG-REQ-0710.3).
fn summary(plan: &Plan) -> String {
    let steps = &plan.document().steps;
    let computed = steps
        .iter()
        .filter(|step| step.action == StepAction::Compute)
        .count();
    let mut text = String::new();
    // Writing to a `String` cannot fail.
    let _ = writeln!(
        text,
        "run: {computed} computed, {} skipped; nothing was published, the results stay as candidates in work",
        steps.len() - computed
    );
    text
}

/// The run parameters: `$run_date` is the day the run starts.
fn parameters(ports: &Ports<'_>) -> RunParameters {
    let days = now_seconds(ports).div_euclid(SECONDS_PER_DAY);
    RunParameters {
        run_date: i32::try_from(days).unwrap_or_default(),
        ..RunParameters::default()
    }
}

fn now_seconds(ports: &Ports<'_>) -> i64 {
    let seconds = ports.clock.now().unix_nanos().div_euclid(1_000_000_000);
    i64::try_from(seconds).unwrap_or(i64::MAX)
}
