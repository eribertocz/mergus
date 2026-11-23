//! Contract types generated from `schemas/` by `cargo xtask contracts-generate`.
//! Do not edit.

pub mod mergus_cli_v1;
pub mod mergus_events_v1;

/// Every contract that has a schema, sorted by name and version.
pub const CONTRACTS: &[crate::ContractSchema] = &[
    crate::ContractSchema::of::<mergus_cli_v1::MergusCli>(),
    crate::ContractSchema::of::<mergus_events_v1::MergusEvents>(),
];
