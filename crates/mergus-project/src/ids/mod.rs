//! Stable identifiers of project objects (MG-REQ-0209): their format (MG-REQ-0368), their
//! derivation from names (MG-REQ-0369), the renames of `ids.toml` (MG-REQ-0370) and the check
//! that no identifier is shared (MG-REQ-0371).

mod assign;
mod file;

pub use assign::Identifiers;
pub use file::{Renames, read_renames};

use std::fmt;

use sha2::{Digest, Sha256};

/// The lowercase RFC 4648 base32 alphabet.
const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

/// The number of base32 characters after the prefix and `_` (MG-REQ-0368).
const BODY_LENGTH: usize = 16;

/// The number of bytes of the SHA-256 that are encoded (MG-REQ-0369).
const HASH_BYTES: usize = 10;

/// The kind of an identified object, which chooses its prefix (MG-REQ-0368).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdKind {
    /// A model and its table: `mdl`.
    Model,
    /// A source: `src`.
    Source,
    /// A table of a source: `stb`.
    SourceTable,
    /// A column of a model or of a source table: `col`.
    Column,
}

impl IdKind {
    /// Every kind.
    const ALL: [Self; 4] = [Self::Model, Self::Source, Self::SourceTable, Self::Column];

    /// Returns the prefix of the identifiers of this kind.
    #[must_use]
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Model => "mdl",
            Self::Source => "src",
            Self::SourceTable => "stb",
            Self::Column => "col",
        }
    }
}

/// A valid identifier: a kind prefix, `_` and 16 lowercase base32 characters (MG-REQ-0368).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
    text: String,
    kind: IdKind,
}

impl Identifier {
    /// Returns the identifier written `text`, or `None` when `text` is not valid under
    /// MG-REQ-0368.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (prefix, body) = text.split_once('_')?;
        let kind = IdKind::ALL
            .into_iter()
            .find(|kind| kind.prefix() == prefix)?;
        let valid = body.len() == BODY_LENGTH && body.bytes().all(|byte| ALPHABET.contains(&byte));
        valid.then(|| Self {
            text: text.to_owned(),
            kind,
        })
    }

    /// Returns the kind of the object the identifier names.
    #[must_use]
    pub fn kind(&self) -> IdKind {
        self.kind
    }

    /// Returns the identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Identifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

/// Returns the identifier of MG-REQ-0369 for one value of `<n>`: the prefix of `kind`, `_`
/// and the base32 of the first 10 bytes of the SHA-256 of `<prefix>\n<parent>\n<name>\n<n>`,
/// `<parent>` being empty when `parent` is `None`.
#[must_use]
pub fn derive_identifier(
    kind: IdKind,
    parent: Option<&Identifier>,
    name: &str,
    n: u64,
) -> Identifier {
    let prefix = kind.prefix();
    let parent = parent.map_or("", Identifier::as_str);
    let digest = Sha256::digest(format!("{prefix}\n{parent}\n{name}\n{n}").as_bytes());
    let mut bytes = [0_u8; HASH_BYTES];
    for (byte, hashed) in bytes.iter_mut().zip(digest.iter()) {
        *byte = *hashed;
    }
    Identifier {
        text: format!("{prefix}_{}", base32(&bytes)),
        kind,
    }
}

/// Encodes `bytes` as lowercase RFC 4648 base32 without padding. Written here rather than
/// taken from a crate (MG-REQ-0185): 10 bytes always give exactly 16 characters.
fn base32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let mut buffer: u16 = 0;
    let mut bits: u32 = 0;
    for byte in bytes {
        buffer = (buffer << 8) | u16::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(symbol(buffer >> bits));
        }
        buffer &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(symbol(buffer << (5 - bits)));
    }
    out
}

/// Returns the base32 character of the low 5 bits of `value`.
fn symbol(value: u16) -> char {
    ALPHABET
        .get(usize::from(value & 0x1f))
        .map_or('a', |byte| char::from(*byte))
}

#[cfg(test)]
mod tests {
    use super::base32;

    /// The test vectors of RFC 4648 section 10, lowercased and without padding.
    #[test]
    fn base32_matches_rfc_4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "my"),
            ("fo", "mzxq"),
            ("foo", "mzxw6"),
            ("foob", "mzxw6yq"),
            ("fooba", "mzxw6ytb"),
            ("foobar", "mzxw6ytboi"),
        ] {
            assert_eq!(base32(input.as_bytes()), expected, "{input}");
        }
    }
}
