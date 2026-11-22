//! Tests of the literal-secret check of configuration files (MG-REQ-0320, MG-REQ-0363).

use std::path::Path;

use mergus_ports::FileSystem;
use mergus_project::{
    LayoutError, ProjectError, read_configuration, read_layout, read_model_options,
};
use mergus_sim::SimFileSystem;

const ROOT: &str = "/project";

/// A simulated project holding `files`, given relative to the root.
fn project(files: &[(&str, &str)]) -> Result<SimFileSystem, LayoutError> {
    let fs = SimFileSystem::new();
    for (path, contents) in files {
        let path = Path::new(ROOT).join(path);
        if let Some(parent) = path.parent() {
            fs.create_dir_all(parent)?;
        }
        fs.write(&path, contents.as_bytes())?;
    }
    Ok(fs)
}

/// Keeps the literal-secret problems of a read; a valid read has none.
fn only_secrets<T>(read: Result<T, LayoutError>) -> Result<Vec<ProjectError>, LayoutError> {
    match read {
        Ok(_) => Ok(Vec::new()),
        Err(LayoutError::Invalid(problems)) => Ok(problems
            .into_iter()
            .filter(|problem| problem.code() == "MG-0309")
            .collect()),
        Err(other) => Err(other),
    }
}

/// The literal-secret problems of reading `project.toml` and `sources/*.toml`.
fn secrets(files: &[(&str, &str)]) -> Vec<ProjectError> {
    project(files)
        .and_then(|fs| only_secrets(read_configuration(&fs, Path::new(ROOT))))
        .unwrap_or_else(|error| {
            vec![ProjectError::InvalidToml {
                path: String::new(),
                line: 0,
                message: error.to_string(),
            }]
        })
}

fn literal(path: &str, line: usize, key: &str) -> ProjectError {
    ProjectError::LiteralSecret {
        path: path.to_owned(),
        line,
        key: key.to_owned(),
    }
}

/// The secrets found in a source file holding `toml`.
fn in_source(toml: &str) -> Vec<ProjectError> {
    secrets(&[
        ("project.toml", "name = \"sales\"\n"),
        ("sources/crm.toml", toml),
    ])
}

#[test]
fn mg_req_0320_literal_secret_rejected() {
    let found = secrets(&[(
        "project.toml",
        "name = \"sales\"\n\n[gold]\nlocation = \"s3://key:hunter2@bucket/gold\"\n",
    )]);
    assert_eq!(found, [literal("project.toml", 4, "gold.location")]);
    let error = &found[0];
    assert_eq!(error.code(), "MG-0309");
    let message = error.to_string();
    assert!(!message.contains("hunter2"), "{message}");
    assert!(message.contains("secret:<name>"), "{message}");
    assert!(message.contains("project.toml") && message.contains("line 4"));
    assert!(message.contains("gold.location"), "{message}");
}

#[test]
fn mg_req_0363_literal_secrets_detected() {
    url_passwords();
    private_keys();
    secret_keys();
    arrays_and_references();
}

/// MG-REQ-0363.1: a URL with a password in its user information.
fn url_passwords() {
    assert_eq!(
        in_source("url = \"postgres://app:pa55@db.local:5432/crm\"\n"),
        [literal("sources/crm.toml", 1, "url")]
    );
    assert_eq!(in_source("url = \"postgres://app@db.local/crm\"\n"), []);
    assert_eq!(in_source("url = \"https://db.local/a:b@c\"\n"), []);
}

/// MG-REQ-0363.2: a private key block, on one line. The marker is built at run time so that
/// this file holds no private key header (MG-REQ-0179).
fn private_keys() {
    let begin = "-----BEGIN";
    let end = "PRIVATE KEY-----";
    assert_eq!(
        in_source(&format!("cert = \"\"\"\n{begin} RSA {end}\nMIIE\n\"\"\"\n")),
        [literal("sources/crm.toml", 1, "cert")]
    );
    assert_eq!(
        in_source(&format!("cert = \"{begin} CERTIFICATE-----\\n{end}\"\n")),
        []
    );
}

/// MG-REQ-0363.3: a key named like a secret, compared without case, with a value.
fn secret_keys() {
    for key in [
        "password",
        "db_passwd",
        "Client_Secret",
        "TOKEN",
        "api_key",
        "apikey",
        "ssh_private_key",
    ] {
        assert_eq!(
            in_source(&format!("{key} = \"x\"\n")),
            [literal("sources/crm.toml", 1, key)],
            "{key}"
        );
    }
    assert_eq!(in_source("password = \"\"\n"), []);
    assert_eq!(in_source("token_ttl = \"1h\"\n"), []);
    // Values that are not strings are never literal secrets.
    assert_eq!(in_source("token = 12\npassword = true\n"), []);
}

/// Arrays and inline tables are checked, and a `secret:` reference passes, with case.
fn arrays_and_references() {
    // The key of an array item is the key of its array.
    assert_eq!(
        in_source("tokens = [\"a\"]\napi_key = [\"\", \"b\"]\nauth = { token = \"c\" }\n"),
        [
            literal("sources/crm.toml", 2, "api_key[1]"),
            literal("sources/crm.toml", 3, "auth.token"),
        ]
    );
    assert_eq!(
        in_source("password = \"secret:crm_password\"\nurl = \"secret:crm_url\"\n"),
        []
    );
    assert_eq!(
        in_source("password = \"Secret:crm\"\n"),
        [literal("sources/crm.toml", 1, "password")]
    );
}

#[test]
fn mg_req_0363_folder_files_are_checked() {
    let fs = project(&[
        ("project.toml", "name = \"sales\"\n"),
        ("models/sales/orders.sql", "select 1\n"),
        ("models/sales/mergus.toml", "[all]\ntoken = \"abc\"\n"),
    ])
    .unwrap();
    let layout = read_layout(&fs, Path::new(ROOT)).unwrap();
    let found = only_secrets(read_model_options(&fs, Path::new(ROOT), &layout)).unwrap();
    assert_eq!(found, [literal("models/sales/mergus.toml", 2, "all.token")]);
}
