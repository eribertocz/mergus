//! The error of secret resolution.

use mergus_ports::PortError;

use crate::{KEYCHAIN_SERVICE, SecretRef};

/// Why a secret reference could not be resolved. It names places, never a value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// Neither the environment variable [`SecretRef::variable`] nor the keychain entry of
    /// service [`KEYCHAIN_SERVICE`] and account [`SecretRef::name`] answered (MG-REQ-1001.3).
    #[error(
        "secret `{reference}` was not found: the environment variable `{}` is not set and the \
         keychain has no entry of service `{KEYCHAIN_SERVICE}` and account `{}`{}",
        reference.variable(),
        reference.name(),
        keychain.as_ref().map(|error| format!(" ({error})")).unwrap_or_default()
    )]
    NotFound {
        /// The reference that was resolved.
        reference: SecretRef,
        /// Why the keychain could not answer, when it failed rather than having no entry.
        keychain: Option<PortError>,
    },
}

impl SecretError {
    /// Returns the `MG-####` code of the error: `MG-1003`, a reference that does not
    /// resolve (MG-REQ-1004.3).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "MG-1003",
        }
    }

    /// Returns the reference that could not be resolved.
    #[must_use]
    pub fn reference(&self) -> &SecretRef {
        match self {
            Self::NotFound { reference, .. } => reference,
        }
    }
}
