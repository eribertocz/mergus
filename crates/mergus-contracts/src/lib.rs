//! Rust types generated from `schemas/`.
//!
//! Every contract Mergus owns has a JSON Schema in `schemas/` (MG-REQ-0201). Its Rust types
//! are generated into [`generated`] by `cargo xtask contracts-generate`, and
//! `cargo xtask contracts-generated` fails when they are out of date (MG-REQ-0202). A
//! document becomes a [`ValidDocument`], the only form that may be written or printed, by
//! passing [`validate`] (MG-REQ-0203). [`names`] holds every name Mergus uses from another
//! module (MG-REQ-0207).
//!
//! [`read`] reads a document of any minor version of a known major version (MG-REQ-0206).
//! A document is hashed or signed only as its canonical JSON (RFC 8785):
//! [`ValidDocument::hash_input`] and [`canonical_json`] give it (MG-REQ-0208).
//!
//! The crate is pure: it performs no I/O (MG-REQ-0004).

#![forbid(unsafe_code)]

mod canonical;
mod emit;
pub mod generated;
pub mod names;
mod read;
mod schema;
mod validate;

pub use canonical::canonical_json;
pub use emit::{Contract, ContractSchema, EmitError, ValidDocument, validate};
pub use read::{ReadError, read};
pub use schema::{KEYWORDS, Schema, SchemaError};
pub use validate::Violation;
