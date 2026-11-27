//! MG-REQ-0006 and MG-REQ-1003: the Python process starts with an environment from which every
//! secret and every cloud SDK credential has been removed.

use mergus_ports::real::{RealEnvironment, RealProcess};
use mergus_ports::{Environment, PortError, Process, ProcessCommand, ProcessOutput};
use mergus_security::{CLOUD_CREDENTIAL_VARIABLES, python_environment};
use mergus_sim::{SimEnvironment, SimProcess};

/// The cloud SDK credential variables of MG-REQ-1003, as the specifier listed them on #241.
const LISTED: [&str; 20] = [
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_SECURITY_TOKEN",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "AWS_CONTAINER_CREDENTIALS_FULL_URI",
    "AWS_CONTAINER_AUTHORIZATION_TOKEN",
    "AZURE_CLIENT_SECRET",
    "AZURE_CLIENT_CERTIFICATE_PATH",
    "AZURE_CLIENT_CERTIFICATE_PASSWORD",
    "AZURE_PASSWORD",
    "AZURE_STORAGE_KEY",
    "AZURE_STORAGE_CONNECTION_STRING",
    "AZURE_STORAGE_SAS_TOKEN",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
    "CLOUDSDK_AUTH_ACCESS_TOKEN",
    "CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE",
    "PGPASSWORD",
    "PGPASSFILE",
];

/// Variables that are neither secrets nor credentials, and must reach Python unchanged.
const KEPT: [(&str, &str); 4] = [
    ("PATH", "/usr/bin"),
    ("HOME", "/home/user"),
    ("MERGUS_PROFILE", "dev"),
    ("AWS_REGION", "eu-west-1"),
];

/// Returns every secret and credential variable, each with a distinct value.
fn secrets() -> Vec<(String, String)> {
    let mut secrets: Vec<(String, String)> = LISTED
        .iter()
        .enumerate()
        .map(|(index, name)| ((*name).to_owned(), format!("credential-value-{index}")))
        .collect();
    for (name, value) in [
        ("MERGUS_SECRET_TOKEN", "secret-value-token"),
        ("MERGUS_SECRET_API_KEY", "secret-value-api-key"),
        ("MERGUS_SECRET_", "secret-value-empty-name"),
        ("Mergus_Secret_Mixed", "secret-value-mixed-case"),
        ("aws_secret_access_key", "credential-value-lower-case"),
    ] {
        secrets.push((name.to_owned(), value.to_owned()));
    }
    secrets
}

/// Returns `environment` with every secret and credential variable added.
fn with_secrets(environment: impl IntoIterator<Item = (String, String)>) -> SimEnvironment {
    SimEnvironment::new(environment.into_iter().chain(secrets()), None)
}

#[test]
fn mg_req_1003_python_env_has_no_credentials() {
    assert_eq!(CLOUD_CREDENTIAL_VARIABLES, LISTED);
    let kept = KEPT.map(|(name, value)| (name.to_owned(), value.to_owned()));
    let environment = with_secrets(kept.clone());
    let mut expected = kept.to_vec();
    expected.sort();
    assert_eq!(python_environment(&environment), expected);
    // An environment with nothing to remove passes through whole.
    let clean = SimEnvironment::new(kept.clone(), None);
    assert_eq!(python_environment(&clean), expected);
    assert_eq!(
        python_environment(&SimEnvironment::default()),
        Vec::<(String, String)>::new()
    );
}

/// Asserts that `text`, what the Python process can read, holds no secret value.
fn assert_no_secret_in(text: &str) {
    for (name, value) in secrets() {
        assert!(!text.contains(&value), "`{name}` reached Python");
    }
}

#[test]
fn mg_req_0006_python_sees_no_secret() {
    let environment = with_secrets(KEPT.map(|(name, value)| (name.to_owned(), value.to_owned())));
    let process = SimProcess::new();
    process.register("python3", ProcessOutput::default());
    let command = ProcessCommand {
        program: "python3".to_owned(),
        args: vec!["-c".to_owned(), "pass".to_owned()],
        env: python_environment(&environment),
        clear_env: true,
        ..ProcessCommand::default()
    };
    process.run(&command).unwrap();
    let runs = process.runs();
    assert_eq!(runs, [command]);
    assert!(
        runs[0].clear_env,
        "the child must not inherit the environment"
    );
    assert_no_secret_in(&format!("{:?}", runs[0]));
    // A real `python3`, when one is installed. In CI (`CI` set), a missing one fails.
    let (env, output) = run_real_python();
    let output = match output {
        Err(PortError::NotFound(_)) if RealEnvironment.var("CI").is_none() => return,
        output => output.unwrap(),
    };
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.code, Some(0), "{stderr}");
    let seen = String::from_utf8(output.stdout).unwrap();
    assert!(seen.lines().any(|line| line.starts_with("PATH=")), "{seen}");
    assert_no_secret_in(&seen);
    for line in seen.lines() {
        let name = line.split_once('=').map_or(line, |(name, _)| name);
        assert!(
            env.iter().any(|(passed, _)| passed == name) || ADDED_BY_PYTHON.contains(&name),
            "Python inherited `{name}`"
        );
    }
}

/// The Python program that prints its whole environment, one `name=value` per line.
const PRINT_ENVIRONMENT: &str =
    "import os, sys; sys.stdout.write(''.join(f'{k}={v}\\n' for k, v in os.environ.items()))";

/// The variables a Python interpreter may add to its own environment: `LC_CTYPE` by locale
/// coercion (PEP 538), `__CF_USER_TEXT_ENCODING` on macOS.
const ADDED_BY_PYTHON: [&str; 2] = ["LC_CTYPE", "__CF_USER_TEXT_ENCODING"];

/// Runs a real `python3` with the clean environment; returns that environment and what the
/// run gave.
fn run_real_python() -> (Vec<(String, String)>, Result<ProcessOutput, PortError>) {
    let real = RealEnvironment;
    let passed = ["PATH", "SYSTEMROOT"]
        .into_iter()
        .filter_map(|name| Some((name.to_owned(), real.var(name)?)));
    let env = python_environment(&with_secrets(passed));
    let command = ProcessCommand {
        program: "python3".to_owned(),
        args: vec![
            "-I".to_owned(),
            "-c".to_owned(),
            PRINT_ENVIRONMENT.to_owned(),
        ],
        env: env.clone(),
        clear_env: true,
        ..ProcessCommand::default()
    };
    (env, RealProcess.run(&command))
}
