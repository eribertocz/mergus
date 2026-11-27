//! Simulated child processes: programs answer with outputs set by the test.

use std::collections::BTreeMap;
use std::sync::Mutex;

use mergus_ports::{PortError, Process, ProcessCommand, ProcessOutput};

use crate::rng::lock;

/// Child processes whose programs return the outputs registered for them.
#[derive(Debug, Default)]
pub struct SimProcess {
    programs: Mutex<BTreeMap<String, ProcessOutput>>,
    runs: Mutex<Vec<ProcessCommand>>,
}

impl SimProcess {
    /// Returns a process port that knows no program.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every later run of `program` return `output`.
    pub fn register(&self, program: &str, output: ProcessOutput) {
        lock(&self.programs).insert(program.to_owned(), output);
    }

    /// Returns every command run so far, in order, whether its program was known or not: what
    /// each child was started with, its environment included.
    #[must_use]
    pub fn runs(&self) -> Vec<ProcessCommand> {
        lock(&self.runs).clone()
    }
}

impl Process for SimProcess {
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, PortError> {
        lock(&self.runs).push(command.clone());
        lock(&self.programs)
            .get(&command.program)
            .cloned()
            .ok_or_else(|| PortError::NotFound(command.program.clone()))
    }
}
