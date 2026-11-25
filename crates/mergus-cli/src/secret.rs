//! `mergus secret set <name>`: the value read from standard input, stored in the keychain
//! (MG-REQ-1007).

use mergus_security::{SecretRef, SecretString, store};

use crate::args::Invocation;
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::output::Output;
use crate::run::{Ports, SecretInput};
use crate::scaffold::valid;

/// Reads the value of the secret named by `invocation` and stores it under the service
/// `mergus` and the account its name. It prints no value.
///
/// # Errors
///
/// Fails with `MG-1305` when the name is not a lowercase identifier, and when the prompt
/// cannot be written. An empty value (`MG-1001`) or a keychain that cannot be written
/// (`MG-1002`) is returned as a diagnostic.
pub(crate) fn secret_set(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Vec<Diagnostic>, CliError> {
    let name = valid("secret", invocation.value("name").unwrap_or_default())?;
    let reference = SecretRef::from_name(name)
        .ok_or_else(|| CliError::Internal(format!("`{name}` is valid but is not a secret name")))?;
    let value = SecretString::from(read(ports.secret_input, name, output)?);
    Ok(match store(&reference, &value, ports.keychain) {
        Ok(()) => Vec::new(),
        Err(error) => vec![Diagnostic::of_store(&error)],
    })
}

/// The value: on a terminal, the line typed after the prompt (MG-REQ-1007.2); otherwise all
/// of standard input without one trailing line ending (MG-REQ-1007.1). Input that cannot be
/// read gives an empty value, which is refused.
fn read(input: &dyn SecretInput, name: &str, output: &mut Output<'_>) -> Result<String, CliError> {
    if input.is_terminal() {
        output.ask(&format!("Value of secret {name}: "))?;
        return Ok(input.hidden_line().unwrap_or_default());
    }
    let mut value = input.all().unwrap_or_default();
    if value.ends_with('\n') {
        value.pop();
        if value.ends_with('\r') {
            value.pop();
        }
    }
    Ok(value)
}
