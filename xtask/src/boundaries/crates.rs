//! The crate table of `architecture/crates.md`.

use std::fmt;

/// The layer of a crate: a number from 0 to 5, or the test crates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Layer {
    /// A product layer, 0 to 5.
    Number(u8),
    /// `mergus-sim` and `mergus-testkit`.
    Test,
}

impl fmt::Display for Layer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => write!(formatter, "{number}"),
            Self::Test => formatter.write_str("test"),
        }
    }
}

/// One row of the crate table.
pub(crate) struct Crate {
    /// The crate name.
    pub(crate) name: &'static str,
    /// Its layer.
    pub(crate) layer: Layer,
    /// Whether it is a pure core crate (MG-REQ-0004).
    pub(crate) core: bool,
    /// Its one responsibility, written as in the table (MG-REQ-0111).
    pub(crate) responsibility: &'static str,
}

const fn row(name: &'static str, layer: u8, core: bool, responsibility: &'static str) -> Crate {
    Crate {
        name,
        layer: Layer::Number(layer),
        core,
        responsibility,
    }
}

/// Every crate of `architecture/crates.md`, in the order of the table.
const CRATES: [Crate; 22] = [
    row(
        "mergus-contracts",
        0,
        true,
        "Rust types generated from `schemas/`",
    ),
    row(
        "mergus-ports",
        0,
        false,
        "I/O traits and their real implementations",
    ),
    row(
        "mergus-project",
        1,
        true,
        "Read and validate the project: files, `project.toml`, folder `mergus.toml`",
    ),
    row(
        "mergus-plan",
        2,
        true,
        "Resolve, order and fingerprint the work; produce the plan",
    ),
    row("mergus-engine-duckdb", 3, false, "Execute DuckDB steps"),
    row("mergus-engine-polars", 3, false, "Execute Polars steps"),
    row(
        "mergus-source",
        3,
        false,
        "Extract from databases (ADBC) and files",
    ),
    row(
        "mergus-source-rest",
        3,
        false,
        "Extract from REST APIs declared in TOML",
    ),
    row(
        "mergus-adapter",
        3,
        false,
        "Storage adapter contract (public API for third parties)",
    ),
    row(
        "mergus-adapter-ducklake",
        3,
        false,
        "DuckLake storage adapter",
    ),
    row(
        "mergus-catalog",
        3,
        false,
        "Catalog documents, manifests, pointer compare-and-swap",
    ),
    row(
        "mergus-quality",
        3,
        false,
        "Tests, contracts, monitors, gate decisions",
    ),
    row(
        "mergus-ops",
        3,
        false,
        "Journal, ops tables, logs, OpenTelemetry, OpenLineage",
    ),
    row(
        "mergus-security",
        3,
        false,
        "Secret resolution, PII handling, hash chains",
    ),
    row("mergus-menura", 3, false, "Bridge to Menura contracts"),
    row(
        "mergus-fetch",
        3,
        false,
        "Download, verify and cache versions and adapters",
    ),
    row(
        "mergus-runtime",
        4,
        false,
        "Run cycle and scheduling; wires ports to crates",
    ),
    row(
        "mergus-cli",
        5,
        false,
        "The `mergus` binary; translates arguments and output only",
    ),
    row("mergus-py", 5, false, "Python bindings (maturin)"),
    row(
        "mergus-certify",
        3,
        false,
        "Conformance kit every storage adapter must pass",
    ),
    Crate {
        name: "mergus-sim",
        layer: Layer::Test,
        core: false,
        responsibility: "Deterministic simulator",
    },
    Crate {
        name: "mergus-testkit",
        layer: Layer::Test,
        core: false,
        responsibility: "Fixtures, generators, golden datasets",
    },
];

/// The test crates (MG-REQ-0124).
pub(crate) const TEST_CRATES: [&str; 2] = ["mergus-sim", "mergus-testkit"];

/// Layer-3 crates every other layer-3 crate may depend on (MG-REQ-0120).
pub(crate) const SHARED_LAYER_3: [&str; 2] = ["mergus-security", "mergus-ops"];

/// The engine, source and adapter crates, and `mergus-runtime`, that `mergus-plan` must not
/// reach (MG-REQ-0121).
pub(crate) const UNREACHABLE_FROM_PLAN: [&str; 7] = [
    "mergus-engine-duckdb",
    "mergus-engine-polars",
    "mergus-source",
    "mergus-source-rest",
    "mergus-adapter",
    "mergus-adapter-ducklake",
    "mergus-runtime",
];

/// The members that may enable the feature `real` of `mergus-ports` (MG-REQ-0126.5).
pub(crate) const REAL_PORTS_USERS: [&str; 4] = [
    "mergus-runtime",
    "mergus-cli",
    "mergus-py",
    "mergus-testkit",
];

/// Returns the row of the crate table for `name`, or `None`.
pub(crate) fn find(name: &str) -> Option<&'static Crate> {
    CRATES.iter().find(|row| row.name == name)
}
