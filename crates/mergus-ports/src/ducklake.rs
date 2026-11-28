//! The DuckLake catalogs a connection of the `Database` port attaches.

/// A DuckLake catalog to attach to a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuckLakeCatalog {
    /// The name the catalog is attached as.
    pub alias: String,
    /// The metadata database of the catalog, such as a DuckDB file.
    pub metadata: String,
    /// The directory or object-store prefix of the catalog's data files.
    pub data_path: String,
}

/// How a connection attaches a DuckLake catalog: read-write at its latest snapshot by
/// default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DuckLakeAccess {
    /// Whether the catalog is attached `READ_ONLY`, so that nothing can be written to it.
    pub read_only: bool,
    /// The snapshot the catalog is attached at, as `SNAPSHOT_VERSION`; `None` attaches its
    /// latest snapshot.
    pub snapshot: Option<i64>,
}
