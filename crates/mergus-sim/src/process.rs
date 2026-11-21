//! Simulated child processes: programs answer with outputs set by the test.

use std::collections::BTreeMap;
use std::sync::Mutex;

use mergus_ports::{PortError, Process, ProcessCommand, ProcessOutput};

use crate::rng::lock;

/// Child processes whose programs return the outputs registered for them.
#[derive(Debug, Default)]
pub struct SimProcess {
    programs: Mutex<BTreeMap<String, ProcessOutput>>,
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
}

impl Process for SimProcess {
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, PortError> {
        lock(&self.programs)
            .get(&command.program)
            .cloned()
            .ok_or_else(|| PortError::NotFound(command.program.clone()))
    }
}
