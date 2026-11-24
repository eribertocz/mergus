//! MG-REQ-1310 and MG-REQ-1352: `mergus explain` and the selector `changed`.

use super::{mergus, shop};

#[test]
fn mg_req_1310_explain_dispatch() {
    let code = mergus(&shop(), &["explain", "MG-1307"]);
    assert_eq!(code.exit, 0, "{code:?}");
    assert_eq!(code.stdout, include_str!("../../errors/MG-1307.md"));
    let model = mergus(&shop(), &["explain", "stg_orders"]);
    assert_eq!(model.exit, 0, "{model:?}");
    assert!(
        model.stdout.starts_with("model: stg_orders\n"),
        "{}",
        model.stdout
    );
    // Not `MG-` and four digits: a model name, which does not exist.
    for argument in ["MG-130", "MG-13077", "mg-1307"] {
        let outcome = mergus(&shop(), &["explain", argument, "--json"]);
        assert_eq!(outcome.of("diagnostic")[0]["code"], "MG-1310", "{argument}");
    }
}

#[test]
fn mg_req_1352_changed_and_explain() {
    // MG-REQ-1352.1: without storage, no result of `prod` is available: every model.
    let changed = mergus(&shop(), &["ls", "changed"]);
    assert_eq!(changed.stdout.lines().count(), 4, "{changed:?}");
    assert!(
        changed
            .stdout
            .lines()
            .all(|line| line.ends_with(": changed: no result of `prod` is available")),
        "{}",
        changed.stdout
    );

    // MG-REQ-1352.2: name, identifier, file, inputs, readers, then every effective option
    // with value and origin.
    let model = mergus(&shop(), &["explain", "stg_customers"]);
    assert_eq!(model.exit, 0, "{model:?}");
    let lines: Vec<&str> = model.stdout.lines().collect();
    assert_eq!(lines[0], "model: stg_customers");
    assert!(lines[1].starts_with("identifier: mdl_"), "{}", lines[1]);
    assert_eq!(lines[2], "file: models/staging/stg_customers.sql");
    assert_eq!(lines[3], "inputs: raw.crm__customers");
    assert_eq!(lines[4], "read by: dim_customer, fact_sales");
    assert_eq!(lines[5], "options:");
    assert!(lines.contains(&"  tags = [\"staging\" (models/staging/mergus.toml:2)]"));
    assert!(lines.contains(&"  materialize = false (default)"));
    assert!(lines.contains(&"  checks = [] (default)"));
    assert!(lines[6..].iter().all(|line| line.starts_with("  ")));

    // MG-REQ-1352.3: a code prints its catalog file; an unknown code or model names the
    // closest one.
    let code = mergus(&shop(), &["explain", "MG-0301"]);
    assert_eq!(code.stdout, include_str!("../../errors/MG-0301.md"));
    let unknown_code = mergus(&shop(), &["explain", "MG-1399", "--json"]);
    assert_eq!(unknown_code.exit, 1);
    let diagnostic = &unknown_code.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1311");
    assert!(
        diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("did you mean `MG-13")
    );
    let unknown_model = mergus(&shop(), &["explain", "fact_sale", "--json"]);
    assert_eq!(unknown_model.exit, 1);
    let diagnostic = &unknown_model.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1310");
    assert_eq!(diagnostic["hint"], "Use `fact_sales`.");
}
