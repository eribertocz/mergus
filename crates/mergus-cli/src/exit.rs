//! The exit codes and whether each one is retryable (`contracts/events.md` section 2,
//! MG-REQ-0262).

/// Success.
pub(crate) const SUCCESS: u8 = 0;

/// A run cancelled by its cancel token (MG-REQ-0708.2).
pub(crate) const CANCELLED: u8 = mergus_runtime::CANCELLED_EXIT_CODE;

/// An internal error of Mergus (MG-1304).
pub(crate) const INTERNAL: u8 = 10;

/// Returns whether a command that ended with exit `code` may succeed when run again, as the
/// table states it; `None` for a code the table does not list, and for 7, whose answer
/// depends on why the command was cancelled.
pub(crate) fn retryable(code: u8) -> Option<bool> {
    match code {
        0..=2 | 5 | 6 | 10..=19 => Some(false),
        3 | 4 => Some(true),
        _ => None,
    }
}
