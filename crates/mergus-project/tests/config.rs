//! Tests of reading `project.toml` and `sources/*.toml` through the simulated file system.

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{ConfigKind, Configuration, LayoutError, ProjectError, read_configuration};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";
const PROJECT: &str = "name = \"sales\"\n";

/// Reads the configuration of a simulated project holding `files`, given relative to the root.
fn read(files: &[(&str, &str)]) -> Result<Configuration, LayoutError> {
    let fs = SimFileSystem::new();
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents.as_bytes())?;
    }
    read_configuration(&fs, Path::new(ROOT))
}

/// Returns the problems of a project that has some.
macro_rules! problems {
    ($files:expr) => {
        match read($files) {
            Err(LayoutError::Invalid(problems)) => problems,
            other => panic!("expected problems, got {other:?}"),
        }
    };
}

/// Returns the canonical JSON of a valid `project.toml`.
fn canonical(project: &str) -> Result<String, LayoutError> {
    read(&[("project.toml", project)]).map(|configuration| configuration.project.canonical)
}

#[test]
fn mg_req_0306_source_name_is_file_stem() {
    let configuration = read(&[
        ("project.toml", PROJECT),
        ("sources/crm.toml", ""),
        ("sources/erp.toml", "# no key yet\n"),
    ])
    .unwrap();
    let names: Vec<&str> = configuration
        .sources
        .iter()
        .map(|source| source.name.as_str())
        .collect();
    assert_eq!(names, ["crm", "erp"]);
    assert_eq!(configuration.sources[0].file.path, "sources/crm.toml");

    let found = problems!(&[("project.toml", "name = \"sales\"\n\n[sources.crm]\n")]);
    assert_eq!(
        found,
        [ProjectError::UnknownKey {
            path: "project.toml".to_owned(),
            line: 3,
            key: "sources".to_owned(),
            suggestion: None,
        }]
    );
}

#[test]
fn mg_req_0316_unknown_key_rejected_with_suggestion() {
    let found = problems!(&[(
        "project.toml",
        "name = \"sales\"\nmergos = \">=1.4\"\n\n[gold]\nprofil = \"cloud\"\n",
    )]);
    assert_eq!(
        found,
        [
            ProjectError::UnknownKey {
                path: "project.toml".to_owned(),
                line: 2,
                key: "mergos".to_owned(),
                suggestion: Some("mergus".to_owned()),
            },
            ProjectError::UnknownKey {
                path: "project.toml".to_owned(),
                line: 5,
                key: "gold.profil".to_owned(),
                suggestion: Some("profile".to_owned()),
            },
        ]
    );
    assert!(found.iter().all(|problem| problem.code() == "MG-0304"));
    assert!(found[0].to_string().contains("did you mean `mergus`"));

    let wrong_type = problems!(&[("project.toml", "name = \"sales\"\n[raw]\nlocation = 3\n")]);
    assert_eq!(wrong_type.len(), 1);
    assert_eq!(wrong_type[0].code(), "MG-0305");
    assert_eq!(wrong_type[0].line(), Some(3));

    let bad_name = problems!(&[("project.toml", "name = \"Sales\"\n")]);
    assert_eq!(bad_name[0].code(), "MG-0305");

    let not_toml = problems!(&[("project.toml", "name = \"sales\"\nname = \"twice\"\n")]);
    assert_eq!(not_toml[0].code(), "MG-0303");
    assert_eq!(not_toml[0].path(), "project.toml");

    let missing = problems!(&[("project.toml", "mergus = \">=1.4\"\n")]);
    assert_eq!(missing[0].code(), "MG-0305");
}

#[test]
fn mg_req_0317_configuration_has_no_expressions() {
    let project = "name = \"sales\"\n[gold]\nlocation = \"s3://bucket/$run_date/$HOME\"\n";
    let configuration = read(&[("project.toml", project)]).unwrap();
    assert_eq!(
        configuration.project.value["gold"]["location"],
        "s3://bucket/$run_date/$HOME"
    );

    let include = problems!(&[(
        "project.toml",
        "name = \"sales\"\ninclude = \"other.toml\"\n",
    )]);
    assert_eq!(include[0].code(), "MG-0304");
}

#[test]
fn mg_req_0318_formatting_does_not_change_fingerprint() {
    let compact =
        "name=\"sales\"\nmergus=\">=1.4, <2\"\ngold={profile=\"cloud\",location=\"s3://g\"}\n";
    let spread =
        "# The sales project.\n\n[gold]   # gold zone\nlocation = 's3://g'\nprofile  = \"cloud\"\n";
    let spread = format!("mergus = \"\"\">=1.4, <2\"\"\"\nname = \"sales\"\n{spread}");
    let (compact, spread, other) = (
        canonical(compact).unwrap(),
        canonical(&spread).unwrap(),
        canonical("name = \"sales\"\n").unwrap(),
    );
    assert_eq!(compact.as_bytes(), spread.as_bytes());
    assert_ne!(compact, other);
}
#[test]
fn mg_req_0361_configuration_schemas_are_closed() {
    for kind in [ConfigKind::Project, ConfigKind::Source, ConfigKind::Folder] {
        let schema: serde_json::Value = serde_json::from_str(kind.schema()).unwrap();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["$id"], kind.schema_id());
        assert!(
            kind.schema_path()
                .starts_with("crates/mergus-project/schemas/")
        );
        assert!(schema["properties"].get("contract").is_none());
        assert!(schema["properties"].get("contract_version").is_none());
        check_node(&schema, "#", kind == ConfigKind::Source);
    }
}

/// The keywords MG-REQ-0361 allows: those of contract schemas and its additions.
const KEYWORDS: [&str; 19] = [
    "$schema",
    "$id",
    "$defs",
    "title",
    "description",
    "$ref",
    "type",
    "const",
    "enum",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "oneOf",
    "pattern",
    "patternProperties",
    "minimum",
    "maximum",
    "default",
];

/// Checks that a schema node and every node under it is described, closed when it is an
/// object, and uses only allowed keywords.
/// The root of the source schema is the one open object (MG-REQ-0362.3): `open` says so.
fn check_node(node: &serde_json::Value, at: &str, open: bool) {
    assert!(node.is_object(), "{at}: not an object");
    let Some(map) = node.as_object() else {
        return;
    };
    for keyword in map.keys() {
        assert!(
            KEYWORDS.contains(&keyword.as_str()),
            "{at}: keyword `{keyword}`"
        );
    }
    assert!(
        map.get("description")
            .is_some_and(serde_json::Value::is_string),
        "{at}: description"
    );
    if open {
        assert!(map.get("additionalProperties").is_none(), "{at}: open");
        assert!(map.get("properties").is_none(), "{at}: no key yet");
    } else if map.get("type").is_some_and(|kind| kind == "object") {
        assert_eq!(
            map.get("additionalProperties"),
            Some(&serde_json::Value::Bool(false)),
            "{at}"
        );
    }
    for (path, child) in children(map) {
        check_node(child, &format!("{at}/{path}"), false);
    }
}

/// The schema nodes directly under a node, with their path from it.
fn children(map: &serde_json::Map<String, serde_json::Value>) -> Vec<(String, &serde_json::Value)> {
    let mut found = Vec::new();
    for group in ["properties", "patternProperties", "$defs"] {
        let named = map
            .get(group)
            .and_then(|g| g.as_object())
            .into_iter()
            .flatten();
        found.extend(named.map(|(name, child)| (format!("{group}/{name}"), child)));
    }
    found.extend(map.get("items").map(|child| ("items".to_owned(), child)));
    let alternatives = map
        .get("oneOf")
        .and_then(|g| g.as_array())
        .into_iter()
        .flatten();
    found.extend(
        alternatives
            .enumerate()
            .map(|(index, child)| (format!("oneOf/{index}"), child)),
    );
    found
}

#[test]
fn mg_req_0362_only_due_keys_are_known() {
    let project: serde_json::Value = serde_json::from_str(ConfigKind::Project.schema()).unwrap();
    let keys: Vec<&String> = project["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["gold", "mergus", "name", "ops", "raw"]);
    assert_eq!(project["required"], serde_json::json!(["name"]));
    let zone = &project["$defs"]["zone"]["properties"];
    let zone_keys: Vec<&String> = zone.as_object().unwrap().keys().collect();
    assert_eq!(zone_keys, ["location", "profile"]);

    let full = "name = \"sales\"\nmergus = \">=1.4, <2\"\n\
                [gold]\nprofile = \"cloud\"\nlocation = \"s3://g\"\n\
                [raw]\nprofile = \"local\"\nlocation = \"/data/raw\"\n\
                [ops]\nprofile = \"local\"\nlocation = \"/data/ops\"\n";
    assert!(read(&[("project.toml", full)]).is_ok());

    let later = problems!(&[(
        "project.toml",
        "name = \"sales\"\nstrict_integrity = true\n[environments.prod]\n",
    )]);
    let keys: Vec<Option<&str>> = later.iter().map(ProjectError::key).collect();
    assert_eq!(keys, [Some("strict_integrity"), Some("environments")]);

    let source = read(&[
        ("project.toml", PROJECT),
        (
            "sources/crm.toml",
            "kind = \"postgres\"\n[options]\nport = 5432\n",
        ),
    ])
    .unwrap();
    assert_eq!(source.sources[0].file.value["options"]["port"], 5432);
    let broken = problems!(&[("project.toml", PROJECT), ("sources/crm.toml", "kind = \n")]);
    assert_eq!(broken[0].code(), "MG-0303");
    assert_eq!(broken[0].path(), "sources/crm.toml");
}

#[test]
fn mg_req_0364_expressions_rejected() {
    for marker in ["${zone}", "{{ zone }}", "{% if x %}"] {
        let project = format!("name = \"sales\"\n\n[gold]\nlocation = \"s3://{marker}\"\n");
        let found = problems!(&[("project.toml", &project)]);
        assert_eq!(
            found,
            [ProjectError::Expression {
                path: "project.toml".to_owned(),
                line: 4,
                key: "gold.location".to_owned(),
            }]
        );
        assert_eq!(found[0].code(), "MG-0306");
    }
    let source = problems!(&[
        ("project.toml", PROJECT),
        ("sources/crm.toml", "hosts = [\"a\", \"${b}\"]\n"),
    ]);
    assert_eq!(source[0].key(), Some("hosts[1]"));
    assert_eq!(source[0].line(), Some(1));
}

#[test]
fn mg_req_0365_toml_maps_to_canonical_json() {
    let source = "s = \"é\\n\"\nt = true\ni = 0x1F\nn = -7\nf = 1.50\ne = 1e3\nz = -0.0\n\
                  a = [1, \"x\"]\n[tab]\nb = { c = 1 }\n\
                  [dates]\nodt = 1979-05-27 07:32:00.500Z\nzero = 1979-05-27t07:32:00+00:00\n\
                  off = 1979-05-27 07:32:00.123000-07:00\nldt = 1979-05-27T07:32:00\n\
                  ld = 1979-05-27\nlt = 07:32:00.000\n";
    let configuration = read(&[("project.toml", PROJECT), ("sources/crm.toml", source)]).unwrap();
    assert_eq!(
        configuration.sources[0].file.canonical,
        concat!(
            r#"{"a":[1,"x"],"dates":{"ld":"1979-05-27","ldt":"1979-05-27T07:32:00","#,
            r#""lt":"07:32:00","odt":"1979-05-27T07:32:00.5Z","#,
            r#""off":"1979-05-27T07:32:00.123-07:00","zero":"1979-05-27T07:32:00Z"},"#,
            r#""e":1000,"f":1.5,"i":31,"n":-7,"s":"é\n","t":true,"tab":{"b":{"c":1}},"z":0}"#
        )
    );
    for bad in ["f = nan\n", "f = +inf\n", "f = -inf\n"] {
        let found = problems!(&[("project.toml", PROJECT), ("sources/crm.toml", bad)]);
        assert_eq!(found[0].code(), "MG-0307");
        assert_eq!(found[0].key(), Some("f"));
    }
    let toml_1_1 = problems!(&[
        ("project.toml", PROJECT),
        ("sources/crm.toml", "t = 07:32\n"),
    ]);
    assert_eq!(toml_1_1[0].code(), "MG-0303");
}

#[test]
fn mg_req_0005_problems_in_total_order() {
    let found = problems!(&[("project.toml", "name = \"sales\"\nzone = \"${x}\"\n")]);
    let codes: Vec<&str> = found.iter().map(ProjectError::code).collect();
    assert_eq!(codes, ["MG-0304", "MG-0306"]);
}

#[test]
fn toml_to_json_maps_a_document_without_a_schema() {
    let value = mergus_project::toml_to_json("config.toml", "max_parallel = 2\nother = \"x\"\n");
    assert_eq!(
        value.unwrap(),
        serde_json::json!({"max_parallel": 2, "other": "x"})
    );
    let problems = mergus_project::toml_to_json("config.toml", "max_parallel = = 2\n").unwrap_err();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].path(), "config.toml");
}
