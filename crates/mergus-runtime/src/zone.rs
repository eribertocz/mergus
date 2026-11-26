//! The storage of each zone, from its profile (MG-REQ-0521).

use std::fmt;

use mergus_project::Configuration;
use serde_json::Value;

/// A zone with its own storage in `project.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    /// `[raw]`: extracted data as received.
    Raw,
    /// `[gold]`: published tables and summaries.
    Gold,
    /// `[ops]`: ops tables and samples of failing rows.
    Ops,
}

impl Zone {
    /// Every zone, in the order of [`zone_storage`].
    pub const ALL: [Self; 3] = [Self::Raw, Self::Gold, Self::Ops];

    /// The name of the zone, which is also its table in `project.toml`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Gold => "gold",
            Self::Ops => "ops",
        }
    }
}

impl fmt::Display for Zone {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// A storage profile (MG-REQ-0521). `server` comes in phase 0.7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    /// Development on one machine: a DuckDB catalog in a folder.
    Local,
    /// Several processes on one machine: a SQLite catalog.
    Shared,
    /// Production: a PostgreSQL catalog, data on an object store.
    Cloud,
}

impl Profile {
    /// The profile named `name` in `project.toml`, or `None`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "local" => Some(Self::Local),
            "shared" => Some(Self::Shared),
            "cloud" => Some(Self::Cloud),
            _ => None,
        }
    }
}

/// Who may use the catalog of a zone at the same time (MG-REQ-0521).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clients {
    /// One process.
    OneProcess,
    /// Several processes of one machine.
    LocalProcesses,
    /// Several clients, remote.
    Remote,
}

/// The DuckLake catalog of a zone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Catalog {
    /// A DuckDB file, for `local`.
    DuckDb {
        /// The path of the file.
        file: String,
    },
    /// A SQLite file, for `shared`.
    Sqlite {
        /// The path of the file.
        file: String,
    },
    /// A PostgreSQL database, for `cloud`, whose connection is a secret.
    Postgres {
        /// The secret reference of the connection, `secret:<name>`.
        secret: String,
    },
}

impl Catalog {
    /// The DuckLake attach string of the catalog: `ducklake:<file>`,
    /// `ducklake:sqlite:<file>`, or `ducklake:postgres:<connection>`, where `connection` is
    /// the value of the secret. `None` for a PostgreSQL catalog when no value is given.
    #[must_use]
    pub fn attach(&self, connection: Option<&str>) -> Option<String> {
        match self {
            Self::DuckDb { file } => Some(format!("ducklake:{file}")),
            Self::Sqlite { file } => Some(format!("ducklake:sqlite:{file}")),
            Self::Postgres { .. } => connection.map(|value| format!("ducklake:postgres:{value}")),
        }
    }

    /// The DuckDB extension the catalog needs besides `ducklake`: `sqlite` for a SQLite
    /// catalog, `postgres` for a PostgreSQL one, none for a DuckDB file (MG-REQ-0521.3).
    #[must_use]
    pub fn extension(&self) -> Option<&'static str> {
        match self {
            Self::DuckDb { .. } => None,
            Self::Sqlite { .. } => Some("sqlite"),
            Self::Postgres { .. } => Some("postgres"),
        }
    }
}

/// The storage of one zone, complete from its profile (MG-REQ-0521).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ZoneStorage {
    /// The zone.
    pub zone: Zone,
    /// Its profile.
    pub profile: Profile,
    /// Its `location`, as written.
    pub location: String,
    /// Its catalog.
    pub catalog: Catalog,
    /// The directory or object-store prefix of its data files.
    pub data_path: String,
    /// Who may use its catalog at the same time.
    pub clients: Clients,
}

/// The storage of each zone that `configuration` declares, in the order raw, gold, ops. The
/// configuration is valid, so a zone has a known `profile`, a `location` and, with `cloud`, a
/// `catalog` (MG-REQ-0521.1); a zone that lacks one is left out.
#[must_use]
pub fn zone_storage(configuration: &Configuration) -> Vec<ZoneStorage> {
    Zone::ALL
        .into_iter()
        .filter_map(|zone| {
            let table = configuration.project.value.get(zone.name())?;
            resolve(zone, table)
        })
        .collect()
}

/// The storage of `zone`, declared by `table`.
fn resolve(zone: Zone, table: &Value) -> Option<ZoneStorage> {
    let text = |key: &str| table.get(key).and_then(Value::as_str);
    let profile = Profile::parse(text("profile")?)?;
    let location = text("location")?;
    let base = location.trim_end_matches('/');
    let catalog = text("catalog").map(str::to_owned);
    let (catalog, data_path, clients) = match profile {
        Profile::Local => (
            Catalog::DuckDb {
                file: catalog.unwrap_or_else(|| format!("{base}/catalog.ducklake")),
            },
            format!("{base}/data/"),
            Clients::OneProcess,
        ),
        Profile::Shared => (
            Catalog::Sqlite {
                file: catalog.unwrap_or_else(|| format!("{base}/catalog.sqlite")),
            },
            format!("{base}/data/"),
            Clients::LocalProcesses,
        ),
        Profile::Cloud => (
            Catalog::Postgres { secret: catalog? },
            location.to_owned(),
            Clients::Remote,
        ),
    };
    Some(ZoneStorage {
        zone,
        profile,
        location: location.to_owned(),
        catalog,
        data_path: text("data_path").map_or(data_path, str::to_owned),
        clients,
    })
}
