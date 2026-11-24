//! Settings (MG-REQ-1309, MG-REQ-1351): in phase 0.2 only `max_parallel`, from the flag, the
//! variable, the user configuration file or the default, in that order.

use std::num::NonZeroU32;
use std::path::PathBuf;

use mergus_ports::{Environment, FileSystem, PortError};

use crate::error::CliError;
use crate::name::{PRODUCT, env_prefix};

/// The key of `max_parallel` in the user configuration file.
const MAX_PARALLEL_KEY: &str = "max_parallel";

/// The default `max_parallel` (MG-REQ-1351).
pub(crate) const DEFAULT_MAX_PARALLEL: NonZeroU32 = NonZeroU32::MIN.saturating_add(3);

/// Where the value of a setting comes from (MG-REQ-1309).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// The flag of the command line.
    Flag(&'static str),
    /// An environment variable.
    Variable(String),
    /// The user configuration file.
    UserFile(PathBuf),
    /// The default.
    Default,
}

impl Origin {
    /// How a message names the origin.
    fn describe(&self) -> String {
        match self {
            Self::Flag(flag) => format!("the flag `{flag}`"),
            Self::Variable(name) => format!("the variable `{name}`"),
            Self::UserFile(path) => format!("`{}`", path.display()),
            Self::Default => "the default".to_owned(),
        }
    }
}

/// The value of `max_parallel` and where it comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MaxParallel {
    pub(crate) value: NonZeroU32,
    pub(crate) origin: Origin,
}

/// Resolves `max_parallel`: `flag` is the value of `--max-parallel` when it is given.
///
/// # Errors
///
/// Fails with [`CliError::InvalidSetting`] when the first level that sets it gives a value
/// that is not an integer from 1, and with [`CliError::InvalidUserConfig`] when the user
/// configuration file is not TOML or has an unknown key, whatever level wins.
pub(crate) fn max_parallel(
    flag: Option<(&'static str, &str)>,
    environment: &dyn Environment,
    files: &dyn FileSystem,
) -> Result<MaxParallel, CliError> {
    let file = user_file(environment, files)?;
    let variable = format!("{}MAX_PARALLEL", env_prefix());
    let (text, origin) = if let Some((name, value)) = flag {
        (value.to_owned(), Origin::Flag(name))
    } else if let Some(value) = environment.var(&variable) {
        (value, Origin::Variable(variable))
    } else if let Some((value, path)) = file {
        (value, Origin::UserFile(path))
    } else {
        return Ok(MaxParallel {
            value: DEFAULT_MAX_PARALLEL,
            origin: Origin::Default,
        });
    };
    let value = text
        .trim()
        .parse::<NonZeroU32>()
        .map_err(|_| CliError::InvalidSetting {
            origin: origin.describe(),
            given: text.clone(),
        })?;
    Ok(MaxParallel { value, origin })
}

/// The value of `max_parallel` in the user configuration file, as text, and the path of the
/// file; `None` when there is no file or it does not set the key. A missing file is empty.
fn user_file(
    environment: &dyn Environment,
    files: &dyn FileSystem,
) -> Result<Option<(String, PathBuf)>, CliError> {
    let Some(directory) = environment.config_dir() else {
        return Ok(None);
    };
    let path = directory.join(PRODUCT).join("config.toml");
    let bytes = match files.read(&path) {
        Ok(bytes) => bytes,
        Err(PortError::NotFound(_)) => return Ok(None),
        Err(error) => return Err(CliError::Unreadable(error.to_string())),
    };
    let invalid = |message: String| CliError::InvalidUserConfig {
        path: path.display().to_string(),
        message,
    };
    let text = String::from_utf8(bytes).map_err(|_| invalid("it is not UTF-8".to_owned()))?;
    let table: toml::Table = toml::from_str(&text).map_err(|error| invalid(error.to_string()))?;
    if let Some(key) = table.keys().find(|key| *key != MAX_PARALLEL_KEY) {
        return Err(invalid(format!(
            "unknown key `{key}`; the only key is `{MAX_PARALLEL_KEY}`"
        )));
    }
    Ok(table.get(MAX_PARALLEL_KEY).map(|value| {
        let text = match value {
            toml::Value::Integer(number) => number.to_string(),
            toml::Value::String(text) => format!("\"{text}\""),
            other => format!("a {}", other.type_str()),
        };
        (text, path)
    }))
}
