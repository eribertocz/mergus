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
//!
//! It keeps the load registry and the retention of raw (MG-REQ-0513, MG-REQ-0514,
//! MG-REQ-0516, MG-REQ-0517):
//!
//! - [`load_raw`] writes a load into raw, then records it in `ops.loads` with
//!   [`record_load`], failed or not; [`load_raw_off`] records the extraction of a source with
//!   `raw = false` and writes nothing to raw; [`last_watermark`] is where the next load starts;
//! - [`expire_raw`] expires raw, as maintenance only, with [`raw_retention`]: the longest
//!   retention of the sources that keep raw, 30 days by default.

#![forbid(unsafe_code)]

mod problem;
mod raw_error;
mod raw_load;
mod raw_open;
mod raw_rows;
mod raw_write;
mod registry;
mod retention;
mod separate;
mod zone;

pub use problem::StorageProblem;
pub use raw_error::RawError;
pub use raw_load::{Watermarks, load_raw, load_raw_off};
pub use raw_open::{RAW_ALIAS, open_raw};
pub use raw_write::{RawLoad, write_raw};
pub use registry::{
    LOADS, LoadMode, LoadRecord, LoadResult, last_watermark, new_load_id, record_load,
};
pub use retention::{
    DEFAULT_RETENTION, expire_raw, keeps_raw, raw_off_sources, raw_retention, show_retention,
    source_retention,
};
pub use separate::check_storage;
pub use zone::{Catalog, Clients, Profile, Zone, ZoneStorage, zone_storage};
