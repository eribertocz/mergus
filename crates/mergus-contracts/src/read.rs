//! Reading a contract document Mergus owns (MG-REQ-0206).
//!
//! A minor version only adds optional fields (MG-REQ-0204), and `contract-compatibility`
//! refuses any other change that keeps the major version. So the latest schema of a major
//! version accepts every document of its older minor versions, and [`read`] checks documents
//! against that schema.

use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::emit::Contract;
use crate::schema::{Schema, SchemaError};
use crate::validate::Violation;

/// Why a document cannot be read as a contract.
#[derive(Debug, Error)]
pub enum ReadError {
    /// The schema of the contract cannot be used.
    #[error("the schema of `{contract}` cannot be used: {source}")]
    Schema {
        /// The name of the contract.
        contract: &'static str,
        /// What is wrong with the schema.
        source: SchemaError,
    },
    /// The document does not match the schema of the contract: it belongs to another
    /// contract or major version, or to a newer minor version than this one knows.
    #[error("the document does not match the schema of `{contract}`: {}", list(.violations))]
    Invalid {
        /// The name of the contract.
        contract: &'static str,
        /// Every way the document breaks the schema.
        violations: Vec<Violation>,
    },
    /// The document matches the schema but not the generated type.
    #[error("the document cannot be read as `{contract}`: {message}")]
    Deserialize {
        /// The name of the contract.
        contract: &'static str,
        /// The deserializer's message.
        message: String,
    },
}

fn list(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Reads `document` as the contract `T`: any minor version of its major version up to the
/// latest this build knows.
///
/// # Errors
///
/// Fails with [`ReadError::Schema`] if the schema of the contract cannot be used, with
/// [`ReadError::Invalid`] if the document does not match it, and with
/// [`ReadError::Deserialize`] if it cannot be turned into `T`.
pub fn read<T: Contract + DeserializeOwned>(document: &Value) -> Result<T, ReadError> {
    let contract = T::NAME;
    let schema =
        Schema::parse(T::SCHEMA).map_err(|source| ReadError::Schema { contract, source })?;
    let violations = schema.validate(document);
    if !violations.is_empty() {
        return Err(ReadError::Invalid {
            contract,
            violations,
        });
    }
    T::deserialize(document).map_err(|error| ReadError::Deserialize {
        contract,
        message: error.to_string(),
    })
}
