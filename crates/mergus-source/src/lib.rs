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
//!
//! [`declared_tables`] lists the tables a source declares, the only ones extracted
//! (MG-REQ-0402), and [`DeclaredTable::raw`] names the raw tables each lands in
//! (MG-REQ-0403). [`read_table`] reads one table in chunks, with the caller's projection and
//! incremental mark pushed down (MG-REQ-0404), completely when it has no `incremental`
//! (MG-REQ-0411), and by keyset pagination when chunks are asked (MG-REQ-0417);
//! [`extract_source`] reads every declared table of a source.
//!
//! [`copied_tables`] are the tables extracted into raw: none of a `connect` or `live` source,
//! whose extraction reads nothing (MG-REQ-0405). [`source_columns`] and [`breaking_changes`]
//! find a dropped or renamed column or a type change since the last load, a [`SchemaChange`]
//! that stops the table (MG-REQ-0407). [`extract_source_timed`] reads a source with both, and
//! records the start and the end of each table's read and their spread (MG-REQ-0419).
//!
//! [`IncrementalReader`] reads an incremental table from `watermark − overlap`
//! (MG-REQ-0412), chunk by chunk, and moves past a chunk only once the caller committed it
//! (MG-REQ-0413, MG-REQ-0418); [`IncrementalReader::watermark`] is the watermark of committed rows. It detects
//! deletes as [`load_options`] declare them (MG-REQ-0415) and reports very late rows,
//! [`LateRows`] (MG-REQ-0420).
//!
//! [`SourceCare`] holds the care options of a source (`extract/source-care.md`): every
//! extraction query goes through the `replica` when it is set
//! ([`SourceCare::extraction_reference`], MG-REQ-0426), and `load` and `max_connections` set
//! the [`Budget`] of [`Load::table`] (MG-REQ-0427). [`read_with_brake`] reads a table at the
//! pace of a [`Brake`] and stops after three consecutive failures with a [`SourceIncident`]
//! (MG-REQ-0428). [`wait_for_window`] holds a full load or a backfill until the [`Window`]
//! opens (MG-REQ-0430). [`check_indexes`] returns an [`IndexWarning`] for each key or
//! incremental column without an index (MG-REQ-0431), and [`source_impact`] the
//! [`TableImpact`] `mergus plan` shows (MG-REQ-0432). A source that cannot be reached or read
//! is an [`InspectError`] for `doctor` and an [`ImpactUnknown`] warning for `plan`.

#![forbid(unsafe_code)]

mod brake;
mod care;
mod check;
mod compare;
mod copied;
mod engine;
mod impact;
mod incremental;
mod incremental_error;
mod index;
mod inspect;
mod kind;
mod marks;
mod options;
mod problem;
mod read;
mod schema_change;
mod snapshot;
mod table;
mod temporal;
mod window;

pub use brake::{AfterFailure, Brake, SourceIncident, read_with_brake};
pub use care::{Budget, Load, SourceCare};
pub use check::check_sources;
pub use compare::Comparison;
pub use copied::copied_tables;
pub use engine::{
    AttachedSource, ConnectError, DatabaseEngine, EngineIdentity, attach_duckdb_file,
    attach_postgres,
};
pub use impact::{PG_ESTIMATES, SCHEMA_ESTIMATES, TableImpact, source_impact};
pub use incremental::{Chunk, IncrementalReader, Start};
pub use incremental_error::{ExtractError, LateRows};
pub use index::{IndexWarning, PG_INDEXES, SCHEMA_INDEXES, Unindexed, check_indexes};
pub use inspect::{ImpactUnknown, InspectError};
pub use kind::{Mode, SourceKind};
pub use options::{DEFAULT_OVERLAP, DeleteCheck, DeletePolicy, LoadOptions, load_options};
pub use problem::{Severity, SourceProblem};
pub use read::{Projection, ReadOptions, TableExtraction, TableReader, extract_source, read_table};
pub use schema_change::{
    ColumnChange, SchemaChange, SourceColumn, breaking_changes, source_columns,
};
pub use snapshot::{SourceExtraction, TableRead, extract_source_timed};
pub use table::{DeclaredTable, RawTables, declared_tables};
pub use temporal::Temporal;
pub use window::{LoadKind, Window, wait_for_window};
