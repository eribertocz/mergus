//! The simulation runner: seeded runs, step-by-step replay and shrinking (MG-REQ-0162).

use std::fmt;

use crate::SimRandom;
use crate::rng::SimRng;

/// A broken invariant: the reason a step failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    message: String,
}

impl Violation {
    /// Returns a violation described by `message`.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns what was violated.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

/// What the simulator drives: steps drawn from a seed, applied one at a time to a fresh
/// world that checks its invariants after each one.
pub trait Scenario {
    /// One action of a run.
    type Step: Clone + fmt::Debug + PartialEq;
    /// The world a run acts on: the code under simulation and its simulated ports.
    type State;

    /// Draws the steps of a run from `random`.
    fn generate(&self, random: &dyn mergus_ports::Random) -> Vec<Self::Step>;

    /// Builds a fresh world; its simulated ports take their seeds from `seed`.
    fn start(&self, seed: u64) -> Self::State;

    /// Applies `step` to `state`.
    ///
    /// # Errors
    ///
    /// Returns the violation when an invariant no longer holds after the step.
    fn apply(&self, state: &mut Self::State, step: &Self::Step) -> Result<(), Violation>;
}

/// One step of a replay and its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepRecord<Step> {
    /// The position of the step in the run, from 0.
    pub index: usize,
    /// The step applied.
    pub step: Step,
    /// Whether the invariants held after it.
    pub result: Result<(), Violation>,
}

/// A failing case: the seed and the steps up to the one that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure<Step> {
    /// The seed of the world.
    pub seed: u64,
    /// The steps applied; the last one broke an invariant.
    pub steps: Vec<Step>,
    /// What the last step violated.
    pub violation: Violation,
}

/// The result of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome<Step> {
    /// Every step kept the invariants.
    Passed {
        /// How many steps were applied.
        steps: usize,
    },
    /// A step broke an invariant.
    Failed(Failure<Step>),
}

/// Runs a scenario from seeds, replays runs and shrinks failures.
#[derive(Debug)]
pub struct Simulator<S> {
    scenario: S,
}

impl<S: Scenario> Simulator<S> {
    /// Returns a simulator of `scenario`.
    #[must_use]
    pub const fn new(scenario: S) -> Self {
        Self { scenario }
    }

    /// Returns the steps that `seed` draws. Their generator is derived from the seed, so it
    /// does not repeat the numbers of ports seeded with `seed` itself.
    #[must_use]
    pub fn steps(&self, seed: u64) -> Vec<S::Step> {
        let random = SimRandom::new(SimRng::new(seed).next_u64());
        self.scenario.generate(&random)
    }

    /// Runs the steps that `seed` draws on the world of `seed`.
    #[must_use]
    pub fn run(&self, seed: u64) -> Outcome<S::Step> {
        self.run_steps(seed, self.steps(seed))
    }

    /// Runs `steps` on the world of `seed` and stops at the first violation.
    #[must_use]
    pub fn run_steps(&self, seed: u64, steps: Vec<S::Step>) -> Outcome<S::Step> {
        let mut applied = Vec::with_capacity(steps.len());
        for record in self.replay(seed, steps) {
            applied.push(record.step);
            if let Err(violation) = record.result {
                return Outcome::Failed(Failure {
                    seed,
                    steps: applied,
                    violation,
                });
            }
        }
        Outcome::Passed {
            steps: applied.len(),
        }
    }

    /// Replays `steps` on the world of `seed`, one step per item. It ends after the step
    /// that breaks an invariant.
    #[must_use]
    pub fn replay(&self, seed: u64, steps: Vec<S::Step>) -> Replay<'_, S> {
        Replay {
            scenario: &self.scenario,
            state: self.scenario.start(seed),
            steps: steps.into_iter().enumerate(),
            failed: false,
        }
    }

    /// Returns a smallest case found from `failure` that still fails on the same seed: no
    /// single step can be removed from it without the run passing.
    #[must_use]
    pub fn shrink(&self, failure: &Failure<S::Step>) -> Failure<S::Step> {
        let mut smallest = failure.clone();
        let mut chunk = (smallest.steps.len() / 2).max(1);
        loop {
            let removed = self.remove_chunks(&mut smallest, chunk);
            if chunk == 1 && !removed {
                return smallest;
            }
            if !removed {
                chunk /= 2;
            }
        }
    }

    /// Removes every run of `chunk` steps whose removal keeps the case failing. Returns
    /// whether it removed any.
    fn remove_chunks(&self, smallest: &mut Failure<S::Step>, chunk: usize) -> bool {
        let mut removed = false;
        let mut start = 0;
        while start < smallest.steps.len() {
            let end = smallest.steps.len().min(start.saturating_add(chunk));
            let mut candidate = smallest.steps[..start].to_vec();
            candidate.extend_from_slice(&smallest.steps[end..]);
            match self.run_steps(smallest.seed, candidate) {
                Outcome::Failed(smaller) => {
                    *smallest = smaller;
                    removed = true;
                }
                Outcome::Passed { .. } => start = end,
            }
        }
        removed
    }
}

/// A run replayed step by step; each item is one applied step.
pub struct Replay<'a, S: Scenario> {
    scenario: &'a S,
    state: S::State,
    steps: std::iter::Enumerate<std::vec::IntoIter<S::Step>>,
    failed: bool,
}

impl<S: Scenario> Iterator for Replay<'_, S> {
    type Item = StepRecord<S::Step>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let (index, step) = self.steps.next()?;
        let result = self.scenario.apply(&mut self.state, &step);
        self.failed = result.is_err();
        Some(StepRecord {
            index,
            step,
            result,
        })
    }
}
