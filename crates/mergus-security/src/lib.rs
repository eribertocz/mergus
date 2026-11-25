//! Secret resolution, PII handling, hash chains.
//!
//! A secret reference is `secret:<name>` ([`SecretRef`]). [`resolve`] looks it up in the
//! environment variable `MERGUS_SECRET_<NAME>`, then in the operating system keychain under
//! the service `mergus` and the account `<name>`, through the ports given by the caller
//! (MG-REQ-1001). The value comes back as a [`SecretString`], which cannot be printed,
//! formatted, serialized or logged (MG-REQ-1002). No `.env` file is ever read (MG-REQ-1005).
//!
//! [`store`] writes a value into the keychain entry of a reference, for `mergus secret set`
//! (MG-REQ-1007).

#![forbid(unsafe_code)]

mod error;
mod reference;
mod resolve;
mod store;

pub use error::SecretError;
pub use reference::SecretRef;
pub use resolve::{KEYCHAIN_SERVICE, resolve};
pub use secrecy::{ExposeSecret, SecretString};
pub use store::{StoreError, store};
