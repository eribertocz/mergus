//! The keychain of the operating system, through `keyring`.

use keyring::{Entry, Error};

use crate::{Keychain, PortError};

/// The keychain of the operating system: Keychain Services on macOS, Credential Manager on
/// Windows and the Secret Service elsewhere.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealKeychain;

impl Keychain for RealKeychain {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, PortError> {
        match entry(service, account)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(failed("read", service, account, &error)),
        }
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), PortError> {
        entry(service, account)?
            .set_password(value)
            .map_err(|error| failed("write", service, account, &error))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), PortError> {
        match entry(service, account)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(Error::NoEntry) => Err(PortError::NotFound(shown(service, account))),
            Err(error) => Err(failed("delete", service, account, &error)),
        }
    }
}

/// Returns the entry of `service` and `account`.
fn entry(service: &str, account: &str) -> Result<Entry, PortError> {
    Entry::new(service, account).map_err(|error| failed("open", service, account, &error))
}

/// Shows an entry by its service and account.
fn shown(service: &str, account: &str) -> String {
    format!("the keychain entry of service `{service}` and account `{account}`")
}

/// Converts a `keyring` error into a [`PortError`]. Only the kind of the error is kept: a
/// `keyring` error can carry the bytes of an entry, which must never be shown (MG-REQ-1002).
fn failed(action: &str, service: &str, account: &str, error: &Error) -> PortError {
    let reason = match error {
        Error::NoDefaultStore | Error::NoStorageAccess(_) | Error::PlatformFailure(_) => {
            "the keychain cannot be reached"
        }
        Error::BadEncoding(_) | Error::BadDataFormat(..) => "the value is not valid text",
        _ => "the keychain refused the operation",
    };
    PortError::Failed(format!(
        "cannot {action} {}: {reason}",
        shown(service, account)
    ))
}

#[cfg(test)]
mod tests {
    use super::RealKeychain;
    use crate::{Keychain, PortError};

    /// A machine without a keychain fails with `Failed`; one with a keychain has no entry for
    /// this account. Either way nothing is written.
    #[test]
    fn real_keychain_reports_a_missing_entry_or_an_unreachable_keychain() {
        let keychain = RealKeychain;
        let account = "mergus_ports_test_missing_entry";
        match keychain.get("mergus-ports-test", account) {
            Ok(value) => assert_eq!(value, None),
            Err(error) => assert!(matches!(error, PortError::Failed(_)), "{error:?}"),
        }
        assert!(keychain.delete("mergus-ports-test", account).is_err());
    }
}
