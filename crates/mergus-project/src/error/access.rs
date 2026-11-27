//! What a [`ProjectError`] tells about itself: its code and where it was found.

use super::ProjectError;

impl ProjectError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateModel { .. } => "MG-0301",
            Self::InvalidName { .. } => "MG-0302",
            Self::InvalidToml { .. } => "MG-0303",
            Self::UnknownKey { .. } => "MG-0304",
            Self::InvalidValue { .. } => "MG-0305",
            Self::Expression { .. } => "MG-0306",
            Self::NonFiniteFloat { .. } => "MG-0307",
            Self::UnknownSection { .. } => "MG-0308",
            Self::LiteralSecret { .. } => "MG-0309",
            Self::InvalidIds { .. } => "MG-0310",
            Self::DuplicateIdsEntry { .. } => "MG-0311",
            Self::DuplicateIdentifier { .. } => "MG-0312",
            Self::UnknownReference { .. } => "MG-0313",
            Self::ForbiddenSql { .. } => "MG-0314",
            Self::ModelCycle { .. } => "MG-0315",
            Self::UnpreparableSql { .. } => "MG-0601",
            Self::UnknownParameter { .. } => "MG-0602",
            Self::PositionalParameter { .. } => "MG-0603",
            Self::ForbiddenMacroFile { .. } => "MG-0611",
            Self::DuplicateMacro { .. } | Self::MacroShadowsBuiltin { .. } => "MG-0612",
            Self::UndeclaredSeed { .. } => "MG-0613",
            Self::SeedWithoutFile { .. } => "MG-0614",
            Self::UnknownSeedKey { .. } => "MG-0615",
            Self::UnknownSeedType { .. } => "MG-0616",
            Self::Python(error) => error.code(),
        }
    }

    /// Returns the path that caused the error, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::DuplicateModel { second, .. } => second,
            Self::InvalidName { path, .. }
            | Self::InvalidToml { path, .. }
            | Self::UnknownKey { path, .. }
            | Self::InvalidValue { path, .. }
            | Self::Expression { path, .. }
            | Self::NonFiniteFloat { path, .. }
            | Self::UnknownSection { path, .. }
            | Self::LiteralSecret { path, .. }
            | Self::InvalidIds { path, .. }
            | Self::DuplicateIdsEntry { path, .. }
            | Self::DuplicateIdentifier { path, .. }
            | Self::UnknownReference { path, .. }
            | Self::ForbiddenSql { path, .. }
            | Self::UnpreparableSql { path, .. }
            | Self::UnknownParameter { path, .. }
            | Self::PositionalParameter { path, .. }
            | Self::ForbiddenMacroFile { path, .. }
            | Self::DuplicateMacro { path, .. }
            | Self::MacroShadowsBuiltin { path, .. }
            | Self::UndeclaredSeed { path, .. }
            | Self::SeedWithoutFile { path, .. }
            | Self::UnknownSeedKey { path, .. }
            | Self::UnknownSeedType { path, .. } => path,
            Self::ModelCycle { references, .. } => references.first().map_or("", |(path, _)| path),
            Self::Python(error) => error.path(),
        }
    }

    /// Returns the line of the file that caused the error, from 1, when there is one.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::DuplicateModel { .. }
            | Self::InvalidName { .. }
            | Self::DuplicateIdentifier { .. }
            | Self::UnpreparableSql { .. }
            | Self::ForbiddenMacroFile { .. }
            | Self::DuplicateMacro { .. }
            | Self::MacroShadowsBuiltin { .. }
            | Self::UndeclaredSeed { .. } => None,
            Self::InvalidToml { line, .. }
            | Self::UnknownKey { line, .. }
            | Self::InvalidValue { line, .. }
            | Self::Expression { line, .. }
            | Self::NonFiniteFloat { line, .. }
            | Self::UnknownSection { line, .. }
            | Self::LiteralSecret { line, .. }
            | Self::InvalidIds { line, .. }
            | Self::DuplicateIdsEntry { line, .. }
            | Self::UnknownReference { line, .. }
            | Self::ForbiddenSql { line, .. }
            | Self::UnknownParameter { line, .. }
            | Self::PositionalParameter { line, .. }
            | Self::SeedWithoutFile { line, .. }
            | Self::UnknownSeedKey { line, .. }
            | Self::UnknownSeedType { line, .. } => Some(*line),
            Self::ModelCycle { references, .. } => references.first().map(|(_, line)| *line),
            Self::Python(error) => error.line(),
        }
    }

    /// Returns the column of the line that caused the error, from 1, when there is one.
    #[must_use]
    pub fn column(&self) -> Option<usize> {
        match self {
            Self::UnknownReference { column, .. }
            | Self::ForbiddenSql { column, .. }
            | Self::UnknownParameter { column, .. }
            | Self::PositionalParameter { column, .. } => Some(*column),
            Self::Python(error) => error.column(),
            _ => None,
        }
    }

    /// Returns the configuration key that caused the error, dotted from the root of its file.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::DuplicateModel { .. }
            | Self::InvalidName { .. }
            | Self::InvalidToml { .. }
            | Self::DuplicateIdentifier { .. }
            | Self::UnknownReference { .. }
            | Self::ForbiddenSql { .. }
            | Self::ModelCycle { .. }
            | Self::UnpreparableSql { .. }
            | Self::UnknownParameter { .. }
            | Self::PositionalParameter { .. }
            | Self::ForbiddenMacroFile { .. }
            | Self::DuplicateMacro { .. }
            | Self::MacroShadowsBuiltin { .. }
            | Self::UndeclaredSeed { .. }
            | Self::SeedWithoutFile { .. }
            | Self::Python(_) => None,
            Self::InvalidIds { key, .. } => key.as_deref(),
            Self::UnknownKey { key, .. }
            | Self::InvalidValue { key, .. }
            | Self::Expression { key, .. }
            | Self::NonFiniteFloat { key, .. }
            | Self::UnknownSection { section: key, .. }
            | Self::LiteralSecret { key, .. }
            | Self::DuplicateIdsEntry { second: key, .. }
            | Self::UnknownSeedKey { key, .. }
            | Self::UnknownSeedType { key, .. } => Some(key),
        }
    }
}
