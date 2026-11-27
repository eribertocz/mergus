//! Resolve, order and fingerprint the work; produce the plan.
//!
//! [`plan`] takes the models of the read project, as [`PlanModel`]s, and what only the caller
//! knows, as a [`PlanRequest`]: the selected models, the fingerprints of the available results,
//! the versions of raw inputs and engines, the environment and `max_parallel` (MG-REQ-0744.1).
//! It returns the [`Plan`], a `mergus_plan` document valid against its schema and identified
//! by its hash (MG-REQ-0714, MG-REQ-0742, MG-REQ-0743).
//!
//! Each model gets a [`fingerprint`] (MG-REQ-0711); a selected model whose fingerprint is
//! available is skipped (MG-REQ-0712, MG-REQ-0744.2); steps of the same depth are cut into
//! parallel groups of at most `max_parallel` (MG-REQ-0716, MG-REQ-0744.3); stale models left
//! out of the selection are listed with the selector that includes them (MG-REQ-0717,
//! MG-REQ-0745). Only ordered collections are used, so the same inputs give the same bytes
//! (MG-REQ-0005, MG-REQ-0715).
//!
//! [`evaluate`] runs every [`Gate`] of stage 4 and returns a [`GateReport`]; stage 5 needs the
//! [`Cleared`] that [`GateReport::clear`] refuses, with every error of every gate, when any
//! gate blocks (MG-REQ-0720, MG-REQ-0722). The credentials gate asks a [`SecretCheck`] given by
//! the caller whether each [`RequiredSecret`] resolves, and never sees a value (MG-REQ-0721).
//!
//! The crate is pure: it performs no I/O (MG-REQ-0004).

#![forbid(unsafe_code)]

mod build;
mod error;
mod fingerprint;
mod gate;
mod graph;
mod input;

pub use build::{Plan, plan};
pub use error::PlanError;
pub use fingerprint::{FingerprintInputs, fingerprint};
pub use gate::{
    Blocked, Cleared, Gate, GateError, GateInput, GateReport, GateResult, RequiredSecret,
    SecretCheck, evaluate,
};
pub use input::{DEFAULT_MAX_PARALLEL, PlanModel, PlanRequest, effective_config, lf_text};
