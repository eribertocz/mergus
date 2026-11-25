//! The capability check of MG-REQ-0287.

use std::fmt;

use arrow::datatypes::DataType;

use crate::{Capabilities, CapabilityError, Versioning, WriteMode};

/// One write mode or feature a run needs from its adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Requirement {
    /// The write mode is implemented.
    WriteMode(WriteMode),
    /// Data versions are kept, natively or by files.
    Versioning,
    /// One commit can span several tables.
    MultiTableTransactions,
    /// Several clients can write at the same time.
    ConcurrentClients,
    /// The Arrow type is stored exactly.
    ExactType(DataType),
}

impl Requirement {
    /// Returns whether `capabilities` declares this requirement.
    fn is_declared(&self, capabilities: &Capabilities) -> bool {
        match self {
            Self::WriteMode(mode) => capabilities.write_modes.contains(mode),
            Self::Versioning => capabilities.versioning != Versioning::None,
            Self::MultiTableTransactions => capabilities.multi_table_transactions,
            Self::ConcurrentClients => capabilities.concurrent_clients,
            Self::ExactType(data_type) => capabilities.exact_types.contains(data_type),
        }
    }
}

impl fmt::Display for Requirement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WriteMode(mode) => write!(formatter, "write mode `{mode}`"),
            Self::Versioning => formatter.write_str("data versioning"),
            Self::MultiTableTransactions => formatter.write_str("multi-table transactions"),
            Self::ConcurrentClients => formatter.write_str("concurrent clients"),
            Self::ExactType(data_type) => write!(formatter, "exact storage of type `{data_type}`"),
        }
    }
}

/// Checks `needs` against what an adapter declares, from its declaration only, so a caller
/// runs it before any data is extracted (MG-REQ-0287).
///
/// # Errors
///
/// Returns a [`CapabilityError`] naming every requirement not declared, in the order of
/// `needs`.
pub fn check_capabilities(
    capabilities: &Capabilities,
    needs: &[Requirement],
) -> Result<(), CapabilityError> {
    let mut missing: Vec<Requirement> = Vec::new();
    for need in needs {
        if !need.is_declared(capabilities) && !missing.contains(need) {
            missing.push(need.clone());
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CapabilityError { missing })
    }
}
