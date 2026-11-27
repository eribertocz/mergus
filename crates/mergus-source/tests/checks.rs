//! Tests of the checks of source files (MG-REQ-0401, MG-REQ-0409, MG-REQ-0414, MG-REQ-0416).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{LayoutError, read_configuration};
use mergus_sim::SimFileSystem;
use mergus_source::{Mode, Severity, SourceKind, SourceProblem, check_sources};

const ROOT: &str = "/project";

const DATABASE: &str =
    "kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:erp\"\n";

/// A database source without `engine`, for the checks of the engine.
const NO_ENGINE: &str = "kind = \"database\"\nconnection = \"secret:erp\"\n";

/// Reads a project whose one source `sources/erp.toml` holds `source`, then checks it. A
/// problem of `mergus-project` is returned as its code, any other failure as its message.
fn check(source: &str) -> Result<Vec<SourceProblem>, Vec<String>> {
    let fs = SimFileSystem::new();
    let root = Path::new(ROOT);
    let written = fs
        .create_dir_all(&root.join("sources"))
        .and_then(|()| fs.write(&root.join("project.toml"), b"name = \"sales\"\n"))
        .and_then(|()| fs.write(&root.join("sources/erp.toml"), source.as_bytes()));
    let failed = |message: String| vec![message];
    written.map_err(|error| failed(error.to_string()))?;
    match read_configuration(&fs, root) {
        Ok(configuration) => {
            check_sources(&fs, root, &configuration).map_err(|error| failed(error.to_string()))
        }
        Err(LayoutError::Invalid(problems)) => Err(problems
            .iter()
            .map(|problem| problem.code().to_owned())
            .collect()),
        Err(other) => Err(failed(other.to_string())),
    }
}

type Found = (&'static str, Severity, usize, String);

/// The `(code, severity, line, key)` of each problem of `source`; a failure to check it
/// gives one `(MG-0000, ...)` entry with its messages, which no assertion expects.
fn found(source: &str) -> Vec<Found> {
    match check(source) {
        Ok(problems) => problems
            .iter()
            .map(|p| (p.code(), p.severity(), p.line(), p.key()))
            .collect(),
        Err(messages) => vec![("MG-0000", Severity::Error, 0, messages.join("; "))],
    }
}

/// The message of the first problem of `source`, if any.
fn first_message(source: &str) -> Option<String> {
    check(source).ok()?.first().map(ToString::to_string)
}

#[test]
fn mg_req_0401_source_kind_required() {
    let kinds = [
        ("database", SourceKind::Database, DATABASE),
        (
            "files",
            SourceKind::Files,
            "kind = \"files\"\nlocation = \"data\"\n",
        ),
        ("rest", SourceKind::Rest, "kind = \"rest\"\n"),
    ];
    for (name, kind, source) in kinds {
        assert_eq!(SourceKind::parse(name), Some(kind));
        assert_eq!(check(source), Ok(Vec::new()), "{name}");
    }
    for other in ["ftp", "Database", ""] {
        assert_eq!(SourceKind::parse(other), None);
        let source = format!("kind = \"{other}\"\nconnection = \"secret:erp\"\n");
        assert_eq!(check(&source), Err(vec!["MG-0305".to_owned()]), "{other}");
    }
    assert_eq!(
        check("connection = \"secret:erp\"\n"),
        Err(vec!["MG-0305".to_owned()])
    );
}

#[test]
fn mg_req_0414_incremental_requires_key() {
    let without =
        format!("{DATABASE}[tables.sales]\nincremental = \"updated_at\"\ndeletes = \"none\"\n");
    let problems = check(&without).unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].code(), "MG-0401");
    assert_eq!(problems[0].severity(), Severity::Error);
    assert_eq!(
        (problems[0].path(), problems[0].line()),
        ("sources/erp.toml", 5)
    );
    assert_eq!(problems[0].key(), "tables.sales.incremental");
    assert!(
        problems[0]
            .to_string()
            .contains("table `sales` of source `erp`")
    );
    let keyed = format!(
        "{DATABASE}[tables.sales]\nkey = [\"sale_id\"]\nincremental = \"updated_at\"\n\
         deletes = \"compare\"\n"
    );
    assert_eq!(found(&keyed), []);
    let full_load = format!("{DATABASE}[tables.sales]\ndeletes = \"none\"\n");
    assert_eq!(found(&full_load), []);
    let inline = format!(
        "{DATABASE}tables = {{ sales = {{ incremental = \"at\", deletes = \"none\" }} }}\n"
    );
    assert_eq!(
        found(&inline),
        [(
            "MG-0401",
            Severity::Error,
            4,
            "tables.sales.incremental".to_owned()
        )]
    );
}

#[test]
fn mg_req_0416_missing_delete_policy_warns() {
    let source = format!(
        "{DATABASE}[tables.sales]\nkey = \"sale_id\"\nincremental = \"updated_at\"\n\
         [tables.lines]\nincremental = \"at\"\n"
    );
    let problems = check(&source).unwrap();
    let summary: Vec<_> = problems
        .iter()
        .map(|p| (p.code(), p.severity(), p.line(), p.key()))
        .collect();
    assert_eq!(
        summary,
        [
            (
                "MG-0403",
                Severity::Warning,
                6,
                "tables.sales.incremental".to_owned()
            ),
            (
                "MG-0401",
                Severity::Error,
                8,
                "tables.lines.incremental".to_owned()
            ),
            (
                "MG-0403",
                Severity::Warning,
                8,
                "tables.lines.incremental".to_owned()
            ),
        ]
    );
    assert!(
        problems[0]
            .to_string()
            .contains("rows deleted in the source will not be seen"),
        "{}",
        problems[0]
    );
    for deletes in ["column:deleted", "compare", "none"] {
        let source = format!(
            "{DATABASE}[tables.sales]\nkey = \"id\"\nincremental = \"at\"\ndeletes = \"{deletes}\"\n"
        );
        assert_eq!(found(&source), [], "{deletes}");
    }
    let full_load = format!("{DATABASE}[tables.sales]\nkey = \"id\"\n");
    assert_eq!(found(&full_load), []);
}

#[test]
fn mg_req_0409_live_and_connect_only_on_analytical_engines() {
    live_on_analytical_engines();
    connect_on_versioned_formats();
    anything_else_is_an_error();
    copy_reads_postgresql_or_duckdb();
}

/// The single problem of a database source with `lines` after its required keys.
fn engine_problem(lines: &str) -> Vec<Found> {
    found(&format!("{NO_ENGINE}{lines}"))
}

fn rejected(line: usize, key: &str) -> Vec<Found> {
    vec![("MG-0402", Severity::Error, line, key.to_owned())]
}

/// MG-REQ-0409.1.
fn live_on_analytical_engines() {
    let engines = [
        "duckdb",
        "ducklake",
        "clickhouse",
        "iceberg",
        "delta",
        "parquet",
    ];
    assert_eq!(Mode::Live.engines(), engines);
    for engine in engines {
        let lines = format!("mode = \"live\"\nengine = \"{engine}\"\n");
        assert_eq!(engine_problem(&lines), [], "{engine}");
    }
    let files = "kind = \"files\"\nlocation = \"data\"\nmode = \"live\"\nengine = \"parquet\"\n";
    assert_eq!(found(files), []);
}

/// MG-REQ-0409.2.
fn connect_on_versioned_formats() {
    let engines = ["ducklake", "iceberg", "delta"];
    assert_eq!(Mode::Connect.engines(), engines);
    for engine in engines {
        let lines = format!("mode = \"connect\"\nengine = \"{engine}\"\n");
        assert_eq!(engine_problem(&lines), [], "{engine}");
    }
    for engine in ["duckdb", "clickhouse", "parquet"] {
        let lines = format!("mode = \"connect\"\nengine = \"{engine}\"\n");
        assert_eq!(engine_problem(&lines), rejected(4, "engine"), "{engine}");
    }
}

/// MG-REQ-0409.3: transactional and unknown engines, a missing engine, and an engine in a
/// `copy` source, each with a fix that suggests `copy` with `fresh`.
fn anything_else_is_an_error() {
    for mode in ["live", "connect"] {
        for engine in ["postgres", "mysql", "sqlserver", "oracle", "made_up"] {
            let lines = format!("mode = \"{mode}\"\nengine = \"{engine}\"\n");
            assert_eq!(
                engine_problem(&lines),
                rejected(4, "engine"),
                "{mode} {engine}"
            );
        }
        let lines = format!("mode = \"{mode}\"\n");
        assert_eq!(engine_problem(&lines), rejected(3, "mode"), "{mode}");
    }
    assert_eq!(engine_problem("mode = \"copy\"\n"), rejected(1, "kind"));

    let messages = [
        (
            format!("{NO_ENGINE}mode = \"live\"\nengine = \"postgres\"\n"),
            "engine `postgres`, which it does not allow",
        ),
        (
            format!("{NO_ENGINE}mode = \"connect\"\n"),
            "declares no `engine`; `connect` allows `ducklake`, `iceberg` or `delta`",
        ),
        (
            NO_ENGINE.to_owned(),
            "is a `database` source but declares no `engine`; a `database` source in `copy` \
             mode allows `postgresql` or `duckdb`",
        ),
        (
            format!("{NO_ENGINE}engine = \"postgres\"\n"),
            "is read in `copy` mode on engine `postgres`, which it does not allow; `copy` \
             allows `postgresql` or `duckdb`",
        ),
        (
            "kind = \"files\"\nlocation = \"data\"\nengine = \"parquet\"\n".to_owned(),
            "declares `engine`, which a `files` source in `copy` mode must not declare",
        ),
    ];
    for (source, expected) in messages {
        let message = first_message(&source).unwrap_or_default();
        assert!(message.contains(expected), "{message}");
        assert!(message.starts_with("`sources/erp.toml` line "), "{message}");
    }
}

/// MG-REQ-0409.4: a `copy` source of `kind = "database"` names its database, `postgresql` or
/// `duckdb`, and a `copy` source of another kind sets no `engine`.
fn copy_reads_postgresql_or_duckdb() {
    let engines = ["postgresql", "duckdb"];
    assert_eq!(Mode::Copy.engines_for(SourceKind::Database), engines);
    assert_eq!(Mode::Copy.engines_for(SourceKind::Files), [] as [&str; 0]);
    assert_eq!(Mode::Copy.engines_for(SourceKind::Rest), [] as [&str; 0]);
    assert_eq!(
        Mode::Live.engines_for(SourceKind::Database),
        Mode::Live.engines()
    );
    for engine in engines {
        for mode in ["", "mode = \"copy\"\n"] {
            let lines = format!("{mode}engine = \"{engine}\"\n");
            assert_eq!(engine_problem(&lines), [], "{engine}");
        }
    }
    let file = "kind = \"database\"\nengine = \"duckdb\"\nlocation = \"data/erp.duckdb\"\n";
    assert_eq!(found(file), []);
    for engine in ["postgres", "mysql", "ducklake", "parquet", "made_up"] {
        let lines = format!("engine = \"{engine}\"\n");
        assert_eq!(engine_problem(&lines), rejected(3, "engine"), "{engine}");
    }
    assert_eq!(engine_problem(""), rejected(1, "kind"));
    let files = "kind = \"files\"\nlocation = \"data\"\nengine = \"parquet\"\n";
    assert_eq!(found(files), rejected(3, "engine"));
    let copied = "kind = \"files\"\nlocation = \"data\"\nmode = \"copy\"\n";
    assert_eq!(found(copied), []);
    assert_eq!(
        found("kind = \"rest\"\nengine = \"duckdb\"\n"),
        rejected(2, "engine")
    );
}

#[test]
fn mg_req_0415_unrunnable_deletes_refused() {
    let source = format!(
        "{DATABASE}[tables.sales]\ndeletes = \"compare\"\n\
         [tables.lines]\nkey = \"id\"\ndelete_window = \"7d\"\n"
    );
    let problems = check(&source).unwrap();
    let summary: Vec<_> = problems
        .iter()
        .map(|p| (p.code(), p.severity(), p.line(), p.key()))
        .collect();
    assert_eq!(
        summary,
        [
            (
                "MG-0413",
                Severity::Error,
                5,
                "tables.sales.deletes".to_owned()
            ),
            (
                "MG-0413",
                Severity::Error,
                8,
                "tables.lines.delete_window".to_owned()
            ),
        ]
    );
    assert_eq!(
        problems[0].to_string(),
        "`sources/erp.toml` line 5: table `sales` of source `erp` declares `deletes` but no \
         `key`; `compare` compares the keys of the source with raw"
    );
    let runnable = format!(
        "{DATABASE}[tables.sales]\nkey = \"id\"\nincremental = \"at\"\ndeletes = \"compare\"\n\
         delete_window = \"7d\"\n"
    );
    assert_eq!(found(&runnable), []);
}
