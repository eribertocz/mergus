//! The files and folders a command creates, checked before any is written, listed by
//! `--dry-run` (MG-REQ-1350.3).

use std::path::{Path, PathBuf};

use mergus_ports::FileSystem;

use crate::error::CliError;

/// One thing to create, its path relative to the root of the plan, with `/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// A folder, and every missing parent.
    Folder(String),
    /// A file with its text; its missing parent folders are created with it.
    File(String, String),
}

/// What a command creates under `root`, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) root: PathBuf,
    pub(crate) entries: Vec<Entry>,
}

impl Plan {
    /// Fails with [`CliError::FileExists`] for the first file that already exists.
    ///
    /// # Errors
    ///
    /// Also fails with [`CliError::Unreadable`] when the file system cannot tell.
    pub(crate) fn check(&self, files: &dyn FileSystem) -> Result<(), CliError> {
        for entry in &self.entries {
            if let Entry::File(path, _) = entry
                && exists(files, &self.root.join(path))?
            {
                return Err(CliError::FileExists { path: path.clone() });
            }
        }
        Ok(())
    }

    /// The lines `--dry-run` prints: every file, and every folder that does not exist yet
    /// followed by `/`.
    ///
    /// # Errors
    ///
    /// Fails with [`CliError::Unreadable`] when the file system cannot tell whether a folder
    /// exists.
    pub(crate) fn listing(&self, files: &dyn FileSystem) -> Result<String, CliError> {
        let mut text = String::new();
        for entry in &self.entries {
            match entry {
                Entry::File(path, _) => text.push_str(&format!("{path}\n")),
                Entry::Folder(path) => {
                    if !exists(files, &self.root.join(path))? {
                        text.push_str(&format!("{path}/\n"));
                    }
                }
            }
        }
        Ok(text)
    }

    /// Creates every entry, in order.
    ///
    /// # Errors
    ///
    /// Fails with [`CliError::Unwritable`] for the first entry the file system refuses.
    pub(crate) fn apply(&self, files: &dyn FileSystem) -> Result<(), CliError> {
        for entry in &self.entries {
            let (path, result) = match entry {
                Entry::Folder(path) => (path, files.create_dir_all(&self.root.join(path))),
                Entry::File(path, text) => {
                    let full = self.root.join(path);
                    let parent = full.parent().unwrap_or(&self.root);
                    let result = files
                        .create_dir_all(parent)
                        .and_then(|()| files.write(&full, text.as_bytes()));
                    (path, result)
                }
            };
            result.map_err(|error| CliError::Unwritable {
                path: path.clone(),
                message: error.to_string(),
            })?;
        }
        Ok(())
    }
}

fn exists(files: &dyn FileSystem, path: &Path) -> Result<bool, CliError> {
    files
        .exists(path)
        .map_err(|error| CliError::Unreadable(error.to_string()))
}

/// `text` as a TOML basic string, quoted and escaped (TOML 1.0).
pub(crate) fn toml_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            '\r' => quoted.push_str("\\r"),
            control if control.is_control() => {
                quoted.push_str(&format!("\\u{:04X}", u32::from(control)));
            }
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::toml_string;

    #[test]
    fn toml_strings_are_escaped() {
        assert_eq!(toml_string("gold"), "\"gold\"");
        assert_eq!(
            toml_string("a\"b\\c\nd\u{7f}"),
            "\"a\\\"b\\\\c\\nd\\u007F\""
        );
    }
}
