//! Conformance kit every storage adapter must pass.
//!
//! [`certify`] runs the kit's checks on a [`Harness`], which opens clients of one storage
//! adapter on fresh stores. The checks cover the six operations of the contract, crash
//! points between write and commit, and concurrency (MG-REQ-0289). The harness routes every
//! storage call through [`Faults`], so the kit crashes a client at any point without a real
//! crash.

#![forbid(unsafe_code)]

mod calls;
mod concurrency;
mod crash;
mod faults;
mod kit;
mod operations;
mod report;
mod rows;
mod versions;

pub use faults::Faults;
pub use kit::{Harness, certify};
pub use report::{Outcome, Report, Status};
