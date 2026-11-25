//! Storage adapter contract (public API for third parties).
//!
//! An adapter connects Mergus to one storage format through the [`StorageAdapter`] trait and
//! its six operations (MG-REQ-0285): [`capabilities`](StorageAdapter::capabilities),
//! [`write`](StorageAdapter::write), [`commit`](StorageAdapter::commit),
//! [`pin_recipe`](StorageAdapter::pin_recipe), [`read_at`](StorageAdapter::read_at) and
//! [`expire`](StorageAdapter::expire). Data crosses the contract as Arrow record batches.
//!
//! [`check_capabilities`] compares what a run needs with what the adapter declares and refuses
//! any undeclared write mode or feature before data is read (MG-REQ-0287).

#![forbid(unsafe_code)]

mod adapter;
mod capabilities;
mod check;
mod error;
mod version;

pub use adapter::{BatchStream, StorageAdapter};
pub use capabilities::{Capabilities, Versioning, WriteMode};
pub use check::{Requirement, check_capabilities};
pub use error::{AdapterError, CapabilityError};
pub use version::{PinRecipe, Retention, VersionRef};
