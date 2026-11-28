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
