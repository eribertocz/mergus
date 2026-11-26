//! What the kit found.

use std::fmt;

/// The result of one check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// The adapter passed the check.
    Passed,
    /// The adapter's declaration rules the check out, for the reason given.
    NotApplicable(String),
    /// The adapter failed the check, for the reason given.
    Failed(String),
}

/// One check and its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The name of the check.
    pub check: &'static str,
    /// Its result.
    pub status: Status,
}

/// The result of every check, in the order they ran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    outcomes: Vec<Outcome>,
}

impl Report {
    /// Records the result of `check`.
    pub(crate) fn record(&mut self, check: &'static str, status: Status) {
        self.outcomes.push(Outcome { check, status });
    }

    /// Returns every check and its result, in the order they ran.
    #[must_use]
    pub fn outcomes(&self) -> &[Outcome] {
        &self.outcomes
    }

    /// Returns the checks the adapter failed.
    pub fn failures(&self) -> impl Iterator<Item = &Outcome> {
        self.outcomes
            .iter()
            .filter(|outcome| matches!(outcome.status, Status::Failed(_)))
    }

    /// Returns whether the adapter failed no check.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures().next().is_none()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for outcome in &self.outcomes {
            match &outcome.status {
                Status::Passed => writeln!(formatter, "passed  {}", outcome.check)?,
                Status::NotApplicable(reason) => {
                    writeln!(formatter, "n/a     {}: {reason}", outcome.check)?;
                }
                Status::Failed(reason) => {
                    writeln!(formatter, "FAILED  {}: {reason}", outcome.check)?;
                }
            }
        }
        Ok(())
    }
}
