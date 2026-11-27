//! The care options of a source (`extract/source-care.md`): the replica every extraction
//! query goes to (MG-REQ-0426), and the load budget its `load` and `max_connections` set
//! (MG-REQ-0427).

use std::fmt;
use std::num::NonZeroU32;
use std::time::Duration;

use mergus_ports::PostgresSource;
use mergus_project::Source;

use crate::window::Window;

/// The environment whose default `load` is `low`; every other one defaults to `normal`.
const PRODUCTION: &str = "prod";

/// The default `max_connections`.
const DEFAULT_MAX_CONNECTIONS: u32 = 2;

/// The default `query_timeout`: 10 minutes.
const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(600);

/// How much load an extraction puts on a source (MG-REQ-0427).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Load {
    /// 1 connection, 10 000 rows per chunk, 1 s between chunks.
    Low,
    /// 2 connections, 50 000 rows per chunk, 200 ms between chunks.
    Normal,
    /// 4 connections, 100 000 rows per chunk, no pause.
    High,
}

impl Load {
    /// The `load` of `name`, as a source file writes it.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "low" => Some(Self::Low),
            "normal" => Some(Self::Normal),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    /// The default `load` of `environment`: `low` in `prod`, `normal` elsewhere.
    #[must_use]
    pub fn default_for(environment: &str) -> Self {
        if environment == PRODUCTION {
            Self::Low
        } else {
            Self::Normal
        }
    }

    /// The row of the load budget table of this load (MG-REQ-0427), before
    /// `max_connections` applies.
    #[must_use]
    pub const fn table(self) -> Budget {
        let (connections, chunk_rows, pause) = match self {
            Self::Low => (1, NonZeroU32::new(10_000), Duration::from_secs(1)),
            Self::Normal => (2, NonZeroU32::new(50_000), Duration::from_millis(200)),
            Self::High => (4, NonZeroU32::new(100_000), Duration::ZERO),
        };
        Budget {
            connections,
            chunk_rows: match chunk_rows {
                Some(rows) => rows,
                None => NonZeroU32::MIN,
            },
            pause,
        }
    }
}

impl fmt::Display for Load {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        })
    }
}

/// What an extraction of a source may use (MG-REQ-0427).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The most connections open at once.
    pub connections: u32,
    /// The rows of one chunk.
    pub chunk_rows: NonZeroU32,
    /// The pause between two chunks.
    pub pause: Duration,
}

/// The care options of one source, with their defaults applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCare {
    /// The secret reference of the source's `connection`, when it has one.
    pub connection: Option<String>,
    /// The secret reference of its `replica`, when it has one (MG-REQ-0426).
    pub replica: Option<String>,
    /// Its `load`, or the default of the environment.
    pub load: Load,
    /// Its `max_connections`, 2 by default.
    pub max_connections: u32,
    /// Its `window`, when it has one (MG-REQ-0430).
    pub window: Option<Window>,
    /// Its `query_timeout`, 10 minutes by default.
    pub query_timeout: Duration,
}

impl SourceCare {
    /// Reads the care options of `source`, a source file validated against its schema
    /// (MG-REQ-0376.5), for a run in `environment`.
    #[must_use]
    pub fn of(source: &Source, environment: &str) -> Self {
        let text = |key: &str| source.file.value.get(key).and_then(|value| value.as_str());
        let max_connections = source
            .file
            .value
            .get("max_connections")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(DEFAULT_MAX_CONNECTIONS);
        Self {
            connection: text("connection").map(str::to_owned),
            replica: text("replica").map(str::to_owned),
            load: text("load")
                .and_then(Load::parse)
                .unwrap_or_else(|| Load::default_for(environment)),
            max_connections,
            window: text("window").and_then(Window::parse),
            query_timeout: text("query_timeout")
                .and_then(parse_duration)
                .unwrap_or(DEFAULT_QUERY_TIMEOUT),
        }
    }

    /// The budget of the source: the row of its `load`, with connections never above
    /// `max_connections` (MG-REQ-0427).
    #[must_use]
    pub fn budget(&self) -> Budget {
        let row = self.load.table();
        Budget {
            connections: row.connections.min(self.max_connections),
            ..row
        }
    }

    /// The secret reference every extraction query is sent through: the `replica` when it
    /// is set, the `connection` otherwise (MG-REQ-0426).
    #[must_use]
    pub fn extraction_reference(&self) -> Option<&str> {
        self.replica.as_deref().or(self.connection.as_deref())
    }

    /// The PostgreSQL source to attach as `alias`, `connection` being the resolved value of
    /// [`SourceCare::extraction_reference`]: its pool holds the budget's connections and its
    /// statements time out after `query_timeout` (MG-REQ-0427, MG-REQ-0196.3).
    #[must_use]
    pub fn postgres_source(&self, alias: &str, connection: String) -> PostgresSource {
        PostgresSource {
            alias: alias.to_owned(),
            connection,
            max_connections: self.budget().connections,
            query_timeout: self.query_timeout,
        }
    }
}

/// A duration of a source file: a positive integer followed by `m`, `h` or `d`
/// (MG-REQ-0366.5).
fn parse_duration(text: &str) -> Option<Duration> {
    let unit = match text.chars().last()? {
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return None,
    };
    let count: u64 = text.get(..text.len() - 1)?.parse().ok()?;
    Some(Duration::from_secs(count.checked_mul(unit)?))
}
