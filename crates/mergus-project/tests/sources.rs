//! Tests of the schema of `sources/*.toml` (MG-REQ-0376).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{LayoutError, ProjectError, read_configuration};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

/// The problems of reading a project whose one source file holds `source`; none when valid.
/// A failure of the simulated file system is returned as an invalid TOML problem, which no
/// assertion of this file expects.
fn problems(source: &str) -> Vec<ProjectError> {
    let fs = SimFileSystem::new();
    let root = Path::new(ROOT);
    let read = fs
        .create_dir_all(&root.join("sources"))
        .and_then(|()| fs.write(&root.join("project.toml"), b"name = \"sales\"\n"))
        .and_then(|()| fs.write(&root.join("sources/crm.toml"), source.as_bytes()))
        .map_err(LayoutError::from)
        .and_then(|()| read_configuration(&fs, root));
    match read {
        Ok(_) => Vec::new(),
        Err(LayoutError::Invalid(problems)) => problems,
        Err(other) => vec![ProjectError::InvalidToml {
            path: String::new(),
            line: 0,
            message: other.to_string(),
        }],
    }
}

/// The `(code, key)` of each problem of `source`.
fn found(source: &str) -> Vec<(&'static str, String)> {
    problems(source)
        .iter()
        .map(|problem| (problem.code(), problem.key().unwrap_or_default().to_owned()))
        .collect()
}

/// The single problem of `source`, as `(code, key)`.
fn one(source: &str) -> (&'static str, String) {
    let all = found(source);
    assert_eq!(all.len(), 1, "{source}: {all:?}");
    all[0].clone()
}

fn invalid(key: &str) -> (&'static str, String) {
    ("MG-0305", key.to_owned())
}

fn unknown(key: &str) -> (&'static str, String) {
    ("MG-0304", key.to_owned())
}

const DATABASE: &str = "kind = \"database\"\nconnection = \"secret:crm\"\n";
const FILES: &str = "kind = \"files\"\nlocation = \"s3://bucket/crm/\"\n";

#[test]
fn mg_req_0376_source_keys() {
    kinds();
    source_options();
    secret_references();
    table_options();
    file_tables();
    rest_is_open();
}

/// MG-REQ-0376.1: `kind` is required and one of three.
fn kinds() {
    assert_eq!(found(DATABASE), []);
    assert_eq!(found(FILES), []);
    assert_eq!(found("kind = \"rest\"\n"), []);
    assert_eq!(one("connection = \"secret:crm\"\n"), invalid("kind"));
    let wrong = problems("kind = \"databse\"\nconnection = \"secret:crm\"\n");
    assert_eq!(wrong.len(), 1, "{wrong:?}");
    assert_eq!(wrong[0].code(), "MG-0305");
    assert!(
        wrong[0].to_string().contains("did you mean `database`"),
        "{}",
        wrong[0]
    );
}

/// MG-REQ-0376.2, MG-REQ-0376.4 and MG-REQ-0376.5: the options of a source and their values.
fn source_options() {
    let full = format!(
        "{DATABASE}replica = \"secret:crm_replica\"\nmode = \"copy\"\nraw = true\n\
         retention = \"30d\"\nload = \"low\"\nmax_connections = 1\nwindow = \"22:00-06:30\"\n\
         query_timeout = \"10m\"\noverlap = \"1h\"\nlocation = \"data/crm\"\n\
         engine = \"postgres\"\n"
    );
    assert_eq!(found(&full), []);
    let engine = format!("{FILES}engine = \"ducklake\"\n");
    assert_eq!(found(&engine), []);
    for (line, key) in [
        ("mode = \"stream\"\n", "mode"),
        ("engine = \"DuckDB\"\n", "engine"),
        ("engine = 1\n", "engine"),
        ("raw = \"yes\"\n", "raw"),
        ("retention = \"30\"\n", "retention"),
        ("retention = \"0d\"\n", "retention"),
        ("load = \"max\"\n", "load"),
        ("max_connections = 0\n", "max_connections"),
        ("max_connections = 1.5\n", "max_connections"),
        ("window = \"24:00-06:00\"\n", "window"),
        ("window = \"9:00-17:00\"\n", "window"),
        ("query_timeout = \"10s\"\n", "query_timeout"),
        ("overlap = 5\n", "overlap"),
        ("location = \"\"\n", "location"),
    ] {
        assert_eq!(one(&format!("{DATABASE}{line}")), invalid(key), "{line}");
    }
    assert_eq!(one("kind = \"files\"\n"), invalid("location"));
    let typo = problems(&format!("{DATABASE}max_conections = 2\n"));
    assert_eq!(typo.len(), 1, "{typo:?}");
    assert_eq!(typo[0].code(), "MG-0304");
    assert_eq!(typo[0].line(), Some(3));
    assert!(
        typo[0]
            .to_string()
            .contains("did you mean `max_connections`")
    );
    assert_eq!(
        one(&format!("{DATABASE}[options]\nport = 5432\n")),
        unknown("options")
    );
}

/// MG-REQ-0376.3: `connection` is required for a database; both are secret references.
fn secret_references() {
    assert_eq!(one("kind = \"database\"\n"), invalid("connection"));
    for reference in [
        "crm",
        "secret:",
        "secret:CRM",
        "secret:crm-db",
        "Secret:crm",
    ] {
        let source = format!("kind = \"database\"\nconnection = \"{reference}\"\n");
        assert_eq!(one(&source), invalid("connection"), "{reference}");
        let source = format!("{DATABASE}replica = \"{reference}\"\n");
        assert_eq!(one(&source), invalid("replica"), "{reference}");
    }
}

/// MG-REQ-0376.6: the tables and their options.
fn table_options() {
    let tables = format!(
        "{DATABASE}[tables.orders]\nkey = \"id\"\nincremental = \"updated_at\"\noverlap = \"2h\"\n\
         deletes = \"column:deleted\"\ndelete_check = \"daily\"\ndelete_window = \"7d\"\n\
         [tables.order_lines]\nkey = [\"order_id\", \"line\"]\ndeletes = \"compare\"\n\
         [tables.events]\ndeletes = \"none\"\ndelete_check = \"every_run\"\n"
    );
    assert_eq!(found(&tables), []);
    for (line, key) in [
        ("key = []\n", "tables.orders.key"),
        ("key = \"Id\"\n", "tables.orders.key"),
        ("incremental = [\"a\"]\n", "tables.orders.incremental"),
        ("deletes = \"column:\"\n", "tables.orders.deletes"),
        ("deletes = \"soft\"\n", "tables.orders.deletes"),
        ("delete_check = \"monthly\"\n", "tables.orders.delete_check"),
        ("delete_window = \"7\"\n", "tables.orders.delete_window"),
    ] {
        let source = format!("{DATABASE}[tables.orders]\n{line}");
        assert_eq!(one(&source), invalid(key), "{line}");
    }
    let bad_name = format!("{DATABASE}[tables.Orders]\n");
    assert_eq!(one(&bad_name), unknown("tables.Orders"));
    let extra = format!("{DATABASE}[tables.orders]\nfilter = \"x\"\n");
    assert_eq!(one(&extra), unknown("tables.orders.filter"));
    let files_only = format!("{DATABASE}[tables.orders]\nformat = \"csv\"\n");
    assert_eq!(one(&files_only), unknown("tables.orders.format"));
}

/// MG-REQ-0376.6: a table of a `files` source has a path and a format.
fn file_tables() {
    let table =
        "[tables.orders]\npath = \"orders/*.parquet\"\nformat = \"parquet\"\nkey = \"id\"\n";
    assert_eq!(found(&format!("{FILES}{table}")), []);
    for format in ["csv", "json"] {
        let source = format!("{FILES}[tables.orders]\npath = \"o/*\"\nformat = \"{format}\"\n");
        assert_eq!(found(&source), [], "{format}");
    }
    let missing = found(&format!("{FILES}[tables.orders]\n"));
    assert_eq!(
        missing,
        [
            invalid("tables.orders.format"),
            invalid("tables.orders.path")
        ]
    );
    let bad_format = format!("{FILES}[tables.orders]\npath = \"o/*\"\nformat = \"xml\"\n");
    assert_eq!(one(&bad_format), invalid("tables.orders.format"));
    let empty_path = format!("{FILES}[tables.orders]\npath = \"\"\nformat = \"csv\"\n");
    assert_eq!(one(&empty_path), invalid("tables.orders.path"));
}

/// MG-REQ-0376.7: a `rest` source accepts any key.
fn rest_is_open() {
    let rest = "kind = \"rest\"\nbase_url = \"https://api.local\"\n[options]\nport = 5432\n\
                [tables.Orders]\nanything = [1, \"x\"]\n";
    assert_eq!(found(rest), []);
}
