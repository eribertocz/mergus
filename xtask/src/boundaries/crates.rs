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
}

const fn row(name: &'static str, layer: u8, core: bool) -> Crate {
    Crate {
        name,
        layer: Layer::Number(layer),
        core,
    }
}

/// Every crate of `architecture/crates.md`, in the order of the table.
const CRATES: [Crate; 22] = [
    row("mergus-contracts", 0, true),
    row("mergus-ports", 0, false),
    row("mergus-project", 1, true),
    row("mergus-plan", 2, true),
    row("mergus-engine-duckdb", 3, false),
    row("mergus-engine-polars", 3, false),
    row("mergus-source", 3, false),
    row("mergus-source-rest", 3, false),
    row("mergus-adapter", 3, false),
    row("mergus-adapter-ducklake", 3, false),
    row("mergus-catalog", 3, false),
    row("mergus-quality", 3, false),
    row("mergus-ops", 3, false),
    row("mergus-security", 3, false),
    row("mergus-menura", 3, false),
    row("mergus-fetch", 3, false),
    row("mergus-runtime", 4, false),
    row("mergus-cli", 5, false),
    row("mergus-py", 5, false),
    row("mergus-certify", 3, false),
    Crate {
        name: "mergus-sim",
        layer: Layer::Test,
        core: false,
    },
    Crate {
        name: "mergus-testkit",
        layer: Layer::Test,
        core: false,
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
