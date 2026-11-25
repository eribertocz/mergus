//! Storing a secret value in the keychain (MG-REQ-1007).

use mergus_ports::{Keychain, PortError};
use secrecy::{ExposeSecret, SecretString};

use crate::{KEYCHAIN_SERVICE, SecretRef};

/// Why a secret value was not stored. It names the reference, never the value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// The value is empty; nothing is stored (MG-REQ-1007.3).
    #[error("secret `{reference}` was not stored: the value is empty")]
    Empty {
        /// The reference whose value was given.
        reference: SecretRef,
    },
    /// The keychain cannot be written; nothing else is stored instead (MG-REQ-1007.4).
    #[error(
        "secret `{reference}` cannot be stored in the keychain under service \
         `{KEYCHAIN_SERVICE}` and account `{}`: {keychain}",
        reference.name()
    )]
    Unwritable {
        /// The reference whose value was given.
        reference: SecretRef,
        /// What the keychain said, which carries no value.
        keychain: PortError,
    },
}

impl StoreError {
    /// Returns the `MG-####` code of the error.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty { .. } => "MG-1001",
            Self::Unwritable { .. } => "MG-1002",
        }
    }
}

/// Stores `value` in `keychain` under the service [`KEYCHAIN_SERVICE`] and the account the
/// name of `reference`, creating or replacing the entry (MG-REQ-1007).
///
/// # Errors
///
/// Fails with [`StoreError::Empty`] when `value` is empty, and with
/// [`StoreError::Unwritable`] when the keychain cannot be written. Nothing is stored then.
pub fn store(
    reference: &SecretRef,
    value: &SecretString,
    keychain: &dyn Keychain,
) -> Result<(), StoreError> {
    let value = value.expose_secret();
    if value.is_empty() {
        return Err(StoreError::Empty {
            reference: reference.clone(),
        });
    }
    keychain
        .set(KEYCHAIN_SERVICE, reference.name(), value)
        .map_err(|keychain| StoreError::Unwritable {
            reference: reference.clone(),
            keychain,
        })
}
