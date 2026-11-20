//! Child processes of the operating system.

use std::process::Command;

use super::port_error;
use crate::{PortError, Process, ProcessCommand, ProcessOutput};

/// Runs child processes of the operating system.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealProcess;

impl Process for RealProcess {
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, PortError> {
        let mut child = Command::new(&command.program);
        child.args(&command.args);
        child.envs(command.env.iter().map(|(name, value)| (name, value)));
        if let Some(dir) = &command.current_dir {
            child.current_dir(dir);
        }
        let output = child
            .output()
            .map_err(|error| port_error(&command.program, &error))?;
        Ok(ProcessOutput {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RealProcess;
    use crate::{PortError, Process, ProcessCommand};

    #[test]
    fn real_process_reports_a_missing_program() {
        let command = ProcessCommand {
            program: "mergus-ports-test-program-that-does-not-exist".to_owned(),
            ..ProcessCommand::default()
        };
        assert!(matches!(
            RealProcess.run(&command),
            Err(PortError::NotFound(_))
        ));
    }

    #[test]
    fn real_process_returns_the_output_of_cargo() {
        let command = ProcessCommand {
            program: env!("CARGO").to_owned(),
            args: vec!["--version".to_owned()],
            ..ProcessCommand::default()
        };
        let output = RealProcess.run(&command).unwrap();
        assert_eq!(output.code, Some(0));
        assert!(output.stdout.starts_with(b"cargo "));
    }
}
