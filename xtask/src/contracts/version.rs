//! The semantic version of a contract, written in `schemas/<contract>/v<MAJOR>/VERSION`
//! (MG-REQ-0204).

use std::fmt;

/// The kind of a change to a schema, from the smallest to the largest (MG-REQ-0204).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Bump {
    /// The schema text means the same: no part of the version changes.
    None,
    /// Only a `title` or a `description` changes.
    Patch,
    /// An optional property or a `$defs` entry is added.
    Minor,
}

impl fmt::Display for Bump {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::None => "no",
            Self::Patch => "a patch",
            Self::Minor => "a minor",
        })
    }
}

/// A version `MAJOR.MINOR.PATCH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Semver {
    pub(crate) major: u64,
    pub(crate) minor: u64,
    pub(crate) patch: u64,
}

impl Semver {
    /// The first version of the major version `major`: `<major>.0.0`.
    pub(crate) fn first(major: u64) -> Self {
        Self {
            major,
            minor: 0,
            patch: 0,
        }
    }

    /// Reads the text of a `VERSION` file: one line `MAJOR.MINOR.PATCH`, each part a decimal
    /// number without a leading zero, ended by at most one line break.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let line = text.strip_suffix('\n').unwrap_or(text);
        let mut parts = line.split('.').map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
            let leading_zero = part.len() > 1 && part.starts_with('0');
            (digits && !leading_zero)
                .then(|| part.parse::<u64>().ok())
                .flatten()
        });
        let version = Self {
            major: parts.next()??,
            minor: parts.next()??,
            patch: parts.next()??,
        };
        parts.next().is_none().then_some(version)
    }

    /// The version that follows this one after a change of the kind `bump`.
    pub(crate) fn bumped(self, bump: Bump) -> Self {
        match bump {
            Bump::None => self,
            Bump::Patch => Self {
                patch: self.patch.saturating_add(1),
                ..self
            },
            Bump::Minor => Self {
                minor: self.minor.saturating_add(1),
                patch: 0,
                ..self
            },
        }
    }
}

impl fmt::Display for Semver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Reads the `VERSION` file of the directory `v<major>`: its version, or why it is refused.
///
/// # Errors
///
/// Returns the problem when the text is not one line `MAJOR.MINOR.PATCH`, or when its major
/// part is not `major`.
pub(crate) fn read(text: &str, major: u64) -> Result<Semver, String> {
    let version = Semver::parse(text)
        .ok_or_else(|| format!("`{}` is not one line `MAJOR.MINOR.PATCH`", text.trim_end()))?;
    if version.major == major {
        Ok(version)
    } else {
        Err(format!(
            "`{version}` does not have the major version {major} of its directory `v{major}`"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Bump, Semver, read};

    fn version(major: u64, minor: u64, patch: u64) -> Semver {
        Semver {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn mg_req_0204_a_version_file_holds_one_line_major_minor_patch() {
        assert_eq!(Semver::parse("1.2.3\n"), Some(version(1, 2, 3)));
        assert_eq!(Semver::parse("10.0.20"), Some(version(10, 0, 20)));
        for text in [
            "",
            "1.2",
            "1.2.3.4",
            "1.02.3",
            "v1.2.3",
            "1.2.3\n\n",
            " 1.2.3",
            "1.2.3 ",
            "1..3",
            "1.2.x",
            "1.2.3\r\n",
            "+1.2.3",
        ] {
            assert_eq!(Semver::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn mg_req_0204_the_major_part_is_the_directory_number() {
        assert_eq!(read("2.1.0\n", 2), Ok(version(2, 1, 0)));
        assert_eq!(
            read("1.1.0\n", 2),
            Err("`1.1.0` does not have the major version 2 of its directory `v2`".to_owned())
        );
        assert_eq!(
            read("1.1\n", 1),
            Err("`1.1` is not one line `MAJOR.MINOR.PATCH`".to_owned())
        );
    }

    #[test]
    fn mg_req_0204_a_bump_raises_its_part_and_resets_the_lower_ones() {
        let base = version(1, 2, 3);
        assert_eq!(base.bumped(Bump::None), base);
        assert_eq!(base.bumped(Bump::Patch), version(1, 2, 4));
        assert_eq!(base.bumped(Bump::Minor), version(1, 3, 0));
        assert_eq!(Semver::first(3).to_string(), "3.0.0");
    }
}
