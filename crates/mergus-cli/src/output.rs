//! What a command prints: events on standard output with `--json` (MG-REQ-0260, MG-REQ-0261),
//! otherwise errors in the layout of MG-REQ-1322 and, with `--verbose`, stage lines on
//! standard error (MG-REQ-1347).

use std::io::Write;
use std::time::Duration;

use mergus_contracts::generated::mergus_events_v1::{
    Location, MergusEvents, MergusEventsSeverity, MergusEventsType, Step,
};
use mergus_contracts::{Contract, ValidDocument, validate_event};
use mergus_ports::{Clock, MonotonicTime, Random};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::exit;
use crate::layout::render;
use crate::mode::{Mode, Verbosity};
use crate::time::rfc3339;

/// The environment every command works on until environments exist (MG-REQ-0296.5).
pub(crate) const ENVIRONMENT: &str = "prod";

/// The output of one command run.
pub(crate) struct Output<'a> {
    json: bool,
    color: bool,
    verbosity: Verbosity,
    clock: &'a dyn Clock,
    run_id: String,
    sequence: i64,
    started: MonotonicTime,
    stdout: &'a mut dyn Write,
    stderr: &'a mut dyn Write,
}

impl<'a> Output<'a> {
    /// Starts the output of a run in `mode`, with colors when `color` is true.
    pub(crate) fn new(
        mode: Mode,
        color: bool,
        clock: &'a dyn Clock,
        random: &dyn Random,
        stdout: &'a mut dyn Write,
        stderr: &'a mut dyn Write,
    ) -> Self {
        Self {
            json: mode == Mode::Events,
            color,
            verbosity: Verbosity::Normal,
            clock,
            run_id: run_id(random),
            sequence: 0,
            started: clock.monotonic(),
            stdout,
            stderr,
        }
    }

    /// Sets how much human output the rest of the run shows.
    pub(crate) fn set_verbosity(&mut self, verbosity: Verbosity) {
        self.verbosity = verbosity;
    }

    /// Reports, with `--verbose` and without `--json`, that stage `number` took `duration`.
    pub(crate) fn stage(&mut self, number: u8, duration: Duration) -> Result<(), CliError> {
        if self.json || self.verbosity != Verbosity::Verbose {
            return Ok(());
        }
        write(
            self.stderr,
            &format!("stage {number}: {} ms\n", duration.as_millis()),
        )
    }

    /// Prints `text`, the result of a command for a person: on standard output, or on
    /// standard error with `--json` so that standard output holds only events; nothing with
    /// `--quiet` (MG-REQ-1347.3).
    pub(crate) fn report(&mut self, text: &str) -> Result<(), CliError> {
        if self.verbosity == Verbosity::Quiet {
            Ok(())
        } else if self.json {
            write(self.stderr, text)
        } else {
            write(self.stdout, text)
        }
    }

    /// Asks `question` on standard error.
    pub(crate) fn ask(&mut self, question: &str) -> Result<(), CliError> {
        write(self.stderr, question)
    }

    /// Prints `document` alone, as one line of standard output (MG-REQ-1348).
    pub(crate) fn document(&mut self, document: &ValidDocument) -> Result<(), CliError> {
        write(self.stdout, &format!("{}\n", document.value()))
    }

    /// Reports that `command` starts with the arguments `args`.
    pub(crate) fn started(&mut self, command: &str, args: &[String]) -> Result<(), CliError> {
        if !self.json {
            return Ok(());
        }
        let mut event = self.event(MergusEventsType::RunStarted);
        event.command = Some(command.to_owned());
        event.arguments = Some(args.to_vec());
        event.environment = Some(ENVIRONMENT.to_owned());
        self.emit(&event)
    }

    /// Reports, with `--json`, that the plan of `steps` is ready (MG-REQ-0746).
    pub(crate) fn plan_ready(&mut self, steps: Vec<Step>) -> Result<(), CliError> {
        if !self.json {
            return Ok(());
        }
        let mut event = self.event(MergusEventsType::PlanReady);
        event.environment = Some(ENVIRONMENT.to_owned());
        event.steps = Some(steps);
        self.emit(&event)
    }

    /// Reports `diagnostic` and returns the exit code it calls for.
    pub(crate) fn diagnostic(&mut self, diagnostic: &Diagnostic) -> Result<u8, CliError> {
        let resolved = diagnostic.resolve()?;
        if self.json {
            let mut event = self.event(MergusEventsType::Diagnostic);
            event.code = Some(diagnostic.code.to_owned());
            event.severity = Some(MergusEventsSeverity::Error);
            event.message = Some(diagnostic.message.clone());
            event.location = diagnostic.location.as_ref().and_then(|at| {
                at.line.map(|line| Location {
                    file: at.file.clone(),
                    line: number(line),
                    column: at.column.map(number),
                })
            });
            event.hint = Some(resolved.hint.clone());
            event.next = (!diagnostic.next.is_empty()).then(|| diagnostic.next.clone());
            self.emit(&event)?;
        } else {
            write(self.stderr, &render(&resolved, self.color))?;
        }
        Ok(resolved.exit_code)
    }

    /// Reports that the run ends with exit `code`.
    pub(crate) fn finished(&mut self, code: u8) -> Result<(), CliError> {
        if !self.json {
            return Ok(());
        }
        let retryable = exit::retryable(code)
            .ok_or_else(|| CliError::Internal(format!("exit code {code} is not in the table")))?;
        let elapsed = self
            .clock
            .monotonic()
            .elapsed()
            .saturating_sub(self.started.elapsed());
        let mut event = self.event(MergusEventsType::RunFinished);
        event.status = Some(
            if code == exit::SUCCESS {
                "succeeded"
            } else {
                "failed"
            }
            .to_owned(),
        );
        event.exit_code = Some(i64::from(code));
        event.retryable = Some(retryable);
        event.duration_ms = Some(i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX));
        self.emit(&event)
    }

    /// Reports an internal error that the output itself cannot carry.
    pub(crate) fn internal(&mut self, error: &CliError) {
        let diagnostic = Diagnostic::of_cli(error);
        let text = match diagnostic.resolve() {
            Ok(resolved) => render(&resolved, self.color),
            Err(_) => format!("error[{}]: {error}\n", error.code()),
        };
        // Nothing is left to report a failure to write standard error to.
        let _ = write(self.stderr, &text);
    }

    /// An event of `kind` with the common fields, the next sequence number and the time now.
    fn event(&mut self, kind: MergusEventsType) -> MergusEvents {
        self.sequence += 1;
        MergusEvents {
            contract: MergusEvents::NAME.to_owned(),
            contract_version: i64::from(MergusEvents::VERSION),
            r#type: kind,
            run_id: self.run_id.clone(),
            sequence: self.sequence,
            timestamp: rfc3339(self.clock.now()),
            actor: None,
            command: None,
            arguments: None,
            environment: None,
            steps: None,
            step_id: None,
            model: None,
            engine: None,
            status: None,
            rows: None,
            bytes: None,
            duration_ms: None,
            code: None,
            severity: None,
            message: None,
            location: None,
            hint: None,
            next: None,
            test: None,
            failing_rows: None,
            manifest_id: None,
            exit_code: None,
            retryable: None,
        }
    }

    /// Validates `event` (MG-REQ-0295) and prints it as one line of standard output.
    fn emit(&mut self, event: &MergusEvents) -> Result<(), CliError> {
        let valid = validate_event(event).map_err(|error| CliError::Internal(error.to_string()))?;
        write(self.stdout, &format!("{}\n", valid.value()))
    }
}

/// A line or a column as an event field.
fn number(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn write(out: &mut dyn Write, text: &str) -> Result<(), CliError> {
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|error| CliError::Internal(format!("the output cannot be written: {error}")))
}

/// A W3C trace id drawn from `random`: 32 lowercase hexadecimal characters, never all zero
/// (MG-REQ-0296.1).
fn run_id(random: &dyn Random) -> String {
    let mut bytes = [0_u8; 16];
    for _ in 0..8 {
        random.fill(&mut bytes);
        if bytes.iter().any(|byte| *byte != 0) {
            break;
        }
    }
    if bytes.iter().all(|byte| *byte == 0) {
        bytes[15] = 1;
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
