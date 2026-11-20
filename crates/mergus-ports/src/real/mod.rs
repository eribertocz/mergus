//! The real implementations, compiled only with the feature `real` (MG-REQ-0133).
//!
//! Only `mergus-runtime`, `mergus-cli`, `mergus-py` and `mergus-testkit` enable the feature
//! (MG-REQ-0126.5); they create these values and pass them to the crates that need them.

mod clock;
mod file_system;
mod process;

pub use clock::RealClock;
pub use file_system::RealFileSystem;
pub use process::RealProcess;

use std::io;

use crate::PortError;

/// Converts an I/O error about `what` into a [`PortError`].
fn port_error(what: &str, error: &io::Error) -> PortError {
    if error.kind() == io::ErrorKind::NotFound {
        PortError::NotFound(what.to_owned())
    } else {
        PortError::Failed(format!("`{what}`: {error}"))
    }
}
