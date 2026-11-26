//! MG-REQ-0263 and MG-REQ-1348: `mergus schema` and the `mergus_cli` contract.

use mergus_contracts::generated::mergus_cli_v1::{ArgumentKind, ArgumentType, MergusCli};
use mergus_contracts::{Contract, validate};
use serde_json::Value;

use super::{PROJECT, mergus, project};

/// Every command of the binary, sorted by name.
const COMMANDS: [&str; 10] = [
    "add source",
    "doctor",
    "explain",
    "init",
    "ls",
    "new model",
    "plan",
    "schema",
    "secret set",
    "validate",
];

/// The document `mergus <args>` prints, after checking it is alone on standard output.
fn document(args: &[&str]) -> Value {
    let outcome = mergus(&project(&[("project.toml", PROJECT)]), args);
    assert_eq!(outcome.exit, 0, "{args:?}: {outcome:?}");
    assert_eq!(outcome.stderr, "", "{args:?}");
    assert_eq!(outcome.stdout.lines().count(), 1, "{args:?}: {outcome:?}");
    serde_json::from_str(&outcome.stdout).unwrap()
}

fn names(document: &Value) -> Vec<String> {
    document["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap().to_owned())
        .collect()
}

fn argument<'a>(command: &'a Value, name: &str) -> &'a Value {
    command["arguments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|argument| argument["name"] == name)
        .unwrap_or_else(|| panic!("{} has no argument {name}", command["name"]))
}

#[test]
fn mg_req_0263_schema_describes_every_command() {
    let all = document(&["schema", "--all"]);
    assert_eq!(names(&all), COMMANDS);
    for (index, name) in COMMANDS.iter().enumerate() {
        let mut args = vec!["schema"];
        args.extend(name.split(' '));
        let one = document(&args);
        assert_eq!(names(&one), [*name]);
        assert_eq!(one["commands"][0], all["commands"][index], "{name}");
        let command = &one["commands"][0];
        assert!(!command["summary"].as_str().unwrap().is_empty());
        let codes = command["exit_codes"].as_array().unwrap();
        assert!(codes.contains(&Value::from(0)) && codes.contains(&Value::from(1)));
        for global in ["--json", "--quiet", "--verbose"] {
            assert_eq!(argument(command, global)["kind"], "flag", "{name}");
        }
    }
    let new_model = &all["commands"][5];
    assert_eq!(argument(new_model, "name")["kind"], "positional");
    assert_eq!(argument(new_model, "name")["required"], true);
    assert_eq!(argument(new_model, "--gold")["kind"], "option");
    assert_eq!(argument(new_model, "--py")["type"], "boolean");
    let init = &all["commands"][3];
    assert_eq!(
        argument(init, "--profile")["values"],
        serde_json::json!(["local"])
    );
    assert_eq!(argument(init, "--profile")["default"], "local");
    assert_eq!(argument(init, "--location")["default"], "gold");
    let schema = &all["commands"][7];
    assert_eq!(
        argument(schema, "command")["values"],
        serde_json::json!(COMMANDS)
    );

    for args in [
        &["schema", "nope"][..],
        &["schema"],
        &["schema", "--all", "init"],
    ] {
        let outcome = mergus(&project(&[]), args);
        assert_eq!(outcome.exit, 1, "{args:?}: {outcome:?}");
        assert!(outcome.stderr.starts_with("error[MG-1301]"), "{outcome:?}");
    }
}

#[test]
fn mg_req_0263_schema_describes_selections_and_settings() {
    let all = document(&["schema", "--all"]);
    let plan = &all["commands"][6];
    assert_eq!(argument(plan, "selection")["type"], "selection");
    assert_eq!(argument(plan, "selection")["repeatable"], true);
    assert_eq!(argument(plan, "--exclude")["type"], "selection");
    assert_eq!(argument(plan, "--max-parallel")["type"], "integer");
    assert_eq!(argument(plan, "--max-parallel")["default"], "4");
    let explain = &all["commands"][2];
    assert_eq!(argument(explain, "argument")["required"], true);
}

#[test]
fn mg_req_1348_cli_schema_document() {
    let all = document(&["schema", "--all"]);
    assert_eq!(all, document(&["schema", "--all", "--json"]));
    let one = document(&["--json", "schema", "new", "model"]);
    assert_eq!(one["commands"][0], all["commands"][5]);
    assert_eq!(one["mergus_version"], all["mergus_version"]);
    let typed: MergusCli = serde_json::from_value(all.clone()).unwrap();
    assert_eq!(validate(&typed).unwrap().value(), &all);
    assert_eq!(all["contract"], MergusCli::NAME);
    assert_eq!(MergusCli::NAME, "mergus_cli");
    assert_eq!(MergusCli::SEMANTIC_VERSION, "1.0.0");
    assert_eq!(all["contract_version"], 1);
    assert_eq!(all["mergus_version"], env!("CARGO_PKG_VERSION"));
    let mut sorted = names(&all);
    sorted.sort();
    assert_eq!(names(&all), sorted);
    for command in &typed.commands {
        assert_eq!(
            command.dry_run,
            ["add source", "init", "new model", "secret set"].contains(&command.name.as_str())
        );
        assert_eq!(command.json, command.name != "schema");
        for argument in &command.arguments {
            let dashed = argument.name.starts_with("--");
            assert_eq!(
                dashed,
                argument.kind != ArgumentKind::Positional,
                "{argument:?}"
            );
            assert_eq!(
                argument.kind == ArgumentKind::Flag,
                argument.r#type == ArgumentType::Boolean,
                "{argument:?}"
            );
        }
    }
}
