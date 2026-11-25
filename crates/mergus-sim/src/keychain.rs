//! A simulated keychain: entries in memory, set by the test.

use std::collections::BTreeMap;
use std::sync::Mutex;

use mergus_ports::{Keychain, PortError};

use crate::rng::lock;

/// Keychain entries in memory, keyed by service and account. It starts empty, or with the
/// entries given to [`SimKeychain::with_entries`].
#[derive(Debug, Default)]
pub struct SimKeychain {
    entries: Mutex<BTreeMap<(String, String), String>>,
}

impl SimKeychain {
    /// Returns a keychain with no entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a keychain holding `entries`, each a service, an account and a value.
    #[must_use]
    pub fn with_entries<I, S, A, V>(entries: I) -> Self
    where
        I: IntoIterator<Item = (S, A, V)>,
        S: Into<String>,
        A: Into<String>,
        V: Into<String>,
    {
        let entries = entries
            .into_iter()
            .map(|(service, account, value)| ((service.into(), account.into()), value.into()))
            .collect();
        Self {
            entries: Mutex::new(entries),
        }
    }
}

impl Keychain for SimKeychain {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, PortError> {
        Ok(lock(&self.entries).get(&key(service, account)).cloned())
    }

    fn set(&self, service: &str, account: &str, value: &str) -> Result<(), PortError> {
        lock(&self.entries).insert(key(service, account), value.to_owned());
        Ok(())
    }

    /// Fails with [`PortError::NotFound`] when there is no such entry.
    fn delete(&self, service: &str, account: &str) -> Result<(), PortError> {
        lock(&self.entries)
            .remove(&key(service, account))
            .map(|_| ())
            .ok_or_else(|| {
                PortError::NotFound(format!(
                    "the keychain entry of service `{service}` and account `{account}`"
                ))
            })
    }
}

/// The key of the entry of `service` and `account`.
fn key(service: &str, account: &str) -> (String, String) {
    (service.to_owned(), account.to_owned())
}
