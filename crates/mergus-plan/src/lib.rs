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
//! The crate is pure: it performs no I/O (MG-REQ-0004).

#![forbid(unsafe_code)]

mod build;
mod error;
mod fingerprint;
mod graph;
mod input;

pub use build::{Plan, plan};
pub use error::PlanError;
pub use fingerprint::{FingerprintInputs, fingerprint};
pub use input::{DEFAULT_MAX_PARALLEL, PlanModel, PlanRequest, effective_config, lf_text};
