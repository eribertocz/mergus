//! SHA-256, the one hash Mergus uses for identifiers, fingerprints and `plan_id`, behind a
//! Mergus interface (MG-REQ-0186). It is computed by `sha2` (RustCrypto).

use std::fmt::Write;

use sha2::{Digest, Sha256};

/// Returns the SHA-256 of `bytes`.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Returns the SHA-256 of `bytes` in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}
