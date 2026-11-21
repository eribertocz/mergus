//! The `sample` contract, version 1, generated from `crates/mergus-contracts/tests/fixtures/schemas/sample/v1/schema.json` by `cargo xtask contracts-generate`.
//! Do not edit.

use serde::{Deserialize, Serialize};

/// A contract that exercises every keyword the generator and the validator support.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// The name of the contract.
    pub contract: String,
    /// The version of the contract.
    pub contract_version: i64,
    /// The name of the sample.
    pub name: String,
    /// What the sample is.
    pub kind: SampleKind,
    /// Where the sample lives.
    pub location: SampleLocation,
    /// A share between zero and one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratio: Option<f64>,
    /// Whether everyone may read the sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    /// Free labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Who owns the sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<Owner>,
}

/// What the sample is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleKind {
    /// `table`.
    #[serde(rename = "table")]
    Table,
    /// `view`.
    #[serde(rename = "view")]
    View,
}

/// Where the sample lives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleLocation {
    /// The path of the sample.
    pub path: String,
    /// How deep the path is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i64>,
}

/// A team that owns something.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// The name of the team.
    pub team: String,
}

impl crate::Contract for Sample {
    const NAME: &'static str = "sample";
    const VERSION: u32 = 1;
    const SEMANTIC_VERSION: &'static str = "1.0.0";
    const SCHEMA: &'static str = include_str!("../schemas/sample/v1/schema.json");
}
