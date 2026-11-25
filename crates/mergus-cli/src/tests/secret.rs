//! MG-REQ-1007: `mergus secret set`.

use mergus_ports::{Keychain, PortError};
use mergus_sim::{SimKeychain, SimRandom};

use super::{Outcome, ROOT, Setup, project};
use crate::run::SecretInput;

/// What standard input holds, and whether it is a terminal.
struct Stdin<'a> {
    terminal: bool,
    text: &'a str,
}

impl SecretInput for Stdin<'_> {
    fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn hidden_line(&self) -> Option<String> {
        Some(
            self.text
                .split(['\r', '\n'])
                .next()
                .unwrap_or_default()
                .to_owned(),
        )
    }

    fn all(&self) -> Option<String> {
        Some(self.text.to_owned())
    }
}

/// A keychain that cannot be written.
struct Locked;

impl Keychain for Locked {
    fn get(&self, _: &str, _: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    fn set(&self, _: &str, _: &str, _: &str) -> Result<(), PortError> {
        Err(PortError::Failed("no Secret Service is running".to_owned()))
    }

    fn delete(&self, _: &str, _: &str) -> Result<(), PortError> {
        Err(PortError::Failed("no Secret Service is running".to_owned()))
    }
}

/// Runs `mergus <args>` with `stdin` and `keychain`, outside any project.
fn secret(args: &[&str], stdin: &Stdin<'_>, keychain: &dyn Keychain) -> Outcome {
    let files = project(&[]);
    let random = SimRandom::new(7);
    Setup {
        files: &files,
        cwd: ROOT,
        args,
        vars: &[],
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    }
    .run_on(keychain, stdin)
}

/// Pipes `text` into `mergus secret set crm` and returns the outcome and the stored value.
fn piped(text: &str) -> (Outcome, Option<String>) {
    let keychain = SimKeychain::new();
    let stdin = Stdin {
        terminal: false,
        text,
    };
    let outcome = secret(&["secret", "set", "crm"], &stdin, &keychain);
    (outcome, keychain.get("mergus", "crm").unwrap())
}

/// Whether `outcome` printed `value` anywhere.
fn shows(outcome: &Outcome, value: &str) -> bool {
    outcome.stdout.contains(value) || outcome.stderr.contains(value)
}

#[test]
fn mg_req_1007_secret_set_stores_in_keychain() {
    // MG-REQ-1007.1: all of standard input, one trailing line ending removed.
    let cases = [
        ("hunter2", "hunter2"),
        ("hunter2\n", "hunter2"),
        ("hunter2\r\n", "hunter2"),
        ("hunter2\n\n", "hunter2\n"),
        (
            "-----BEGIN KEY-----\nabc\n-----END KEY-----\n",
            "-----BEGIN KEY-----\nabc\n-----END KEY-----",
        ),
    ];
    for (text, stored) in cases {
        let (outcome, value) = piped(text);
        assert_eq!(outcome.exit, 0, "{text:?}: {outcome:?}");
        assert_eq!(value.as_deref(), Some(stored), "{text:?}");
        assert!(!shows(&outcome, stored), "{text:?}: {outcome:?}");
        assert_eq!(outcome.stderr, "", "no prompt when piped");
    }
    // The entry of another account is left alone; an entry is replaced.
    let keychain = SimKeychain::with_entries([("mergus", "crm", "old"), ("mergus", "erp", "e")]);
    let stdin = Stdin {
        terminal: false,
        text: "new\n",
    };
    assert_eq!(secret(&["secret", "set", "crm"], &stdin, &keychain).exit, 0);
    assert_eq!(
        keychain.get("mergus", "crm").unwrap().as_deref(),
        Some("new")
    );
    assert_eq!(keychain.get("mergus", "erp").unwrap().as_deref(), Some("e"));
}

#[test]
fn mg_req_1007_terminal_prompt_once_and_first_line() {
    let keychain = SimKeychain::new();
    let stdin = Stdin {
        terminal: true,
        text: "hunter2\nignored\n",
    };
    let outcome = secret(&["secret", "set", "crm"], &stdin, &keychain);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert_eq!(outcome.stderr, "Value of secret crm: ");
    assert_eq!(outcome.stdout, "");
    assert_eq!(
        keychain.get("mergus", "crm").unwrap().as_deref(),
        Some("hunter2")
    );
}

#[test]
fn mg_req_1007_json_events_carry_no_value() {
    let keychain = SimKeychain::new();
    let stdin = Stdin {
        terminal: false,
        text: "hunter2\n",
    };
    let outcome = secret(&["secret", "set", "crm", "--json"], &stdin, &keychain);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert!(!shows(&outcome, "hunter2"), "{outcome:?}");
    let types: Vec<String> = outcome
        .events()
        .iter()
        .map(|event| event["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(types, ["run_started", "run_finished"]);
}

#[test]
fn mg_req_1007_empty_value_refused() {
    for text in ["", "\n", "\r\n"] {
        let (outcome, value) = piped(text);
        assert_eq!(outcome.exit, 1, "{text:?}");
        assert!(outcome.stderr.contains("MG-1001"), "{}", outcome.stderr);
        assert_eq!(value, None, "{text:?}");
    }
    let keychain = SimKeychain::new();
    let typed = Stdin {
        terminal: true,
        text: "\nlater\n",
    };
    let outcome = secret(&["secret", "set", "crm"], &typed, &keychain);
    assert_eq!(outcome.exit, 1);
    assert!(outcome.stderr.contains("MG-1001"), "{}", outcome.stderr);
    assert_eq!(keychain.get("mergus", "crm").unwrap(), None);
}

#[test]
fn mg_req_1007_unwritable_keychain_fails() {
    let stdin = Stdin {
        terminal: false,
        text: "hunter2\n",
    };
    let outcome = secret(&["secret", "set", "crm"], &stdin, &Locked);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert!(outcome.stderr.contains("MG-1002"), "{}", outcome.stderr);
    assert!(
        outcome.stderr.contains(platform_service()),
        "{}",
        outcome.stderr
    );
    assert!(!shows(&outcome, "hunter2"), "{outcome:?}");
    let json = secret(&["secret", "set", "crm", "--json"], &stdin, &Locked);
    let diagnostics = json.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{json:?}");
    assert_eq!(diagnostics[0]["code"], "MG-1002");
    assert!(!shows(&json, "hunter2"), "{json:?}");
}

/// The keychain service of the platform the tests run on.
fn platform_service() -> &'static str {
    if cfg!(target_os = "macos") {
        "Keychain"
    } else if cfg!(windows) {
        "Credential Manager"
    } else {
        "Secret Service"
    }
}

#[test]
fn mg_req_1007_bad_arguments_never_show_the_value() {
    let keychain = SimKeychain::new();
    let stdin = Stdin {
        terminal: false,
        text: "hunter2\n",
    };
    let invalid = secret(&["secret", "set", "Crm"], &stdin, &keychain);
    assert_eq!(invalid.exit, 1);
    assert!(invalid.stderr.contains("MG-1305"), "{}", invalid.stderr);
    for args in [
        ["secret", "set", "crm", "s3cr3t"],
        ["secret", "set", "crm", "--value=s3cr3t"],
        ["secret", "set", "crm", "-s3cr3t"],
    ] {
        let outcome = secret(&args, &stdin, &keychain);
        assert_eq!(outcome.exit, 1, "{args:?}");
        assert!(outcome.stderr.contains("MG-1301"), "{}", outcome.stderr);
        assert!(!shows(&outcome, "s3cr3t"), "{args:?}: {outcome:?}");
        let mut json = args.to_vec();
        json.push("--json");
        let outcome = secret(&json, &stdin, &keychain);
        assert!(!shows(&outcome, "s3cr3t"), "{json:?}: {outcome:?}");
    }
    assert_eq!(keychain.get("mergus", "crm").unwrap(), None);
}
