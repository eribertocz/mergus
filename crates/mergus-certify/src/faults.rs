//! The crash points the kit injects.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The crash plan of one client. The harness asks [`crashed`](Self::crashed) before every
/// storage call of the client and fails the call when it answers `true`. Once crashed, a
/// client stays crashed, as a stopped process does.
#[derive(Debug)]
pub struct Faults {
    calls: AtomicU64,
    crash_at: AtomicU64,
}

impl Faults {
    /// Returns a plan that never crashes until the kit arms it.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU64::new(0),
            crash_at: AtomicU64::new(u64::MAX),
        })
    }

    /// Counts one storage call and returns whether it must fail.
    pub fn crashed(&self) -> bool {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        call >= self.crash_at.load(Ordering::SeqCst)
    }

    /// Crashes the client once `calls` more storage calls have succeeded.
    pub(crate) fn crash_after(&self, calls: u64) {
        let now = self.calls.load(Ordering::SeqCst);
        self.crash_at
            .store(now.saturating_add(calls), Ordering::SeqCst);
    }

    /// Returns whether the client has crashed.
    pub(crate) fn has_crashed(&self) -> bool {
        self.calls.load(Ordering::SeqCst) > self.crash_at.load(Ordering::SeqCst)
    }
}
