//! MG-REQ-1007: storing a secret value in the keychain.

use mergus_ports::{Keychain, PortError};
use mergus_security::{ExposeSecret, SecretRef, SecretString, StoreError, check, store};
use mergus_sim::SimKeychain;

/// A keychain that cannot be written.
struct Locked;

impl Keychain for Locked {
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), PortError> {
        Err(PortError::Failed("the keychain is locked".to_owned()))
    }

    fn delete(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(PortError::Failed("the keychain is locked".to_owned()))
    }
}

#[test]
fn mg_req_1007_store_writes_service_mergus_account_name() {
    let keychain = SimKeychain::new();
    store(
        &SecretRef::from_name("crm").unwrap(),
        &SecretString::from("pw\nline"),
        &keychain,
    )
    .unwrap();
    assert_eq!(
        keychain.get("mergus", "crm").unwrap().as_deref(),
        Some("pw\nline")
    );
    let value = SecretString::from("pw");
    assert_eq!(value.expose_secret(), "pw");
}

#[test]
fn mg_req_1007_store_refuses_empty_value() {
    let keychain = SimKeychain::new();
    let error = store(
        &SecretRef::from_name("crm").unwrap(),
        &SecretString::from(""),
        &keychain,
    )
    .unwrap_err();
    assert_eq!(error.code(), "MG-1001");
    assert!(matches!(error, StoreError::Empty { .. }));
    assert_eq!(keychain.get("mergus", "crm").unwrap(), None);
}

#[test]
fn mg_req_1007_store_reports_unwritable_keychain() {
    let error = store(
        &SecretRef::from_name("crm").unwrap(),
        &SecretString::from("hunter2"),
        &Locked,
    )
    .unwrap_err();
    assert_eq!(error.code(), "MG-1002");
    let message = error.to_string();
    assert!(message.contains("secret:crm"), "{message}");
    assert!(message.contains("`mergus`"), "{message}");
    assert!(message.contains("the keychain is locked"), "{message}");
    assert!(!message.contains("hunter2"), "{message}");
    assert!(!format!("{error:?}").contains("hunter2"));
}

#[test]
fn mg_req_1007_reference_from_name_needs_a_valid_name() {
    assert_eq!(
        SecretRef::from_name("crm").unwrap().to_string(),
        "secret:crm"
    );
    assert_eq!(SecretRef::from_name("Crm"), None);
    assert_eq!(SecretRef::from_name(""), None);
}

#[test]
fn mg_req_1007_check_refuses_empty_value_and_writes_nothing() {
    let reference = SecretRef::from_name("crm").unwrap();
    assert_eq!(check(&reference, &SecretString::from("pw")), Ok(()));
    let error = check(&reference, &SecretString::from("")).unwrap_err();
    assert_eq!(error.code(), "MG-1001");
}
