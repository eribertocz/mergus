//! A simulated environment: variables and a configuration directory set by the test.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mergus_ports::Environment;

/// Environment variables and a configuration directory given at creation.
#[derive(Clone, Debug, Default)]
pub struct SimEnvironment {
    vars: BTreeMap<String, String>,
    config_dir: Option<PathBuf>,
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
            vars: vars
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            config_dir,
        }
    }
}

impl Environment for SimEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }

    fn config_dir(&self) -> Option<PathBuf> {
        self.config_dir.clone()
    }
}
