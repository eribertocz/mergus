//! A simulated environment: arguments, current directory, variables and configuration and
//! cache directories set by the test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mergus_ports::{Environment, PortError};

/// Environment variables and a configuration directory given at creation; no arguments and
/// no current directory and no cache directory until [`SimEnvironment::with_args`],
/// [`SimEnvironment::with_current_dir`] and [`SimEnvironment::with_cache_dir`] set them.
#[derive(Clone, Debug, Default)]
pub struct SimEnvironment {
    args: Vec<String>,
    current_dir: Option<PathBuf>,
    vars: BTreeMap<String, String>,
    config_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
}

impl SimEnvironment {
    /// Returns an environment with `vars` and the configuration directory `config_dir`.
    #[must_use]
    pub fn new<I, K, V>(vars: I, config_dir: Option<PathBuf>) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            args: Vec::new(),
            current_dir: None,
            vars: vars
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            config_dir,
            cache_dir: None,
        }
    }

    /// Returns this environment with the command-line arguments `args`, after the program
    /// name.
    #[must_use]
    pub fn with_args<I, A>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = A>,
        A: Into<String>,
    {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Returns this environment with the current directory `current_dir`.
    #[must_use]
    pub fn with_current_dir(mut self, current_dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(current_dir.into());
        self
    }

    /// Returns this environment with the user cache directory `cache_dir`.
    #[must_use]
    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }
}

impl Environment for SimEnvironment {
    fn args(&self) -> Vec<String> {
        self.args.clone()
    }

    /// Fails with [`PortError::NotFound`] when no current directory was set.
    fn current_dir(&self) -> Result<PathBuf, PortError> {
        self.current_dir
            .clone()
            .ok_or_else(|| PortError::NotFound("the current directory".to_owned()))
    }

    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }

    fn vars(&self) -> Vec<(String, String)> {
        self.vars
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }

    fn config_dir(&self) -> Option<PathBuf> {
        self.config_dir.clone()
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.cache_dir.clone()
    }
}
