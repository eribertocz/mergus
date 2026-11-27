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
//! gate blocks (MG-REQ-0720, MG-REQ-0722). The credentials gate reads, for each
//! [`RequiredSecret`], the [`Existence`] the caller found, and never sees a value
//! (MG-REQ-0721).
//!
//! Each model with [`ModelStep`]s gets its segments: consecutive steps of one [`Engine`]
//! fused (MG-REQ-0631); [`segment`] also finds the columns that cross between segments, and
//! only those (MG-REQ-0632). Each boundary crosses as Arrow up to 1 GiB of estimated data,
//! as Parquet above or without an estimate (MG-REQ-0633), and each Polars segment runs at the
//! highest level available (MG-REQ-0635).
//!
//! [`type_passport`] and [`determinism`] return the errors of the `Type passport` and
//! `Determinism` gates (MG-REQ-0634, MG-REQ-0637), for the caller to give to [`evaluate`];
//! [`detect_sql`] finds the non-deterministic operations of a SQL text on its parsed tree.
//!
//! The crate is pure: it performs no I/O (MG-REQ-0004).

#![forbid(unsafe_code)]

mod build;
mod determinism;
mod error;
mod execution;
mod fingerprint;
mod gate;
mod graph;
mod input;
mod passport;
mod segment;
mod types;

pub use build::{Plan, plan};
pub use determinism::{Detected, Operation, detect_sql, determinism};
pub use error::PlanError;
pub use fingerprint::{FingerprintInputs, fingerprint};
pub use gate::{
    Blocked, Cleared, Existence, Gate, GateCheck, GateError, GateInput, GateReport, GateResult,
    RequiredSecret, evaluate,
};
pub use input::{DEFAULT_MAX_PARALLEL, PlanModel, PlanRequest, effective_config, lf_text};
pub use passport::type_passport;
pub use segment::{
    ColumnRead, Crossing, CrossingColumn, Engine, Fused, ModelStep, OutputColumn, Segments, segment,
};
