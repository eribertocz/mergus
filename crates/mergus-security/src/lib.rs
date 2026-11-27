//! Secret resolution, PII handling, hash chains.
//!
//! A secret reference is `secret:<name>` ([`SecretRef`]). [`resolve`] looks it up in the
//! environment variable `MERGUS_SECRET_<NAME>`, then in the operating system keychain under
//! the service `mergus` and the account `<name>`, through the ports given by the caller
//! (MG-REQ-1001). The value comes back as a [`SecretString`], which cannot be printed,
//! formatted, serialized or logged (MG-REQ-1002). No `.env` file is ever read (MG-REQ-1005).
//!
//! [`origin`] follows the same chain and returns which link answered, a [`SecretOrigin`],
//! without the value, for `mergus doctor` (MG-REQ-1004).
//!
//! [`python_environment`] builds the environment of the Python process: the current one
//! without any `MERGUS_SECRET_*` variable or [`CLOUD_CREDENTIAL_VARIABLES`] entry
//! (MG-REQ-1003), so no secret reaches Python (MG-REQ-0006).
//!
//! [`store`] writes a value into the keychain entry of a reference, for `mergus secret set`
//! (MG-REQ-1007); [`check`] makes the same checks without writing, for `--dry-run`.

#![forbid(unsafe_code)]

mod error;
mod python;
mod reference;
mod resolve;
mod store;

pub use error::SecretError;
pub use python::{CLOUD_CREDENTIAL_VARIABLES, python_environment};
pub use reference::SecretRef;
pub use resolve::{KEYCHAIN_SERVICE, SecretOrigin, origin, resolve};
pub use secrecy::{ExposeSecret, SecretString};
pub use store::{StoreError, check, store};
