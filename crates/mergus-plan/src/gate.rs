//! The gates of stage 4: every gate is evaluated, and stage 5 needs a clearance that a
//! blocking gate refuses (MG-REQ-0720, MG-REQ-0721, MG-REQ-0722).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::PlanError;

/// The code of a required secret that does not resolve (MG-REQ-0721).
const MISSING_SECRET: &str = "MG-0701";

/// One gate of stage 4, in the order of the gate table of `run/gates.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Gate {
    /// A boundary loses information (MG-REQ-0634).
    TypePassport,
    /// A step reads something undeclared, writes, attaches or uses the network (MG-REQ-0308).
    SqlSafety,
    /// A published model uses undeclared non-deterministic operations (MG-REQ-0637).
    Determinism,
    /// The computed schema differs from `contracts/<model>.json` (MG-REQ-0802).
    OutputContract,
    /// Menura `validate` reports errors or new `W0*` warnings (MG-REQ-0232).
    SchemaGate,
    /// A guarantee rule of the storage profiles is violated (MG-REQ-0523).
    StorageRules,
    /// A required secret is missing, checked without reading its value (MG-REQ-0721).
    Credentials,
    /// A column labelled `pii` has no decision (MG-REQ-0146).
    Pii,
}

impl Gate {
    /// Every gate, in the order of the table.
    pub const ALL: [Self; 8] = [
        Self::TypePassport,
        Self::SqlSafety,
        Self::Determinism,
        Self::OutputContract,
        Self::SchemaGate,
        Self::StorageRules,
        Self::Credentials,
        Self::Pii,
    ];

    /// Returns the name of the gate, as errors and reports show it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::TypePassport => "type_passport",
            Self::SqlSafety => "sql_safety",
            Self::Determinism => "determinism",
            Self::OutputContract => "output_contract",
            Self::SchemaGate => "schema_gate",
            Self::StorageRules => "storage_rules",
            Self::Credentials => "credentials",
            Self::Pii => "pii",
        }
    }
}

/// Shows the name of the gate.
impl fmt::Display for Gate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// One error of a gate: the gate, its `MG-####` code, the model, the step and the fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateError {
    /// The gate that blocks.
    pub gate: Gate,
    /// The `MG-####` code of the error.
    pub code: &'static str,
    /// The model the error is about, when there is one.
    pub model: Option<String>,
    /// The step the error is about, when there is one.
    pub step: Option<String>,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub fix: String,
}

/// Shows ``<code>: gate `<gate>`, model `<model>`, step `<step>`: <message>; fix: <fix>``,
/// without the model or the step when there is none.
impl fmt::Display for GateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: gate `{}`", self.code, self.gate)?;
        if let Some(model) = &self.model {
            write!(formatter, ", model `{model}`")?;
        }
        if let Some(step) = &self.step {
            write!(formatter, ", step `{step}`")?;
        }
        write!(formatter, ": {}; fix: {}", self.message, self.fix)
    }
}

/// The result of one gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateResult {
    /// The gate was checked and found nothing.
    Passed,
    /// The gate was checked and blocks, with every error it found.
    Blocked(Vec<GateError>),
    /// The gate's checks belong to a later phase: it does not block.
    NotApplicable {
        /// The phase whose checks the gate waits for.
        phase: String,
    },
}

impl GateResult {
    /// Whether the gate blocks stage 5.
    #[must_use]
    pub fn blocks(&self) -> bool {
        matches!(self, Self::Blocked(_))
    }
}

/// Shows `passes`, `blocks: <n> errors`, or `not_applicable until phase <phase>`.
impl fmt::Display for GateResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Passed => formatter.write_str("passes"),
            Self::Blocked(errors) => write!(formatter, "blocks: {}", count(errors.len(), "error")),
            Self::NotApplicable { phase } => {
                write!(formatter, "not_applicable until phase {phase}")
            }
        }
    }
}

/// What the caller found for one gate other than [`Gate::Credentials`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateCheck {
    /// The gate was checked: every error it found, none when it passed.
    Checked(Vec<GateError>),
    /// The gate's checks belong to the later phase `phase`.
    NotApplicable {
        /// The phase whose checks the gate waits for.
        phase: String,
    },
}

/// Whether a secret reference resolves, as the caller's existence check found it. It holds
/// no value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Existence {
    /// The reference resolves.
    Resolves,
    /// The reference does not resolve.
    Missing,
}

/// A secret reference a step needs, as written: `secret:<name>`, never a value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RequiredSecret {
    /// The reference, as written.
    pub reference: String,
    /// The model whose step needs it.
    pub model: String,
    /// The step that needs it, when there is one.
    pub step: Option<String>,
}

/// What the caller gives to stage 4.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GateInput {
    /// What the caller found for each gate other than [`Gate::Credentials`], whose entry, if
    /// any, is not read.
    pub checks: BTreeMap<Gate, GateCheck>,
    /// Every secret reference the steps need.
    pub required: Vec<RequiredSecret>,
    /// Whether each required reference resolves, from the existence check that never reads a
    /// value (MG-REQ-0721).
    pub existence: BTreeMap<String, Existence>,
}

/// The result of every gate of stage 4, in the order of the table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateReport {
    results: Vec<(Gate, GateResult)>,
}

impl GateReport {
    /// Returns each gate with its result, in the order of [`Gate::ALL`].
    #[must_use]
    pub fn results(&self) -> &[(Gate, GateResult)] {
        &self.results
    }

    /// Whether any gate blocks.
    #[must_use]
    pub fn blocks(&self) -> bool {
        self.results.iter().any(|(_, result)| result.blocks())
    }

    /// Returns the clearance stage 5 needs when no gate blocks (MG-REQ-0720).
    ///
    /// # Errors
    ///
    /// Fails with [`Blocked`], holding the errors of every blocking gate, when any gate blocks
    /// (MG-REQ-0722).
    pub fn clear(&self) -> Result<Cleared, Blocked> {
        let errors: Vec<GateError> = self
            .results
            .iter()
            .filter_map(|(_, result)| match result {
                GateResult::Blocked(errors) => Some(errors.iter().cloned()),
                GateResult::Passed | GateResult::NotApplicable { .. } => None,
            })
            .flatten()
            .collect();
        if errors.is_empty() {
            Ok(Cleared { _private: () })
        } else {
            Err(Blocked { errors })
        }
    }
}

/// Shows one line per gate: `<gate>: <result>`.
impl fmt::Display for GateReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (gate, result) in &self.results {
            writeln!(formatter, "{gate}: {result}")?;
        }
        Ok(())
    }
}

/// The proof that every gate of stage 4 passed: stage 5 starts only with it (MG-REQ-0720).
#[derive(Debug, PartialEq, Eq)]
pub struct Cleared {
    _private: (),
}

/// Stage 4 blocked: the errors of every blocking gate, in the order of the table
/// (MG-REQ-0722).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub struct Blocked {
    errors: Vec<GateError>,
}

impl Blocked {
    /// Returns every error, in the order of the gates, then as each gate found them.
    #[must_use]
    pub fn errors(&self) -> &[GateError] {
        &self.errors
    }
}

/// Shows `stage 4 blocked: <n> errors in <m> gates`, then one error per line.
impl fmt::Display for Blocked {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gates: BTreeSet<Gate> = self.errors.iter().map(|error| error.gate).collect();
        write!(
            formatter,
            "stage 4 blocked: {} in {}",
            count(self.errors.len(), "error"),
            count(gates.len(), "gate")
        )?;
        for error in &self.errors {
            write!(formatter, "\n{error}")?;
        }
        Ok(())
    }
}

/// Evaluates every gate of stage 4 (MG-REQ-0720). The credentials gate blocks on each
/// required reference that `input.existence` marks missing, and sees no value (MG-REQ-0721).
/// Every error of every gate is kept (MG-REQ-0722).
///
/// # Errors
///
/// Fails with an internal [`PlanError`] when `input.checks` lacks a gate other than
/// [`Gate::Credentials`], or `input.existence` lacks a required reference.
pub fn evaluate(input: &GateInput) -> Result<GateReport, PlanError> {
    let mut results = Vec::with_capacity(Gate::ALL.len());
    for gate in Gate::ALL {
        let check = if gate == Gate::Credentials {
            GateCheck::Checked(missing_secrets(input)?)
        } else {
            input
                .checks
                .get(&gate)
                .cloned()
                .ok_or(PlanError::GateNotGiven(gate))?
        };
        let result = match check {
            GateCheck::NotApplicable { phase } => GateResult::NotApplicable { phase },
            GateCheck::Checked(errors) if errors.is_empty() => GateResult::Passed,
            GateCheck::Checked(errors) => GateResult::Blocked(errors),
        };
        results.push((gate, result));
    }
    Ok(GateReport { results })
}

/// One `MG-0701` error per required reference marked missing, in reference, model and step
/// order.
fn missing_secrets(input: &GateInput) -> Result<Vec<GateError>, PlanError> {
    let required: BTreeSet<&RequiredSecret> = input.required.iter().collect();
    let mut errors = Vec::new();
    for secret in required {
        let existence = input
            .existence
            .get(&secret.reference)
            .ok_or_else(|| PlanError::ExistenceNotGiven(secret.reference.clone()))?;
        if *existence == Existence::Missing {
            let name = secret
                .reference
                .strip_prefix("secret:")
                .unwrap_or(&secret.reference);
            errors.push(GateError {
                gate: Gate::Credentials,
                code: MISSING_SECRET,
                model: Some(secret.model.clone()),
                step: secret.step.clone(),
                message: format!("the secret `{}` does not resolve", secret.reference),
                fix: format!("run `mergus secret set {name}`"),
            });
        }
    }
    Ok(errors)
}

/// `<n> <noun>`, with an `s` unless `n` is 1.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}
