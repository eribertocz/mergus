//! The resolution chain of MG-REQ-1001, and which link of it answers (MG-REQ-1004).

use std::fmt;

use mergus_ports::{Environment, Keychain};
use secrecy::SecretString;

use crate::{SecretError, SecretRef};

/// The keychain service under which every secret is stored (MG-REQ-1001.2).
pub const KEYCHAIN_SERVICE: &str = "mergus";

/// The link of the chain that answered for a reference. It names a place, never a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretOrigin {
    /// The environment variable [`SecretRef::variable`] is set (MG-REQ-1001.1).
    Environment {
        /// The name of the variable.
        variable: String,
    },
    /// The keychain has an entry of service [`KEYCHAIN_SERVICE`] and account the name
    /// (MG-REQ-1001.2).
    Keychain {
        /// The account of the entry: the name of the secret.
        account: String,
    },
}

/// Shows the origin as `mergus doctor` prints it (MG-REQ-1004.1): `environment
/// MERGUS_SECRET_<NAME>` or `keychain mergus/<name>`.
impl fmt::Display for SecretOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment { variable } => write!(formatter, "environment {variable}"),
            Self::Keychain { account } => {
                write!(formatter, "keychain {KEYCHAIN_SERVICE}/{account}")
            }
        }
    }
}

/// Resolves `reference` by the first place that answers (MG-REQ-1001):
///
/// 1. the environment variable [`SecretRef::variable`], even when it is set to an empty value;
/// 2. the entry of `keychain` of service [`KEYCHAIN_SERVICE`] and account the name.
///
/// No other place is looked at: no `.env` file is read (MG-REQ-1005).
///
/// # Errors
///
/// Fails with [`SecretError::NotFound`] when neither answers, naming the reference and both
/// places (MG-REQ-1001.3). A keychain that fails does not answer; its error is kept.
pub fn resolve(
    reference: &SecretRef,
    environment: &dyn Environment,
    keychain: &dyn Keychain,
) -> Result<SecretString, SecretError> {
    chain(reference, environment, keychain).map(|(_, value)| value)
}

/// Returns which link of the chain of [`resolve`] answers for `reference`, and not the value,
/// which is dropped at once: for `mergus doctor` (MG-REQ-1004).
///
/// # Errors
///
/// Fails exactly as [`resolve`] does, when neither place answers.
pub fn origin(
    reference: &SecretRef,
    environment: &dyn Environment,
    keychain: &dyn Keychain,
) -> Result<SecretOrigin, SecretError> {
    chain(reference, environment, keychain).map(|(origin, _)| origin)
}

/// The chain of MG-REQ-1001: the value and the link that gave it.
fn chain(
    reference: &SecretRef,
    environment: &dyn Environment,
    keychain: &dyn Keychain,
) -> Result<(SecretOrigin, SecretString), SecretError> {
    let variable = reference.variable();
    if let Some(value) = environment.var(&variable) {
        return Ok((
            SecretOrigin::Environment { variable },
            SecretString::from(value),
        ));
    }
    let failure = match keychain.get(KEYCHAIN_SERVICE, reference.name()) {
        Ok(Some(value)) => {
            let account = reference.name().to_owned();
            return Ok((
                SecretOrigin::Keychain { account },
                SecretString::from(value),
            ));
        }
        Ok(None) => None,
        Err(error) => Some(error),
    };
    Err(SecretError::NotFound {
        reference: reference.clone(),
        keychain: failure,
    })
}
