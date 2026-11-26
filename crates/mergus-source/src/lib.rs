//! Extract from databases (ADBC) and files.
//!
//! In this phase the crate holds the checks of the source files that `mergus-project` has
//! read and validated against their schema (MG-REQ-0376). [`check_sources`] returns every
//! [`SourceProblem`] of a [`Configuration`](mergus_project::Configuration):
//!
//! - a `connect` or `live` source whose `engine` is missing or not in the allowlist of its
//!   mode, a `database` source without `postgresql` or `duckdb` in `copy` mode, and another
//!   `copy` source that declares an `engine` (MG-REQ-0409);
//! - an incremental table without `key` (MG-REQ-0414);
//! - an incremental table without `deletes`, a warning (MG-REQ-0416).
//!
//! [`SourceKind`] and [`Mode`] are the kinds (MG-REQ-0401) and modes of a source.
//!
//! [`attach_duckdb_file`] and [`attach_postgres`] attach a database source read-only through
//! the `Database` port (MG-REQ-0421, MG-REQ-0196) and identify the engine that answers before
//! any data is read (MG-REQ-0410).

#![forbid(unsafe_code)]

mod check;
mod engine;
mod kind;
mod problem;

pub use check::check_sources;
pub use engine::{
    AttachedSource, ConnectError, DatabaseEngine, EngineIdentity, attach_duckdb_file,
    attach_postgres,
};
pub use kind::{Mode, SourceKind};
pub use problem::{Severity, SourceProblem};
