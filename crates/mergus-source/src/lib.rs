//! Extract from databases (ADBC) and files.
//!
//! In this phase the crate holds the checks of the source files that `mergus-project` has
//! read and validated against their schema (MG-REQ-0376). [`check_sources`] returns every
//! [`SourceProblem`] of a [`Configuration`](mergus_project::Configuration):
//!
//! - a `connect` or `live` source whose `engine` is missing or not in the allowlist of its
//!   mode, and a `copy` source that declares an `engine` (MG-REQ-0409);
//! - an incremental table without `key` (MG-REQ-0414);
//! - an incremental table without `deletes`, a warning (MG-REQ-0416).
//!
//! [`SourceKind`] and [`Mode`] are the kinds (MG-REQ-0401) and modes of a source.

#![forbid(unsafe_code)]

mod check;
mod kind;
mod problem;

pub use check::check_sources;
pub use kind::{Mode, SourceKind};
pub use problem::{Severity, SourceProblem};
