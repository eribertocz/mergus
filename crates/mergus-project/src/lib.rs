//! Read and validate the project: files, `project.toml`, folder `mergus.toml`.
//!
//! [`read_layout`] walks a project through the [`FileSystem`](mergus_ports::FileSystem) port
//! given by the caller and returns its [`Layout`]: one [`Model`] per `.sql` or `.py` file
//! under `models/`, named after the file stem (MG-REQ-0302), published as
//! `gold.<schema>.<table>` when it sits at `models/gold/<schema>/<table>` (MG-REQ-0304). File
//! contents are never read, so no comment or decorator has meaning (MG-REQ-0301). Every
//! failure is collected as a typed [`ProjectError`].
//!
//! [`read_configuration`] reads `project.toml` and every `sources/<source>.toml`, checks them
//! against the JSON Schemas of `schemas/` of this crate (MG-REQ-0316), rejects expressions
//! (MG-REQ-0364) and returns each file as JSON with its canonical form (MG-REQ-0318).

#![forbid(unsafe_code)]

mod config;
mod error;
mod layout;
mod names;

pub use config::{ConfigFile, ConfigKind, Configuration, Source, read_configuration};
pub use error::{LayoutError, NameKind, ProjectError};
pub use layout::{Layout, Model, ModelLanguage, Publication, read_layout};
pub use names::{closest_name, is_valid_name, suggest_valid_name};
