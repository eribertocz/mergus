//! Reuse: macros, DuckDB macros only (MG-REQ-0641, MG-REQ-0642), fingerprinted by use
//! (MG-REQ-0644), and seeds, declared, typed, hashed (MG-REQ-0645) and small (MG-REQ-0646).

mod macros;
mod seeds;

pub use macros::{Macro, Macros, read_macros, sql_calls};
pub use seeds::{LARGE_SEED_BYTES, Seed, Seeds, read_seeds};
