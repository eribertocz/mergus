//! MG-REQ-0321, MG-REQ-1301 and MG-REQ-1349: `mergus init`.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_sim::SimFileSystem;

use super::{Outcome, PROJECT, ROOT, exists, mergus, mergus_in, project, text};

/// The `project.toml` of the defaults in [`ROOT`].
const DEFAULT_PROJECT: &str = "#:schema https://mergus.dev/schemas/project.schema.json\n\
                               name = \"sales\"\n\
                               \n\
                               [gold]\n\
                               profile = \"local\"\n\
                               location = \"gold\"\n";

/// The three next steps, in order.
const NEXT: [&str; 3] = [
    "mergus new model first_model",
    "mergus validate",
    "mergus plan",
];

/// The lines of `out` that are next steps, in order.
fn next_steps(out: &str) -> Vec<String> {
    out.lines()
        .map(str::trim)
        .filter(|line| NEXT.contains(line))
        .map(str::to_owned)
        .collect()
}

/// The number of questions asked on standard error.
fn questions(outcome: &Outcome) -> usize {
    outcome.stderr.matches("]: ").count()
}

#[test]
fn mg_req_0321_schema_line_written() {
    let files = project(&[]);
    assert_eq!(mergus(&files, &["init"]).exit, 0);
    let written = text(&files, "project.toml").unwrap();
    assert_eq!(
        written.lines().next(),
        Some("#:schema https://mergus.dev/schemas/project.schema.json")
    );
    assert_eq!(mergus(&files, &["add", "source", "crm"]).exit, 0);
    let source = text(&files, "sources/crm.toml").unwrap();
    assert_eq!(
        source.lines().next(),
        Some("#:schema https://mergus.dev/schemas/source.schema.json")
    );
}

#[test]
fn mg_req_1301_init_three_questions_three_next_steps() {
    let files = project(&[]);
    let outcome = mergus_in(&files, ROOT, &["init"], Some("shop\n\nlake\n"));
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert_eq!(questions(&outcome), 3, "{}", outcome.stderr);
    for default in ["[sales]", "[local]", "[gold]"] {
        assert!(outcome.stderr.contains(default), "{}", outcome.stderr);
    }
    assert_eq!(next_steps(&outcome.stdout), NEXT);
    let written = text(&files, "project.toml").unwrap();
    assert!(written.contains("name = \"shop\"\n"), "{written}");
    assert!(written.contains("profile = \"local\"\n"), "{written}");
    assert!(written.contains("location = \"lake\"\n"), "{written}");
    let validated = mergus(&files, &["validate"]);
    assert_eq!(validated.exit, 0, "a valid project: {validated:?}");
}

#[test]
fn mg_req_1349_init_defaults_files_and_next_steps() {
    init_defaults_files_and_next_steps_1();
    init_defaults_files_and_next_steps_1_refused();
    init_defaults_files_and_next_steps_2();
    init_defaults_files_and_next_steps_3();
}

/// MG-REQ-1349.1: the defaults, the options that answer the questions, `--yes`.
fn init_defaults_files_and_next_steps_1() {
    let files = SimFileSystem::new();
    let folder = "/work/My Sales-2";
    files.create_dir_all(Path::new(folder)).unwrap();
    let outcome = mergus_in(&files, folder, &["init"], None);
    assert_eq!(outcome.exit, 0, "{outcome:?}");
    assert_eq!(questions(&outcome), 0, "no terminal: no question");
    let written = String::from_utf8(files.read(&Path::new(folder).join("project.toml")).unwrap());
    assert!(written.unwrap().contains("name = \"my_sales_2\"\n"));

    let files = project(&[]);
    let answered = mergus_in(&files, ROOT, &["init", "--yes"], Some("ignored\n"));
    assert_eq!(
        (answered.exit, questions(&answered)),
        (0, 0),
        "{answered:?}"
    );
    assert_eq!(text(&files, "project.toml").unwrap(), DEFAULT_PROJECT);

    let files = project(&[]);
    let args = ["init", "--name", "shop", "--location=lake"];
    let partly = mergus_in(&files, ROOT, &args, Some("\n"));
    assert_eq!((partly.exit, questions(&partly)), (0, 1), "{partly:?}");
    assert!(partly.stderr.contains("[local]"), "the profile is asked");
    let written = text(&files, "project.toml").unwrap();
    assert!(written.contains("name = \"shop\"\n") && written.contains("location = \"lake\"\n"));

    let files = project(&[]);
    let options = [
        "init",
        "--name",
        "shop",
        "--profile",
        "local",
        "--location",
        "a\"b",
    ];
    assert_eq!(mergus_in(&files, ROOT, &options, Some("")).exit, 0);
    let written = text(&files, "project.toml").unwrap();
    assert!(written.contains("location = \"a\\\"b\"\n"), "{written}");
}

/// MG-REQ-1349.1: an answer that is not allowed writes nothing.
fn init_defaults_files_and_next_steps_1_refused() {
    let files = project(&[]);
    let wrong = mergus(&files, &["init", "--profile", "cloud", "--json"]);
    assert_eq!(wrong.exit, 1);
    assert_eq!(wrong.of("diagnostic")[0]["code"], "MG-1301");
    assert!(!exists(&files, "project.toml"));

    let files = project(&[]);
    let invalid = mergus(&files, &["init", "--name", "Sales", "--json"]);
    assert_eq!(invalid.exit, 1);
    assert_eq!(invalid.of("diagnostic")[0]["code"], "MG-1305");
    assert!(!exists(&files, "project.toml"));
}

/// MG-REQ-1349.2: the files written, and a folder that already has a project.
fn init_defaults_files_and_next_steps_2() {
    let files = project(&[]);
    assert_eq!(mergus(&files, &["init"]).exit, 0);
    assert_eq!(text(&files, "project.toml").unwrap(), DEFAULT_PROJECT);
    assert_eq!(text(&files, ".gitignore").unwrap(), ".mergus/\n");
    for folder in ["models", "sources"] {
        assert!(exists(&files, folder), "{folder}");
        assert!(
            files
                .list_dir(&Path::new(ROOT).join(folder))
                .unwrap()
                .is_empty()
        );
    }

    let taken = project(&[("project.toml", PROJECT)]);
    let outcome = mergus(&taken, &["init", "--json"]);
    assert_eq!(outcome.exit, 1, "{outcome:?}");
    assert_eq!(outcome.of("diagnostic")[0]["code"], "MG-1307");
    assert_eq!(text(&taken, "project.toml").unwrap(), PROJECT);
    assert!(!exists(&taken, ".gitignore") && !exists(&taken, "models"));
    let human = mergus(&taken, &["init"]);
    assert_eq!(human.exit, 1);
    assert!(
        human.stderr.starts_with("error[MG-1307]"),
        "{}",
        human.stderr
    );
}

/// MG-REQ-1349.3: the next steps, last and in order, with or without `--json`.
fn init_defaults_files_and_next_steps_3() {
    let outcome = mergus(&project(&[]), &["init"]);
    assert_eq!(next_steps(&outcome.stdout), NEXT);
    let last: Vec<&str> = outcome.stdout.lines().rev().take(3).collect();
    let mut last: Vec<&str> = last.into_iter().map(str::trim).collect();
    last.reverse();
    assert_eq!(last, NEXT);

    let json = mergus(&project(&[]), &["init", "--json"]);
    assert_eq!(json.exit, 0);
    assert_eq!(json.of("run_started")[0]["command"], "init");
    assert_eq!(json.of("run_finished")[0]["exit_code"], 0);
    assert_eq!(
        next_steps(&json.stderr),
        NEXT,
        "events alone on standard output"
    );
}
