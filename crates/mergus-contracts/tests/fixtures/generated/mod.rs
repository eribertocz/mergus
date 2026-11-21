//! Contract types generated from `crates/mergus-contracts/tests/fixtures/schemas/` by `cargo xtask contracts-generate`.
//! Do not edit.

pub mod sample_v1;

/// Every contract that has a schema, sorted by name and version.
pub const CONTRACTS: &[crate::ContractSchema] = &[crate::ContractSchema::of::<sample_v1::Sample>()];
