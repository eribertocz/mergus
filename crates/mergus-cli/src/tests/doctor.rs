//! MG-REQ-1004: `mergus doctor` shows where each secret reference resolves, never its value.

use mergus_security::SecretRef;
use mergus_sim::{SimKeychain, SimRandom};

use super::{NoInput, Outcome, PROJECT, ROOT, Setup, project};

/// The environment variable of the secret `name`, as `mergus-security` names it.
fn variable(name: &str) -> String {
    SecretRef::from_name(name).unwrap().variable()
}

/// The value of the variable of `secret:erp`.
const FROM_ENVIRONMENT: &str = "pg-env-Qx7v";

/// The value of the keychain entry `mergus/crm`.
const FROM_KEYCHAIN: &str = "pg-keychain-Zr4w";

/// Runs `mergus <args>` on a project of four sources: `erp` uses `secret:erp` and
/// `secret:erp_replica`, `crm` and `billing` both use `secret:crm`, `local` is a DuckDB file.
/// `secret:erp` is in the environment, `secret:crm` in the keychain, and `secret:erp_replica`
/// in the environment only when `replica` is true.
fn doctor(args: &[&str], replica: bool) -> Outcome {
    let files = project(&[
        ("project.toml", PROJECT),
        (
            "sources/erp.toml",
            "kind = \"database\"\nconnection = \"secret:erp\"\nreplica = \"secret:erp_replica\"\n",
        ),
        (
            "sources/crm.toml",
            "kind = \"database\"\nconnection = \"secret:crm\"\n",
        ),
        (
            "sources/billing.toml",
            "kind = \"database\"\nconnection = \"secret:crm\"\n",
        ),
        (
            "sources/local.toml",
            "kind = \"database\"\nengine = \"duckdb\"\nlocation = \"data/local.duckdb\"\n",
        ),
    ]);
    let keychain = SimKeychain::with_entries([("mergus", "crm", FROM_KEYCHAIN)]);
    let (erp, erp_replica) = (variable("erp"), variable("erp_replica"));
    let mut vars = vec![(erp.as_str(), FROM_ENVIRONMENT)];
    if replica {
        vars.push((erp_replica.as_str(), "pg-replica-Kd2m"));
    }
    let random = SimRandom::new(7);
    Setup {
        files: &files,
        cwd: ROOT,
        args,
        vars: &vars,
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    }
    .run_on(&keychain, &NoInput)
}

/// Fails when `outcome` shows a value, or any part of one (MG-REQ-1004.2).
fn assert_no_value(outcome: &Outcome) {
    for value in [FROM_ENVIRONMENT, FROM_KEYCHAIN, "pg-replica-Kd2m"] {
        for part in [value, &value[..6], &value[value.len() - 4..]] {
            assert!(!outcome.stdout.contains(part), "{part}: {outcome:?}");
            assert!(!outcome.stderr.contains(part), "{part}: {outcome:?}");
        }
    }
}

#[test]
fn mg_req_1004_doctor_shows_secret_origin() {
    every_reference_once_with_its_origin();
    missing_reference_is_an_error();
    events_of_a_read_command();
}

fn every_reference_once_with_its_origin() {
    // MG-REQ-1004.1: every reference once, sorted by name, with the link that answered.
    let found = doctor(&["doctor"], true);
    assert_eq!(found.exit, 0, "{found:?}");
    assert_eq!(
        found.stdout,
        format!(
            "crm: keychain mergus/crm\nerp: environment {}\nerp_replica: environment {}\n",
            variable("erp"),
            variable("erp_replica")
        )
    );
    assert_eq!(found.stderr, "");
    assert_no_value(&found);
}

fn missing_reference_is_an_error() {
    // MG-REQ-1004.3: a missing reference is an error `MG-1003` whose fix is `secret set`.
    let missing = doctor(&["doctor"], false);
    assert_eq!(missing.exit, 1, "{missing:?}");
    assert_eq!(
        missing.stdout,
        format!(
            "crm: keychain mergus/crm\nerp: environment {}\nerp_replica: missing\n",
            variable("erp")
        )
    );
    assert!(
        missing.stderr.starts_with("error[MG-1003]"),
        "{}",
        missing.stderr
    );
    assert!(
        missing.stderr.contains("mergus secret set erp_replica"),
        "{}",
        missing.stderr
    );
    assert_no_value(&missing);
}

fn events_of_a_read_command() {
    // MG-REQ-1004.4: the events of the read commands, each missing reference a diagnostic.
    let json = doctor(&["doctor", "--json"], false);
    assert_eq!(json.exit, 1, "{json:?}");
    let kinds: Vec<String> = json
        .events()
        .iter()
        .map(|event| event["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["run_started", "diagnostic", "run_finished"]);
    let diagnostic = &json.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1003");
    assert_eq!(diagnostic["severity"], "error");
    let message = diagnostic["message"].as_str().unwrap();
    assert!(
        message.contains("secret:erp_replica")
            && message.contains(&variable("erp_replica"))
            && message.contains("service `mergus`"),
        "{message}"
    );
    let hint = diagnostic["hint"].as_str().unwrap();
    assert!(hint.contains("`mergus secret set erp_replica`"), "{hint}");
    assert_eq!(
        diagnostic["next"],
        serde_json::json!([["mergus", "secret", "set", "erp_replica"]])
    );
    assert_eq!(json.of("run_finished")[0]["exit_code"], 1);
    assert_no_value(&json);

    // It changes nothing, so it takes no `--dry-run`.
    let dry = doctor(&["doctor", "--dry-run"], true);
    assert_eq!(dry.exit, 1, "{dry:?}");
    assert!(dry.stdout.is_empty(), "{dry:?}");
}
