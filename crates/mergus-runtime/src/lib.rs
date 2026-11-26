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
//! Nothing is attached, read or written here.

#![forbid(unsafe_code)]

mod problem;
mod separate;
mod zone;

pub use problem::StorageProblem;
pub use separate::check_storage;
pub use zone::{Catalog, Clients, Profile, Zone, ZoneStorage, zone_storage};
