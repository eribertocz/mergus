//! Data version references, pinned-read recipes and retention.

use std::time::Duration;

/// The reference of one data version, as the storage names it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VersionRef(String);

impl VersionRef {
    /// Wraps the storage's name of a version.
    #[must_use]
    pub fn new(reference: impl Into<String>) -> Self {
        Self(reference.into())
    }

    /// Returns the storage's name of the version.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How to read one version pinned, as the adapter describes it; stored in the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PinRecipe(String);

impl PinRecipe {
    /// Wraps the adapter's description.
    #[must_use]
    pub fn new(recipe: impl Into<String>) -> Self {
        Self(recipe.into())
    }

    /// Returns the adapter's description.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The versions [`expire`](crate::StorageAdapter::expire) must keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retention {
    /// The versions referenced by a live manifest.
    pub live: Vec<VersionRef>,
    /// Versions younger than this are inside the rollback window.
    pub rollback_window: Duration,
}
