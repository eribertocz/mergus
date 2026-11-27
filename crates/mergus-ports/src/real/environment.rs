//! The environment of the operating-system process.

use std::path::PathBuf;

use super::port_error;
use crate::{Environment, PortError};

/// The command line, current directory and variables of this process.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealEnvironment;

impl Environment for RealEnvironment {
    fn args(&self) -> Vec<String> {
        std::env::args_os()
            .skip(1)
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect()
    }

    fn current_dir(&self) -> Result<PathBuf, PortError> {
        std::env::current_dir().map_err(|error| port_error("the current directory", &error))
    }

    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    /// `XDG_CONFIG_HOME` when it is an absolute path, otherwise `.config` under `HOME`; on
    /// Windows, `APPDATA`.
    fn config_dir(&self) -> Option<PathBuf> {
        if cfg!(windows) {
            return absolute(self.var("APPDATA"));
        }
        absolute(self.var("XDG_CONFIG_HOME"))
            .or_else(|| absolute(self.var("HOME")).map(|home| home.join(".config")))
    }

    /// `LOCALAPPDATA` on Windows; `Library/Caches` under `HOME` on macOS; elsewhere
    /// `XDG_CACHE_HOME` when it is an absolute path, otherwise `.cache` under `HOME`.
    fn cache_dir(&self) -> Option<PathBuf> {
        if cfg!(windows) {
            return absolute(self.var("LOCALAPPDATA"));
        }
        let home = || absolute(self.var("HOME"));
        if cfg!(target_os = "macos") {
            return home().map(|home| home.join("Library").join("Caches"));
        }
        absolute(self.var("XDG_CACHE_HOME")).or_else(|| home().map(|home| home.join(".cache")))
    }
}

/// Returns `value` as a path when it is set and absolute.
fn absolute(value: Option<String>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::RealEnvironment;
    use crate::Environment;

    #[test]
    fn real_environment_knows_the_current_directory_and_the_path() {
        let environment = RealEnvironment;
        assert!(environment.current_dir().unwrap().is_absolute());
        assert!(environment.var("PATH").is_some());
        assert_eq!(environment.var("MERGUS_PORTS_TEST_UNSET_VARIABLE"), None);
        if let Some(cache_dir) = environment.cache_dir() {
            assert!(cache_dir.is_absolute());
        }
        let _ = environment.args();
    }
}
