//! The cancel token of a run (MG-REQ-0708.1).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The exit code of a cancelled run (MG-REQ-0708.2).
pub const CANCELLED_EXIT_CODE: u8 = 7;

/// Whether a run is cancelled. Every clone shares one flag: it is set once, by the CLI on
/// Ctrl-C, and read anywhere, so the same token can be passed down to the engines.
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    /// Returns a token that is not cancelled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels the run. Cancelling twice changes nothing.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns whether the run is cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}
