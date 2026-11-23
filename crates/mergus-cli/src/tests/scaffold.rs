//! MG-REQ-1304 and MG-REQ-1350: `mergus new model` and `mergus add source`.

use std::path::Path;

use mergus_ports::FileSystem;

use super::{PROJECT, ROOT, exists, mergus, mergus_in, project, text};

/// The Python model `orders` of MG-REQ-1350.1.
const PYTHON: &str = "import polars as pl\n\
                      \n\
                      \n\
                      def orders() -> pl.DataFrame:\n    \
                      return pl.DataFrame({\"id\": [1]})\n";

/// The source `crm` of MG-REQ-1350.2.
const SOURCE: &str = "#:schema https://mergus.dev/schemas/source.schema.json\n\
                      kind = \"database\"\n\
                      connection = \"secret:crm\"\n";

#[test]
fn mg_req_1304_scaffolding_commands() {
    let files = project(&[("project.toml", PROJECT)]);
    for args in [
        &["new", "model", "orders"][..],
        &["new", "model", "events", "--py"],
        &["new", "model", "--gold", "sales", "customers"],
        &["add", "source", "crm"],
    ] {
        let outcome = mergus(&files, args);
        assert_eq!(outcome.exit, 0, "{args:?}: {outcome:?}");
    }
    for path in [
        "models/orders.sql",
        "models/events.py",
        "models/gold/sales/customers.sql",
        "sources/crm.toml",
    ] {
        assert!(exists(&files, path), "{path}");
    }
    assert!(
        text(&files, "sources/crm.toml")
            .unwrap()
            .contains("\"secret:crm\"")
    );
    let validated = mergus(&files, &["validate", "--json"]);
    assert_eq!(validated.exit, 0, "{validated:?}");

    let below = format!("{ROOT}/models/gold");
    let outcome = mergus_in(&files, &below, &["new", "model", "refunds"], None);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert!(
        exists(&files, "models/refunds.sql"),
        "relative to the project root"
    );
    let nowhere = mergus(&project(&[]), &["add", "source", "crm", "--json"]);
    assert_eq!(nowhere.of("diagnostic")[0]["code"], "MG-1302");
}

#[test]
fn mg_req_1350_scaffolding_files() {
    scaffolding_files_1();
    scaffolding_files_2();
    scaffolding_files_3();
}

/// MG-REQ-1350.1: the model files.
fn scaffolding_files_1() {
    let files = project(&[("project.toml", PROJECT)]);
    assert_eq!(mergus(&files, &["new", "model", "orders"]).exit, 0);
    assert_eq!(
        text(&files, "models/orders.sql").unwrap(),
        "SELECT 1 AS id\n"
    );
    assert_eq!(mergus(&files, &["new", "model", "items", "--sql"]).exit, 0);
    assert_eq!(
        text(&files, "models/items.sql").unwrap(),
        "SELECT 1 AS id\n"
    );
    let gold = ["new", "model", "customers", "--gold", "sales"];
    assert_eq!(mergus(&files, &gold).exit, 0);
    let path = "models/gold/sales/customers.sql";
    assert_eq!(text(&files, path).unwrap(), "SELECT 1 AS id\n");

    let files = project(&[("project.toml", PROJECT)]);
    assert_eq!(mergus(&files, &["new", "model", "orders", "--py"]).exit, 0);
    assert_eq!(text(&files, "models/orders.py").unwrap(), PYTHON);
    assert!(!exists(&files, "models/orders.sql"));
    let both = mergus(&files, &["new", "model", "x", "--py", "--sql", "--json"]);
    assert_eq!(
        (both.exit, both.of("diagnostic")[0]["code"].as_str()),
        (1, Some("MG-1301"))
    );
}

/// MG-REQ-1350.2: the source file.
fn scaffolding_files_2() {
    let files = project(&[("project.toml", PROJECT)]);
    assert_eq!(mergus(&files, &["add", "source", "crm"]).exit, 0);
    assert_eq!(text(&files, "sources/crm.toml").unwrap(), SOURCE);
}

/// The single diagnostic code of `mergus <args> --json` on `files`, after checking the exit
/// code is 1.
fn code(files: &dyn FileSystem, args: &[&str]) -> String {
    let mut args = args.to_vec();
    args.push("--json");
    let outcome = mergus(files, &args);
    assert_eq!(outcome.exit, 1, "{args:?}: {outcome:?}");
    let diagnostics = outcome.of("diagnostic");
    assert_eq!(diagnostics.len(), 1, "{outcome:?}");
    diagnostics[0]["code"].as_str().unwrap().to_owned()
}

/// MG-REQ-1350.3: invalid and taken names, existing files, `--dry-run`.
fn scaffolding_files_3() {
    let files = project(&[
        ("project.toml", PROJECT),
        ("models/staging/orders.sql", "select 1"),
        ("sources/crm.toml", "kind = \"database\"\n"),
    ]);
    let cases: [(&[&str], &str); 7] = [
        (&["new", "model", "Orders"], "MG-1305"),
        (&["new", "model", "items", "--gold", "Sales"], "MG-1305"),
        (&["add", "source", "my-crm"], "MG-1305"),
        (&["new", "model", "orders"], "MG-1306"),
        (&["new", "model", "orders", "--py"], "MG-1306"),
        (&["new", "model", "orders", "--gold", "x"], "MG-1306"),
        (&["add", "source", "crm"], "MG-1307"),
    ];
    for (args, expected) in cases {
        assert_eq!(code(&files, args), expected, "{args:?}");
    }
    let staging = format!("{ROOT}/models/staging");
    let existing = mergus_in(
        &files,
        &staging,
        &["new", "model", "orders", "--json"],
        None,
    );
    assert_eq!(existing.exit, 1);
    assert_eq!(
        text(&files, "sources/crm.toml").unwrap(),
        "kind = \"database\"\n"
    );
    assert!(!exists(&files, "models/Orders.sql") && !exists(&files, "sources/my-crm.toml"));
    let human = mergus(&files, &["new", "model", "Orders"]);
    assert!(
        human.stderr.contains("`orders`"),
        "the valid name: {}",
        human.stderr
    );

    let files = project(&[("project.toml", PROJECT), ("models/orders.py", "")]);
    assert_eq!(code(&files, &["new", "model", "orders", "--py"]), "MG-1307");

    let refused = super::project::ReadOnly {
        inner: project(&[("project.toml", PROJECT)]),
        changes: std::sync::Mutex::new(Vec::new()),
    };
    assert_eq!(code(&refused, &["add", "source", "crm"]), "MG-1308");

    dry_run();
}

/// `--dry-run` lists the files and writes none.
fn dry_run() {
    let files = project(&[("project.toml", PROJECT)]);
    let cases: [(&[&str], &str); 3] = [
        (
            &["new", "model", "orders", "--dry-run"],
            "models/orders.sql\n",
        ),
        (
            &["new", "model", "c", "--gold", "s", "--dry-run"],
            "models/gold/s/c.sql\n",
        ),
        (&["add", "source", "crm", "--dry-run"], "sources/crm.toml\n"),
    ];
    for (args, listed) in cases {
        let outcome = mergus(&files, args);
        assert_eq!(
            (outcome.exit, outcome.stdout.as_str()),
            (0, listed),
            "{args:?}"
        );
    }
    let root = files.list_dir(Path::new(ROOT)).unwrap();
    assert_eq!(
        root,
        [Path::new(ROOT).join("project.toml")],
        "nothing written"
    );

    let empty = project(&[]);
    let outcome = mergus(&empty, &["init", "--dry-run"]);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert_eq!(
        outcome.stdout,
        "project.toml\nmodels/\nsources/\n.gitignore\n"
    );
    assert!(empty.list_dir(Path::new(ROOT)).unwrap().is_empty());
    let taken = project(&[("project.toml", PROJECT)]);
    assert_eq!(code(&taken, &["init", "--dry-run"]), "MG-1307");
    assert_eq!(
        code(&taken, &["new", "model", "Bad", "--dry-run"]),
        "MG-1305"
    );
}
