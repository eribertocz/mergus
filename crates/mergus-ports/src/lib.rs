//! I/O traits and their real implementations.
//!
//! Everything that touches the outside world goes through one of these traits
//! (MG-REQ-0130). Ports are passed explicitly, as constructor or context arguments; this crate
//! has no global or thread-local instance (MG-REQ-0132). The real implementations compile only
//! with the feature `real` (MG-REQ-0133); `mergus-sim` holds the simulated ones (MG-REQ-0131).

#![forbid(unsafe_code)]

mod clock;
mod database;
mod environment;
mod error;
mod file_system;
mod keychain;
mod network;
mod object_store;
mod process;
mod random;
mod session;
mod source;
mod sql_parser;

#[cfg(feature = "real")]
#[allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "MG-REQ-0130: the real implementations are the only code allowed to touch the world"
)]
pub mod real;

pub use clock::{Clock, MonotonicTime, Timestamp};
pub use database::{
    Database, DatabaseConnection, DatabaseLocation, DuckLakeCatalog, Extension, LoadedExtension,
    PostgresSource, ResultColumn, SqlValue,
};
pub use environment::Environment;
pub use error::PortError;
pub use file_system::FileSystem;
pub use keychain::Keychain;
pub use network::{Connection, HttpRequest, HttpResponse, Network};
pub use object_store::ObjectStore;
pub use process::{Process, ProcessCommand, ProcessOutput};
pub use random::Random;
pub use source::{SourceConnector, SourceReader};
pub use sql_parser::SqlParser;
