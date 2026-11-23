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
//!
//! [`read_renames`] reads `ids.toml` (MG-REQ-0370), and [`Identifiers`] gives each model,
//! source, source table and column its stable identifier (MG-REQ-0209): the stored one, or one
//! derived from its name (MG-REQ-0369), never shared (MG-REQ-0371).
//!
//! [`check_sql_models`] checks each `.sql` model on the tree DuckDB's parser gives through the
//! [`SqlParser`](mergus_ports::SqlParser) port (MG-REQ-0193): it reads only models, declared
//! source tables and seeds (MG-REQ-0373) and is one read-only `SELECT` (MG-REQ-0374).
//! [`read_sql_references`] returns the references of each `.sql` model that resolve.

#![forbid(unsafe_code)]

mod config;
mod error;
mod ids;
mod layout;
mod names;
mod sql;

pub use config::{
    ConfigFile, ConfigKind, Configuration, Effective, ModelOptions, Origin, Setting, Source,
    read_configuration, read_model_options,
};
pub use error::{LayoutError, NameKind, ProjectError};
pub use ids::{IdKind, Identifier, Identifiers, Renames, derive_identifier, read_renames};
pub use layout::{Layout, Model, ModelLanguage, Publication, read_layout};
pub use names::{closest_name, is_valid_name, suggest_valid_name};
pub use sql::{KnownTables, check_sql, check_sql_models, read_sql_references};
