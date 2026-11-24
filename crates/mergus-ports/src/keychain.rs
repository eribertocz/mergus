//! The `Keychain` port: entries of the operating system keychain, by service and account.

use crate::PortError;

/// Entries of the operating system keychain (Keychain, Credential Manager, Secret Service),
/// each found by a service and an account.
///
/// Values cross this port as plain strings; callers hold them in a type that cannot be printed
/// as soon as they receive them (MG-REQ-1002). No error of this port carries a value.
pub trait Keychain: Send + Sync {
    /// Returns the value of the entry of `service` and `account`, or `None` when there is no
    /// such entry.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the keychain cannot be reached or the entry
    /// cannot be read.
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, PortError>;

    /// Writes `value` into the entry of `service` and `account`, creating or replacing it.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the keychain cannot be reached or the entry
    /// cannot be written.
    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), PortError>;

    /// Deletes the entry of `service` and `account`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when there is no such entry, and with
    /// [`PortError::Failed`] when the keychain cannot be reached or the entry cannot be
    /// deleted.
    fn delete(&self, service: &str, account: &str) -> Result<(), PortError>;
}
