//! The environment of the Python process (MG-REQ-0006, MG-REQ-1003).

use mergus_ports::Environment;

/// The prefix of the variables that hold secret values (MG-REQ-1001.1).
const SECRET_PREFIX: &str = "MERGUS_SECRET_";

/// The credential variables of known cloud SDKs, removed from the environment of the Python
/// process (MG-REQ-1003): AWS, Azure, Google and PostgreSQL clients.
pub const CLOUD_CREDENTIAL_VARIABLES: [&str; 20] = [
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

/// Returns the environment the Python process starts with (MG-REQ-1003): every variable of
/// `environment`, sorted by name, except each `MERGUS_SECRET_*` variable and each variable of
/// [`CLOUD_CREDENTIAL_VARIABLES`]. Names are compared in ASCII upper case, because Windows
/// reads variable names without regard to case.
///
/// The process must start with only these variables (`ProcessCommand::clear_env`), so that
/// nothing is inherited around them (MG-REQ-0006).
#[must_use]
pub fn python_environment(environment: &dyn Environment) -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> = environment
        .vars()
        .into_iter()
        .filter(|(name, _)| !is_credential(name))
        .collect();
    vars.sort();
    vars
}

/// Whether the variable `name` may hold a secret or a cloud SDK credential.
fn is_credential(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with(SECRET_PREFIX) || CLOUD_CREDENTIAL_VARIABLES.contains(&upper.as_str())
}
