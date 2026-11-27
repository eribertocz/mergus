//! What running a macro file in a sandboxed session shows (MG-REQ-0641, MG-REQ-0642).

/// A macro of the catalog: a row of `duckdb_functions()` with `internal = false` and
/// `function_type` `macro` or `table_macro` (MG-REQ-0641.3).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CatalogMacro {
    /// The name of the macro, as DuckDB gives it.
    pub name: String,
    /// Whether it is a table macro (`table_macro`) rather than a scalar one (`macro`).
    pub table: bool,
    /// Its `macro_definition`, as DuckDB gives it.
    pub definition: String,
}

/// What one macro file did in its own fresh session (MG-REQ-0641.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MacroRun {
    /// The names of the `internal = true` rows of `duckdb_functions()` before the file ran,
    /// sorted, without repeats (MG-REQ-0642.2).
    pub builtins: Vec<String>,
    /// DuckDB's message when it refused a statement of the file; its later statements did not
    /// run.
    pub refused: Option<String>,
    /// The rows of `duckdb_tables()`, `duckdb_views()`, `duckdb_sequences()`,
    /// `duckdb_types()`, `duckdb_indexes()` and `duckdb_schemas()` that differ after the file
    /// ran, each as `<function>: <row>`, sorted (MG-REQ-0641.2).
    pub changed: Vec<String>,
    /// The macros of the session after the file ran, sorted by name, then definition
    /// (MG-REQ-0641.3).
    pub macros: Vec<CatalogMacro>,
}
