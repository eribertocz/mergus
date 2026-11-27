//! MG-REQ-0746 and MG-REQ-1306: `mergus plan`.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_sim::SimFileSystem;
use serde_json::Value;

use super::{ROOT, mergus, shop};

/// The paths of every file under `folder` of `files`, relative to [`ROOT`], sorted.
fn tree(files: &SimFileSystem, folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in files.list_dir(folder).unwrap() {
        if files.read(&entry).is_ok() {
            let relative = entry.strip_prefix(ROOT).unwrap();
            found.push(relative.to_string_lossy().replace('\\', "/"));
        } else {
            found.extend(tree(files, &entry));
        }
    }
    found.sort();
    found
}

#[test]
fn mg_req_0746_plan_command_writes_artifact() {
    let files = shop();
    let before = tree(&files, Path::new(ROOT));
    let outcome = mergus(&files, &["plan", "+fact_sales", "--json"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");

    // The plan is written to `.mergus/plans/<plan_id>.json`, and nothing else is written.
    let after = tree(&files, Path::new(ROOT));
    let added: Vec<&String> = after.iter().filter(|path| !before.contains(path)).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    let path = Path::new(ROOT).join(added[0]);
    let bytes = files.read(&path).unwrap();
    let plan: Value = serde_json::from_slice(&bytes).unwrap();
    let plan_id = plan["plan_id"].as_str().unwrap();
    assert_eq!(*added[0], format!(".mergus/plans/{plan_id}.json"));
    assert_eq!(plan["contract"], "mergus_plan");
    assert_eq!(plan["environment"], "prod");
    assert_eq!(
        plan["selected"],
        serde_json::json!(["fact_sales", "stg_customers", "stg_orders"])
    );

    // `plan_ready` carries the steps; nothing is executed.
    let ready = outcome.of("plan_ready");
    assert_eq!(ready.len(), 1);
    let steps = ready[0]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);
    assert!(steps.iter().all(|step| step["action"] == "compute"));
    assert!(steps.iter().all(|step| step["reason"] == "no_result"));
    assert!(outcome.of("step_started").is_empty());
    assert_eq!(outcome.of("run_finished")[0]["exit_code"], 0);

    // An existing file with that name is left untouched.
    files.write(&path, b"kept").unwrap();
    let again = mergus(&files, &["plan", "+fact_sales"]);
    assert_eq!(again.exit, 0, "{again:?}");
    assert_eq!(files.read(&path).unwrap(), b"kept");
    assert!(
        again
            .stdout
            .starts_with(&format!("plan {plan_id}: .mergus/plans/{plan_id}.json\n"))
    );
    assert!(
        again.stdout.contains("compute stg_orders (no_result)\n"),
        "{}",
        again.stdout
    );
}

#[test]
fn mg_req_0746_plan_reports_stale_upstream() {
    // A stale model left out is reported with the selector that includes it (MG-REQ-0717).
    let partial = mergus(&shop(), &["plan", "fact_sales"]);
    assert!(
        partial
            .stdout
            .contains("stale upstream: stg_orders; include it with `+fact_sales`\n"),
        "{}",
        partial.stdout
    );
}

#[test]
fn mg_req_0746_plan_without_selection_plans_every_model() {
    let files = shop();
    let outcome = mergus(&files, &["plan", "--json"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert_eq!(
        outcome.of("plan_ready")[0]["steps"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    // The same project gives the same plan file (MG-REQ-0715).
    let again = shop();
    mergus(&again, &["plan"]);
    let plans = |files: &SimFileSystem| {
        files
            .list_dir(&Path::new(ROOT).join(".mergus/plans"))
            .unwrap()
    };
    assert_eq!(plans(&files), plans(&again));
}

#[test]
fn mg_req_0746_invalid_project_writes_no_plan() {
    let files = super::broken();
    let outcome = mergus(&files, &["plan", "--json"]);
    assert_eq!(outcome.exit, 1);
    assert!(outcome.of("plan_ready").is_empty());
    assert!(!files.exists(&Path::new(ROOT).join(".mergus")).unwrap());
}

#[test]
fn mg_req_0517_plan_shows_raw_off_sources() {
    let files = shop();
    let crm = Path::new(ROOT).join("sources/crm.toml");
    files
        .write(
            &crm,
            b"kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:crm\"\nraw = false\nretention = \"90d\"\n[tables.customers]\n",
        )
        .unwrap();
    let outcome = mergus(&files, &["plan"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    // MG-REQ-0517.2: only the sources with `raw = false`, one line each.
    assert!(
        outcome.stdout.contains("source crm: raw = false\n"),
        "{}",
        outcome.stdout
    );
    assert!(
        !outcome.stdout.contains("source erp:"),
        "{}",
        outcome.stdout
    );
    // MG-REQ-0516.2: the retention in effect, of the sources kept in raw only.
    assert!(
        outcome.stdout.contains("raw retention: 30d\n"),
        "{}",
        outcome.stdout
    );
    // The JSON output does not change.
    let json = mergus(&files, &["plan", "--json"]);
    assert!(!json.stdout.contains("raw = false"), "{}", json.stdout);
}
