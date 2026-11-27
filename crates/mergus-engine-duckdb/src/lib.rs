//! Execute DuckDB steps.
//!
//! [`run_segment`] runs one [`Segment`] in its own DuckDB session through the `Database`
//! port (MG-REQ-0194): it executes the project's macro texts first (MG-REQ-0643), opens each
//! input [`Table`] in the types its schema declares or fails with `MG-0101` (MG-REQ-0180), runs the
//! step's SQL with its bound parameters and leaves the rows at its output [`Boundary`] with the
//! [`Transport`] the plan chose: Arrow IPC in the run's work directory, or Parquet in the work
//! zone read lazily (MG-REQ-0633). [`table_hash`] hashes a table independently of the order
//! of its rows (MG-REQ-0638).

#![forbid(unsafe_code)]

mod checks;
mod error;
mod exact;
mod hash;
mod history;
mod history_apply;
mod history_sql;
mod output;
mod segment;
mod transport;

pub use error::EngineError;
pub use hash::table_hash;
pub use history::{
    AS_OF, ChangeDetection, Deletes, History, HistoryInput, RebuildHistory, rebuild_history,
};
pub use history_apply::apply_history;
pub use segment::{Segment, Table, run_segment};
pub use transport::{Boundary, MEMORY_BUDGET, Transport};
