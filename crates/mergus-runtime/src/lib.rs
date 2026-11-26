//! Run cycle and scheduling; wires ports to crates.
//!
//! In this phase the crate turns the storage of each zone of `project.toml` (`[raw]`, `[gold]`
//! and `[ops]`), as `mergus-project` has validated it, into the settings DuckLake needs:
//!
//! - [`zone_storage`] gives each zone its [`ZoneStorage`]: the [`Catalog`] and its attach
//!   string, the data path and the [`Clients`] of its [`Profile`], from `location` alone, or
//!   from the `catalog` and `data_path` that override them (MG-REQ-0521);
//! - [`check_storage`] returns a [`StorageProblem`] when raw and gold would sit behind the
//!   same credentials (MG-REQ-0501).
//!
//! It writes raw (MG-REQ-0511, MG-REQ-0512, MG-REQ-0515, MG-REQ-0518):
//!
//! - [`open_raw`] opens the DuckLake adapter of the raw zone with its catalog attached
//!   encrypted, and refuses an unencrypted catalog or a local one inside the project;
//! - [`write_raw`] writes one [`RawLoad`] into `raw.<source>__<table>__changes` and
//!   `raw.<source>__<table>` in one commit, with the source's native types and the metadata
//!   columns.

#![forbid(unsafe_code)]

mod problem;
mod raw_error;
mod raw_open;
mod raw_rows;
mod raw_write;
mod separate;
mod zone;

pub use problem::StorageProblem;
pub use raw_error::RawError;
pub use raw_open::{RAW_ALIAS, open_raw};
pub use raw_write::{RawLoad, write_raw};
pub use separate::check_storage;
pub use zone::{Catalog, Clients, Profile, Zone, ZoneStorage, zone_storage};
