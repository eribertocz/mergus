//! What a command prints: events on standard output with `--json` (MG-REQ-0260, MG-REQ-0261),
//! otherwise plain diagnostics on standard error.

use std::io::Write;

use mergus_contracts::generated::mergus_events_v1::{
    Location, MergusEvents, MergusEventsSeverity, MergusEventsType,
};
use mergus_contracts::{Contract, validate_event};
use mergus_ports::{Clock, MonotonicTime, Random};

use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::exit;
use crate::time::rfc3339;

/// The environment every command works on until environments exist (MG-REQ-0296.5).
const ENVIRONMENT: &str = "prod";

/// The output of one command run.
pub(crate) struct Output<'a> {
    json: bool,
    clock: &'a dyn Clock,
    run_id: String,
    sequence: i64,
    started: MonotonicTime,
    stdout: &'a mut dyn Write,
    stderr: &'a mut dyn Write,
}

impl<'a> Output<'a> {
    /// Starts the output of a run; `json` chooses events.
    pub(crate) fn new(
        json: bool,
        clock: &'a dyn Clock,
        random: &dyn Random,
        stdout: &'a mut dyn Write,
        stderr: &'a mut dyn Write,
    ) -> Self {
        Self {
            json,
            clock,
            run_id: run_id(random),
            sequence: 0,
            started: clock.monotonic(),
            stdout,
            stderr,
        }
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

    /// Reports `diagnostic` and returns the exit code it calls for.
    pub(crate) fn diagnostic(&mut self, diagnostic: &Diagnostic) -> Result<u8, CliError> {
        let resolved = diagnostic.resolve()?;
        if self.json {
            let mut event = self.event(MergusEventsType::Diagnostic);
            event.code = Some(diagnostic.code.to_owned());
            event.severity = Some(MergusEventsSeverity::Error);
            event.message = Some(diagnostic.message.clone());
            event.location = diagnostic.location.as_ref().map(|at| Location {
                file: at.file.clone(),
                line: i64::try_from(at.line).unwrap_or(i64::MAX),
                column: None,
            });
            event.hint = Some(resolved.hint.clone());
            event.next = (!diagnostic.next.is_empty()).then(|| diagnostic.next.clone());
            self.emit(&event)?;
        } else {
            let mut text = format!("error[{}]: {}\n", diagnostic.code, diagnostic.message);
            if let Some(at) = &diagnostic.location {
                text.push_str(&format!("  --> {}:{}\n", at.file, at.line));
            }
            text.push_str(&format!("  hint: {}\n", resolved.hint));
            write(self.stderr, &text)?;
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
        let text = format!("error[{}]: {error}\n", error.code());
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
