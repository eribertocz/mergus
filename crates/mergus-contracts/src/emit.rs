//! Validation before emitting (MG-REQ-0203).
//!
//! A contract document becomes a [`ValidDocument`] only by passing [`validate`]. Code that
//! writes or prints a contract document takes a [`ValidDocument`], so a document that fails
//! validation cannot be emitted.

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::schema::{Schema, SchemaError};
use crate::validate::Violation;

/// A contract type generated from its schema in `schemas/` (MG-REQ-0202).
pub trait Contract: Serialize {
    /// The name of the contract: the name of its directory in `schemas/`.
    const NAME: &'static str;
    /// The version of its schema: the `v<version>` directory in `schemas/<name>/`.
    const VERSION: u32;
    /// The text of its JSON Schema.
    const SCHEMA: &'static str;
}

/// The schema of one contract, as the generated registry lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContractSchema {
    /// The name of the contract.
    pub name: &'static str,
    /// The version of its schema.
    pub version: u32,
    /// The text of its JSON Schema.
    pub schema: &'static str,
}

impl ContractSchema {
    /// Returns the schema of the contract `T`.
    pub const fn of<T: Contract>() -> Self {
        Self {
            name: T::NAME,
            version: T::VERSION,
            schema: T::SCHEMA,
        }
    }
}

/// A contract document that matches its schema, ready to be written or printed.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidDocument {
    contract: &'static str,
    value: Value,
}

impl ValidDocument {
    /// The name of the contract of the document.
    pub fn contract(&self) -> &'static str {
        self.contract
    }

    /// The document as a JSON value.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The bytes to hash or sign: the canonical JSON of the document (MG-REQ-0208).
    pub fn hash_input(&self) -> Vec<u8> {
        crate::canonical::canonical_json(&self.value).into_bytes()
    }

    /// Takes the document as a JSON value.
    pub fn into_value(self) -> Value {
        self.value
    }
}

/// Why a contract document cannot be emitted. Every variant is an internal error: the
/// document Mergus built does not match the contract it claims to follow, and the run ends.
#[derive(Debug, Error)]
pub enum EmitError {
    /// The document cannot be serialized to JSON.
    #[error("internal error: the `{contract}` document cannot be serialized: {message}")]
    Serialize {
        /// The name of the contract.
        contract: &'static str,
        /// The serializer's message.
        message: String,
    },
    /// The schema of the contract cannot be used.
    #[error("internal error: the schema of `{contract}` cannot be used: {source}")]
    Schema {
        /// The name of the contract.
        contract: &'static str,
        /// What is wrong with the schema.
        source: SchemaError,
    },
    /// The document does not match the schema of its contract.
    #[error("internal error: the `{contract}` document does not match its schema: {}", list(.violations))]
    Invalid {
        /// The name of the contract.
        contract: &'static str,
        /// Every way the document breaks the schema.
        violations: Vec<Violation>,
    },
}

fn list(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Serializes `document` and validates it against the schema of its contract.
///
/// # Errors
///
/// Fails with [`EmitError::Serialize`] if the document cannot be serialized, with
/// [`EmitError::Schema`] if the schema of the contract cannot be used, and with
/// [`EmitError::Invalid`] if the document does not match it.
pub fn validate<T: Contract>(document: &T) -> Result<ValidDocument, EmitError> {
    let contract = T::NAME;
    let value = serde_json::to_value(document).map_err(|error| EmitError::Serialize {
        contract,
        message: error.to_string(),
    })?;
    let schema =
        Schema::parse(T::SCHEMA).map_err(|source| EmitError::Schema { contract, source })?;
    let violations = schema.validate(&value);
    if violations.is_empty() {
        Ok(ValidDocument { contract, value })
    } else {
        Err(EmitError::Invalid {
            contract,
            violations,
        })
    }
}
