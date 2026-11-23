//! `mergus init`: three questions with defaults, a valid project, three next steps
//! (MG-REQ-1301, MG-REQ-1349).

use mergus_project::ConfigKind;

use crate::args::Invocation;
use crate::commands::PROFILES;
use crate::error::CliError;
use crate::files::{Entry, Plan, toml_string};
use crate::name::PRODUCT;
use crate::output::Output;
use crate::run::Ports;
use crate::scaffold::valid;

/// The default gold location (MG-REQ-1349.1).
const LOCATION: &str = "gold";

/// The text of `.gitignore` (MG-REQ-1349.2).
const GITIGNORE: &str = ".mergus/\n";

/// The plan of `mergus init` in the current folder: `project.toml`, `models/`, `sources/`
/// and `.gitignore`. Each question whose option is not given is asked when standard input is
/// a terminal and `--yes` is not given; otherwise its default answers it.
///
/// # Errors
///
/// Fails when the current folder cannot be known, when an answer is not valid, and when
/// `project.toml` or `.gitignore` exists.
pub(crate) fn init(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Plan, CliError> {
    let root = ports
        .environment
        .current_dir()
        .map_err(|error| CliError::Unreadable(error.to_string()))?;
    let folder = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let input = ports.input.filter(|_| !invocation.flag("--yes"));
    let mut answer = |option: &str, question: &str, default: &str| -> Result<String, CliError> {
        if let Some(given) = invocation.value(option) {
            return Ok(given.to_owned());
        }
        let Some(input) = input else {
            return Ok(default.to_owned());
        };
        output.ask(&format!("{question} [{default}]: "))?;
        let typed = input.line().unwrap_or_default();
        let typed = typed.trim();
        Ok(if typed.is_empty() { default } else { typed }.to_owned())
    };
    let name = answer("--name", "Project name", &default_name(&folder))?;
    let profile = answer("--profile", "Storage profile", PROFILES[0])?;
    let location = answer("--location", "Gold location", LOCATION)?;
    valid("project", &name)?;
    if !PROFILES.contains(&profile.as_str()) {
        return Err(CliError::InvalidValue {
            argument: "--profile",
            given: profile,
            closest: PROFILES[0],
        });
    }
    let project = format!(
        "#:schema {}\nname = {}\n\n[gold]\nprofile = {}\nlocation = {}\n",
        ConfigKind::Project.schema_id(),
        toml_string(&name),
        toml_string(&profile),
        toml_string(&location),
    );
    let plan = Plan {
        root,
        entries: vec![
            Entry::File("project.toml".to_owned(), project),
            Entry::Folder("models".to_owned()),
            Entry::Folder("sources".to_owned()),
            Entry::File(".gitignore".to_owned(), GITIGNORE.to_owned()),
        ],
    };
    plan.check(ports.files)?;
    Ok(plan)
}

/// The next steps `init` ends with (MG-REQ-1349.3).
pub(crate) fn next_steps() -> String {
    ["new model first_model", "validate", "plan"]
        .iter()
        .fold("Next steps:\n".to_owned(), |text, step| {
            format!("{text}  {PRODUCT} {step}\n")
        })
}

/// The default project name: the folder name, lowercased, every character outside
/// `[a-z0-9_]` turned into `_` (MG-REQ-1349.1).
fn default_name(folder: &str) -> String {
    folder
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'a'..='z' | '0'..='9' | '_' => character,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::default_name;

    #[test]
    fn default_names_are_lowercase_identifiers_of_the_folder() {
        assert_eq!(default_name("My Sales-2"), "my_sales_2");
        assert_eq!(default_name("Café"), "caf_");
    }
}
