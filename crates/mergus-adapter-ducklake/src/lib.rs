//! DuckLake storage adapter.
//!
//! [`DuckLakeAdapter`] implements the `StorageAdapter` trait of `mergus-adapter` over one
//! DuckLake catalog, reached only through the `Database` port of `mergus-ports`
//! (MG-REQ-0194). Written rows stay outside the catalog until `commit` (MG-REQ-0286), and
//! `expire` never deletes a live snapshot or one inside the rollback window (MG-REQ-0288).

#![forbid(unsafe_code)]

mod adapter;
mod error;
mod snapshots;
mod sql;

pub use adapter::DuckLakeAdapter;
pub use error::DuckLakeError;
