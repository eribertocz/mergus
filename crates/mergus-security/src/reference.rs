//! Secret references: `secret:<name>` (MG-REQ-1001).

use std::fmt;

/// The prefix of every secret reference.
const PREFIX: &str = "secret:";

/// A reference `secret:<name>`, where `<name>` is a valid name (MG-REQ-0305). It holds the
/// name of a secret, never its value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SecretRef {
    name: String,
}

impl SecretRef {
    /// Returns the reference written `text`, or `None` when `text` is not `secret:` followed
    /// by a valid name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let name = text.strip_prefix(PREFIX)?;
        mergus_project::is_valid_name(name).then(|| Self {
            name: name.to_owned(),
        })
    }

    /// Returns the name of the secret.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the environment variable that answers first: `MERGUS_SECRET_<NAME>`, `<NAME>`
    /// being the name in ASCII upper case (MG-REQ-1001.1).
    #[must_use]
    pub fn variable(&self) -> String {
        format!("MERGUS_SECRET_{}", self.name.to_ascii_uppercase())
    }
}

/// Shows the reference as written: `secret:<name>`.
impl fmt::Display for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{PREFIX}{}", self.name)
    }
}
