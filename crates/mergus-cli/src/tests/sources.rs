//! MG-REQ-0409, MG-REQ-0414, MG-REQ-0416: `mergus validate` reports the checks of
//! `mergus-source` like the other stage-1 problems.

use super::{PROJECT, mergus, project};

/// A source whose table `sales` is incremental without `deletes`, on line 5.
const NO_DELETES: &str = "kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:erp\"\n\
                          [tables.sales]\n\
                          incremental = \"updated_at\"\nkey = \"sale_id\"\n";

#[test]
fn mg_req_0416_validate_warns_and_succeeds() {
    let files = project(&[("project.toml", PROJECT), ("sources/erp.toml", NO_DELETES)]);
    let outcome = mergus(&files, &["validate", "--json"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{outcome:?}");
    let warning = &diagnostics[0];
    assert_eq!(warning["code"], "MG-0403");
    assert_eq!(warning["severity"], "warning");
    assert_eq!(warning["location"]["file"], "sources/erp.toml");
    assert_eq!(warning["location"]["line"], 5);
    let message = warning["message"].as_str().unwrap();
    assert!(message.contains("will not be seen"), "{message}");
    assert_eq!(outcome.of("run_finished")[0]["status"], "succeeded");

    let human = mergus(&files, &["validate"]);
    assert_eq!(human.exit, 0);
    assert!(
        human.stderr.starts_with("warning[MG-0403]"),
        "{}",
        human.stderr
    );
    assert!(
        human.stderr.contains(" 5 | incremental = \"updated_at\""),
        "{}",
        human.stderr
    );
}

#[test]
fn mg_req_0409_validate_fix_suggests_copy_with_fresh() {
    let source = "kind = \"database\"\nconnection = \"secret:erp\"\nmode = \"live\"\n\
                  engine = \"postgres\"\n[tables.sales]\nincremental = \"at\"\ndeletes = \"none\"\n";
    let files = project(&[("project.toml", PROJECT), ("sources/erp.toml", source)]);
    let outcome = mergus(&files, &["validate", "--json"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    let diagnostics = outcome.of("diagnostic");
    let codes: Vec<&str> = diagnostics
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["MG-0402", "MG-0401"]);
    assert_eq!(diagnostics[0]["severity"], "error");
    assert_eq!(diagnostics[0]["location"]["line"], 4);
    let hint = diagnostics[0]["hint"].as_str().unwrap();
    assert!(
        hint.contains("mode = \"copy\"") && hint.contains("`fresh`"),
        "{hint}"
    );
    assert_eq!(diagnostics[1]["location"]["line"], 6);
    assert_eq!(outcome.of("run_finished")[0]["status"], "failed");
}
