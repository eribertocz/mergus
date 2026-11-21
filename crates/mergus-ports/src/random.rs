//! The `Random` port: random numbers and identifiers.

/// Random numbers and identifiers.
pub trait Random: Send + Sync {
    /// Returns a random 64-bit number.
    fn next_u64(&self) -> u64;

    /// Fills `bytes` with random bytes.
    fn fill(&self, bytes: &mut [u8]);
}
