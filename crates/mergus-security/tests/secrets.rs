//! MG-REQ-1001, MG-REQ-1002 and MG-REQ-1005: how a secret reference is resolved, and what a
//! resolved value can never do.

use std::fmt::Display;
use std::path::Path;

use mergus_ports::{FileSystem, Keychain};
use mergus_security::{ExposeSecret, KEYCHAIN_SERVICE, SecretRef, SecretString, resolve};
use mergus_sim::{SimEnvironment, SimFileSystem, SimKeychain};

/// Resolves the reference written `text`, or returns `None` when it is not a reference or
/// does not resolve.
fn resolved(env: &[(&str, &str)], keychain: &SimKeychain, text: &str) -> Option<String> {
    let environment = SimEnvironment::new(env.iter().copied(), None);
    resolve(&SecretRef::parse(text)?, &environment, keychain)
        .ok()
        .map(|value| value.expose_secret().to_owned())
}

#[test]
fn mg_req_1001_resolution_chain_order() {
    let keychain = SimKeychain::with_entries([
        ("mergus", "token", "from-keychain"),
        ("other", "api_key", "wrong-service"),
        ("mergus", "API_KEY", "wrong-account"),
    ]);
    // MG-REQ-1001.1: the variable answers first, even when the keychain has the entry.
    let env = [("MERGUS_SECRET_TOKEN", "from-env")];
    assert_eq!(
        resolved(&env, &keychain, "secret:token").as_deref(),
        Some("from-env")
    );
    // A variable set to an empty value answers.
    let empty = [("MERGUS_SECRET_TOKEN", "")];
    assert_eq!(
        resolved(&empty, &keychain, "secret:token").as_deref(),
        Some("")
    );
    // `<NAME>` is the name in ASCII upper case.
    let upper = [("MERGUS_SECRET_API_KEY_2", "upper")];
    assert_eq!(
        resolved(&upper, &keychain, "secret:api_key_2").as_deref(),
        Some("upper")
    );
    let lower = [("MERGUS_SECRET_api_key_2", "lower")];
    assert_eq!(resolved(&lower, &keychain, "secret:api_key_2"), None);
    // MG-REQ-1001.2: then the keychain, under the service `mergus` and the account `<name>`.
    assert_eq!(KEYCHAIN_SERVICE, "mergus");
    assert_eq!(
        resolved(&[], &keychain, "secret:token").as_deref(),
        Some("from-keychain")
    );
    assert_eq!(resolved(&[], &keychain, "secret:api_key"), None);
}

#[test]
fn mg_req_1001_unresolved_reference_names_both_places() {
    let reference = |text: &str| SecretRef::parse(text).unwrap();
    let keychain = SimKeychain::with_entries([
        ("mergus", "token", "from-keychain"),
        ("other", "api_key", "wrong-service"),
        ("mergus", "API_KEY", "wrong-account"),
    ]);
    // MG-REQ-1001.3: when neither answers, a typed error names the reference and both
    // places, and never a value.
    let environment = SimEnvironment::new([("MERGUS_SECRET_TOKEN", "from-env")], None);
    let error = resolve(&reference("secret:api_key"), &environment, &keychain).unwrap_err();
    assert_eq!(error.reference(), &reference("secret:api_key"));
    let message = error.to_string();
    for expected in [
        "secret:api_key",
        "MERGUS_SECRET_API_KEY",
        "`mergus`",
        "`api_key`",
    ] {
        assert!(message.contains(expected), "{message}");
    }
    for value in [
        "from-env",
        "from-keychain",
        "wrong-service",
        "wrong-account",
    ] {
        assert!(!message.contains(value), "{message}");
        assert!(!format!("{error:?}").contains(value), "{error:?}");
    }
}

#[test]
fn mg_req_1001_references_are_secret_and_a_valid_name() {
    let reference = |text: &str| SecretRef::parse(text).unwrap();
    assert_eq!(reference("secret:token").name(), "token");
    assert_eq!(reference("secret:token").to_string(), "secret:token");
    for invalid in [
        "token",
        "secret:",
        "secret:Token",
        "secret:1st",
        "secret:a-b",
        "env:token",
    ] {
        assert_eq!(SecretRef::parse(invalid), None, "{invalid}");
    }
    assert_eq!(
        SecretRef::parse(&format!("secret:{}", "a".repeat(64))),
        None
    );
}

/// Compiles only when `T` does not implement `Display`: with an implementation, the call
/// below has two candidate impls and is ambiguous.
trait AmbiguousIfDisplay<A> {
    fn check() {}
}
impl<T: ?Sized> AmbiguousIfDisplay<()> for T {}
struct ImplementsDisplay;
impl<T: ?Sized + Display> AmbiguousIfDisplay<ImplementsDisplay> for T {}

/// The same for `serde::Serialize`.
trait AmbiguousIfSerialize<A> {
    fn check() {}
}
impl<T: ?Sized> AmbiguousIfSerialize<()> for T {}
struct ImplementsSerialize;
impl<T: ?Sized + serde::Serialize> AmbiguousIfSerialize<ImplementsSerialize> for T {}

#[test]
fn mg_req_1002_secret_type_never_displays() {
    let reference = |text: &str| SecretRef::parse(text).unwrap();
    // A resolved value cannot be printed with `{}` nor serialized: both checks are made by
    // the compiler.
    <SecretString as AmbiguousIfDisplay<_>>::check();
    <SecretString as AmbiguousIfSerialize<_>>::check();
    // Its `Debug` form, which logs and `{:?}` use, never shows the value.
    let keychain = SimKeychain::new();
    let environment = SimEnvironment::new([("MERGUS_SECRET_PASSWORD", "hunter2")], None);
    let value = resolve(&reference("secret:password"), &environment, &keychain).unwrap();
    assert_eq!(value.expose_secret(), "hunter2");
    let debug = format!("{value:?} {value:#?}");
    assert!(!debug.contains("hunter2"), "{debug}");
    assert!(debug.contains("REDACTED"), "{debug}");
}

#[test]
fn mg_req_1005_dotenv_ignored() {
    let reference = |text: &str| SecretRef::parse(text).unwrap();
    let file_system = SimFileSystem::new();
    let dotenv = "MERGUS_SECRET_TOKEN=from-dotenv\nTOKEN=from-dotenv\n";
    file_system.create_dir_all(Path::new("/project")).unwrap();
    file_system
        .write(Path::new("/project/.env"), dotenv.as_bytes())
        .unwrap();
    let environment =
        SimEnvironment::new([("HOME", "/project")], None).with_current_dir("/project");
    let keychain = SimKeychain::new();
    let error = resolve(&reference("secret:token"), &environment, &keychain).unwrap_err();
    assert!(!error.to_string().contains("from-dotenv"), "{error}");
    assert!(keychain.get("mergus", "token").unwrap().is_none());
}
