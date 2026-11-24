//! MG-REQ-1309 and MG-REQ-1351: the settings of phase 0.2.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_sim::{SimFileSystem, SimRandom};
use serde_json::Value;

use super::{Outcome, ROOT, Setup, shop};

/// The flags, variables and user configuration file of one run.
type Inputs<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)], Option<&'a str>);

/// The user configuration directory of the tests.
const CONFIG_DIR: &str = "/home/ana/.config";

/// Runs `mergus plan --json <args>` on `files` with `vars` and the user configuration file
/// `config`, when it is given.
fn plan(
    files: &SimFileSystem,
    args: &[&str],
    vars: &[(&str, &str)],
    config: Option<&str>,
) -> Outcome {
    if let Some(config) = config {
        let path = Path::new(CONFIG_DIR).join("mergus/config.toml");
        files.create_dir_all(path.parent().unwrap()).unwrap();
        files.write(&path, config.as_bytes()).unwrap();
    }
    let mut full = vec!["plan", "--json"];
    full.extend_from_slice(args);
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd: ROOT,
        args: &full,
        vars,
        random: &random,
        seed: 7,
        terminal: false,
        input: None,
    }
    .run_with_config_dir(Some(CONFIG_DIR))
}

/// The `max_parallel` of the only plan written in `files`.
fn written_max_parallel(files: &SimFileSystem) -> Value {
    let plans = files
        .list_dir(&Path::new(ROOT).join(".mergus/plans"))
        .unwrap();
    assert_eq!(plans.len(), 1, "{plans:?}");
    let plan: Value = serde_json::from_slice(&files.read(&plans[0]).unwrap()).unwrap();
    plan["max_parallel"].clone()
}

/// The `max_parallel` of the plan made with the flag, variable and file values given.
fn resolved(flag: Option<&str>, variable: Option<&str>, file: Option<&str>) -> Value {
    let files = shop();
    let args: Vec<&str> = flag
        .map(|value| vec!["--max-parallel", value])
        .unwrap_or_default();
    let vars: Vec<(&str, &str)> = variable
        .map(|value| vec![("MERGUS_MAX_PARALLEL", value)])
        .unwrap_or_default();
    let config = file.map(|value| format!("max_parallel = {value}\n"));
    let outcome = plan(&files, &args, &vars, config.as_deref());
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    written_max_parallel(&files)
}

#[test]
fn mg_req_1309_settings_precedence() {
    // Flags, then variables, then the user configuration file, then the default; the
    // `project.toml` levels have no `max_parallel` key in phase 0.2.
    assert_eq!(resolved(Some("1"), Some("2"), Some("3")), 1);
    assert_eq!(resolved(None, Some("2"), Some("3")), 2);
    assert_eq!(resolved(None, None, Some("3")), 3);
    assert_eq!(resolved(None, None, None), 4);
    assert_eq!(resolved(Some("5"), None, None), 5);
}

#[test]
fn mg_req_1351_settings_sources_and_file() {
    // A missing file is empty: the default applies.
    assert_eq!(resolved(None, None, None), 4);
    // The file is `<config dir>/mergus/config.toml`, read through the `Environment` port.
    assert_eq!(resolved(None, None, Some("6")), 6);
    // An unknown key is an error with exit code 1, even when a flag sets the value.
    let files = shop();
    let unknown = plan(
        &files,
        &["--max-parallel", "2"],
        &[],
        Some("max_paralel = 2\n"),
    );
    assert_eq!(unknown.exit, 1);
    let diagnostic = &unknown.of("diagnostic")[0];
    assert_eq!(diagnostic["code"], "MG-1313");
    assert!(
        diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("max_paralel")
    );
    let not_toml = plan(&shop(), &[], &[], Some("max_parallel = = 2\n"));
    assert_eq!(not_toml.of("diagnostic")[0]["code"], "MG-1313");
    // An integer from 1, wherever it comes from.
    let invalid: [Inputs<'_>; 4] = [
        (&["--max-parallel", "0"], &[], None),
        (&["--max-parallel", "two"], &[], None),
        (&[], &[("MERGUS_MAX_PARALLEL", "-1")], None),
        (&[], &[], Some("max_parallel = \"3\"\n")),
    ];
    for (args, vars, config) in invalid {
        let files = shop();
        let outcome = plan(&files, args, vars, config);
        assert_eq!(outcome.exit, 1, "{args:?} {vars:?} {config:?}");
        assert_eq!(outcome.of("diagnostic")[0]["code"], "MG-1312");
        assert!(!files.exists(&Path::new(ROOT).join(".mergus")).unwrap());
    }
}
