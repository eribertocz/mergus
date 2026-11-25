//! The resolution chain of MG-REQ-1001.

use mergus_ports::{Environment, Keychain};
use secrecy::SecretString;

use crate::{SecretError, SecretRef};

/// The keychain service under which every secret is stored (MG-REQ-1001.2).
pub const KEYCHAIN_SERVICE: &str = "mergus";

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
    if let Some(value) = environment.var(&reference.variable()) {
        return Ok(SecretString::from(value));
    }
    let failure = match keychain.get(KEYCHAIN_SERVICE, reference.name()) {
        Ok(Some(value)) => return Ok(SecretString::from(value)),
        Ok(None) => None,
        Err(error) => Some(error),
    };
    Err(SecretError::NotFound {
        reference: reference.clone(),
        keychain: failure,
    })
}
