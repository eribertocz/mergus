//! Fingerprints (MG-REQ-0711, MG-REQ-0743).

use std::collections::BTreeMap;
use std::fmt::Write;

use mergus_contracts::canonical_json;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Everything a model's fingerprint is made of, and nothing else (MG-REQ-0743).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FingerprintInputs<'a> {
    /// The model file's text, CRLF turned into LF.
    pub code: &'a str,
    /// The effective option values, without origins.
    pub config: &'a BTreeMap<String, Value>,
    /// Each input name to the upstream fingerprint, the version the caller gives, or `None`.
    pub inputs: &'a BTreeMap<String, Option<String>>,
    /// The run parameters the model uses: none in 0.2.
    pub parameters: &'a BTreeMap<String, Value>,
    /// Each engine name to its version, as the caller gives.
    pub engines: &'a BTreeMap<String, String>,
    /// The version of the output contract: `None` until locked outputs exist.
    pub output_contract: Option<&'a str>,
}

/// Returns the fingerprint of `of`: the lowercase hexadecimal SHA-256 of the canonical JSON
/// of the object `code`, `config`, `inputs`, `parameters`, `engines`, `output_contract`.
#[must_use]
pub fn fingerprint(of: &FingerprintInputs<'_>) -> String {
    let optional = |text: Option<&str>| text.map_or(Value::Null, Value::from);
    let mut object = Map::new();
    object.insert("code".to_owned(), Value::from(of.code));
    object.insert("config".to_owned(), map(of.config, Value::clone));
    object.insert(
        "inputs".to_owned(),
        map(of.inputs, |version| optional(version.as_deref())),
    );
    object.insert("parameters".to_owned(), map(of.parameters, Value::clone));
    object.insert(
        "engines".to_owned(),
        map(of.engines, |version| Value::from(version.as_str())),
    );
    object.insert("output_contract".to_owned(), optional(of.output_contract));
    sha256_hex(canonical_json(&Value::Object(object)).as_bytes())
}

/// Returns the lowercase hexadecimal SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// A JSON object with the entries of `entries`, each value converted by `convert`.
fn map<T>(entries: &BTreeMap<String, T>, convert: impl Fn(&T) -> Value) -> Value {
    Value::Object(
        entries
            .iter()
            .map(|(key, value)| (key.clone(), convert(value)))
            .collect(),
    )
}
