//! Every name Mergus uses from another module, defined once (MG-REQ-0207).
//!
//! Code never spells these names as literals: it uses these constants. The names of the
//! contracts Mergus owns are the `NAME` constants of the generated types
//! ([`crate::Contract::NAME`]).

/// The name of the Mergus binary.
pub const BINARY: &str = "mergus";

/// The Menura contract that describes the catalog schema; Mergus produces it.
pub const MENURA_CATALOG_SCHEMA: &str = "catalog_schema";

/// The Menura contract that describes the catalog state; Mergus produces it.
pub const MENURA_CATALOG_STATE: &str = "catalog_state";

/// The Menura contract that states what a producer must provide; Mergus consumes it.
pub const MENURA_PRODUCER_REQUIREMENTS: &str = "producer_requirements";

/// The Menura contract that lists the integrity checks; Mergus consumes it.
pub const MENURA_INTEGRITY_CHECKS: &str = "integrity_checks";
