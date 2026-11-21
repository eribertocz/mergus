//! Rust types generated from `schemas/`.
//!
//! Every contract Mergus owns has a JSON Schema in `schemas/` (MG-REQ-0201). Its Rust types
//! are generated into [`generated`] by `cargo xtask contracts-generate`, and
//! `cargo xtask contracts-generated` fails when they are out of date (MG-REQ-0202). A
//! document becomes a [`ValidDocument`], the only form that may be written or printed, by
//! passing [`validate`] (MG-REQ-0203). [`names`] holds every name Mergus uses from another
//! module (MG-REQ-0207).
//!
//! The crate is pure: it performs no I/O (MG-REQ-0004).

#![forbid(unsafe_code)]

mod emit;
pub mod generated;
pub mod names;
mod schema;
mod validate;

pub use emit::{Contract, ContractSchema, EmitError, ValidDocument, validate};
pub use schema::{KEYWORDS, Schema, SchemaError};
pub use validate::Violation;
