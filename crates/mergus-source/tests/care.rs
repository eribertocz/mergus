//! MG-REQ-0426, MG-REQ-0427 and MG-REQ-0430: the replica, the load budget and the time
//! window, checked on the statements sent to a scripted `Database` port and on a simulated
//! clock.

mod scripted;

use std::collections::BTreeMap;
use std::error::Error;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, StringArray};
use arrow::record_batch::RecordBatch;
use mergus_ports::{Clock, SqlValue};
use mergus_project::{ConfigFile, Source};
use mergus_source::{
    Brake, Budget, Load, LoadKind, SourceCare, Window, attach_postgres, read_with_brake,
    wait_for_window,
};
use scripted::{Recorded, START, Scripted, chunks_of, clock_at, ids, orders};
use serde_json::{Value, json};

/// The parameters of every statement of `database` whose text starts with `prefix`.
fn params_of(database: &Scripted, prefix: &str) -> Vec<Vec<SqlValue>> {
    database
        .statements()
        .into_iter()
        .filter(|(sql, _)| sql.starts_with(prefix))
        .map(|(_, params)| params)
        .collect()
}

fn source(options: &Value) -> Source {
    let mut value = json!({
        "kind": "database",
        "engine": "postgresql",
        "connection": "secret:erp",
        "tables": { "orders": { "key": "id" } }
    });
    for (key, option) in options.as_object().into_iter().flatten() {
        value[key] = option.clone();
    }
    Source {
        name: "erp".to_owned(),
        file: ConfigFile {
            path: "sources/erp.toml".to_owned(),
            canonical: value.to_string(),
            value,
        },
    }
}

/// A PostgreSQL server that answers `version()` and serves `orders` in chunks.
fn server(total: i64) -> Scripted {
    Scripted::new(move |sql, params| {
        if sql.starts_with("SELECT * FROM postgres_query") {
            let column: ArrayRef = Arc::new(StringArray::from(vec!["PostgreSQL 17.2"]));
            return Ok(RecordBatch::try_from_iter([("version", column)])
                .into_iter()
                .collect());
        }
        let limit = match params.last() {
            Some(SqlValue::Integer(limit)) => *limit,
            _ => total,
        };
        let after = match params.first() {
            Some(SqlValue::Integer(after)) if params.len() == 2 => *after,
            _ => 0,
        };
        Ok(ids(after + 1, limit.min(total - after).max(0)))
    })
}

#[test]
fn mg_req_0426_queries_go_to_replica() {
    let secrets = BTreeMap::from([
        ("secret:erp", "postgresql://primary/erp"),
        ("secret:erp_replica", "postgresql://replica/erp"),
    ]);
    for (options, expected) in [
        (
            json!({ "replica": "secret:erp_replica" }),
            "postgresql://replica/erp",
        ),
        (json!({}), "postgresql://primary/erp"),
    ] {
        let care = SourceCare::of(&source(&options), "prod");
        let reference = care.extraction_reference().unwrap();
        let value = secrets[reference].to_owned();
        let database = server(3);
        let mut attached =
            attach_postgres(&database, "erp", &care.postgres_source("erp", value)).unwrap();
        let clock = Recorded::new(clock_at(START));
        let mut brake = Brake::new(&care.budget());
        read_with_brake(
            &mut attached,
            "erp",
            &orders(),
            &chunks_of(2),
            &mut brake,
            &clock,
        )
        .unwrap();
        let secrets_created = params_of(&database, "CREATE TEMPORARY SECRET");
        assert_eq!(
            secrets_created,
            vec![vec![SqlValue::Text(expected.to_owned())]]
        );
        let reads = params_of(&database, "SELECT * FROM \"erp\".\"orders\"");
        assert_eq!(reads.len(), 2, "{:?}", database.statements());
    }
}

#[test]
fn mg_req_0427_connections_never_exceed_budget() {
    let table = [
        (Load::Low, 1, 10_000, Duration::from_secs(1)),
        (Load::Normal, 2, 50_000, Duration::from_millis(200)),
        (Load::High, 4, 100_000, Duration::ZERO),
    ];
    for (load, connections, chunk_rows, pause) in table {
        for max_connections in 1..=5_u32 {
            let expected = Budget {
                connections: connections.min(max_connections),
                chunk_rows: NonZeroU32::new(chunk_rows).unwrap(),
                pause,
            };
            budget_is_kept(load, max_connections, expected).unwrap();
        }
    }
    let defaults = SourceCare::of(&source(&json!({})), "prod");
    assert_eq!((defaults.load, defaults.max_connections), (Load::Low, 2));
    let elsewhere = SourceCare::of(&source(&json!({})), "dev");
    assert_eq!(elsewhere.load, Load::Normal);
    assert_eq!(defaults.query_timeout, Duration::from_secs(600));
}

/// Checks that a source of `load` and `max_connections` gets the budget `expected`, that
/// its pool is no larger, and that its chunks and pauses follow it.
fn budget_is_kept(
    load: Load,
    max_connections: u32,
    expected: Budget,
) -> Result<(), Box<dyn Error>> {
    let options = json!({ "load": load.to_string(), "max_connections": max_connections });
    let care = SourceCare::of(&source(&options), "dev");
    let budget = care.budget();
    assert_eq!(budget, expected, "{load} {max_connections}");
    assert!(budget.connections <= max_connections);
    let rows = i64::from(budget.chunk_rows.get());
    let database = server(rows * 2 + 1);
    let pg = care.postgres_source("erp", "postgresql://primary/erp".to_owned());
    let mut attached = attach_postgres(&database, "erp", &pg)?;
    let pool = params_of(&database, "SET pg_pool_max_connections");
    let connections = SqlValue::Integer(i64::from(budget.connections));
    assert_eq!(pool, vec![vec![connections]]);
    let clock = Recorded::new(clock_at(START));
    let mut brake = Brake::new(&budget);
    let options = chunks_of(budget.chunk_rows.get());
    let read = read_with_brake(
        &mut attached,
        "erp",
        &orders(),
        &options,
        &mut brake,
        &clock,
    );
    assert!(read.is_ok());
    let limits: Vec<_> = params_of(&database, "SELECT * FROM \"erp\".\"orders\"")
        .into_iter()
        .map(|params| params.last().cloned())
        .collect();
    let limit = Some(SqlValue::Integer(rows));
    assert_eq!(limits, vec![limit.clone(), limit.clone(), limit]);
    let pauses = if budget.pause.is_zero() {
        Vec::new()
    } else {
        vec![budget.pause; 2]
    };
    assert_eq!(clock.sleeps(), pauses);
    Ok(())
}

#[test]
fn mg_req_0430_full_loads_wait_for_window() {
    let night = Window::parse("01:00-03:00").unwrap();
    for (kind, waits) in [
        (LoadKind::Full, true),
        (LoadKind::Backfill, true),
        (LoadKind::Incremental, false),
    ] {
        let clock = clock_at(START);
        let waited = wait_for_window(&*clock, Some(&night), kind);
        let expected = if waits {
            // From 23:15:14 to 01:00:00.
            Duration::from_secs(3_600 + 44 * 60 + 46)
        } else {
            Duration::ZERO
        };
        assert_eq!(waited, expected, "{kind:?}");
        assert_eq!(night.contains(clock.now()), waits, "{kind:?}");
    }
    let clock = clock_at(START);
    assert_eq!(
        wait_for_window(&*clock, None, LoadKind::Full),
        Duration::ZERO
    );
    let across_midnight = Window::parse("22:00-02:00").unwrap();
    assert_eq!(
        wait_for_window(&*clock, Some(&across_midnight), LoadKind::Full),
        Duration::ZERO
    );
    let morning = Window::parse("23:16-23:20").unwrap();
    assert_eq!(
        wait_for_window(&*clock, Some(&morning), LoadKind::Full),
        Duration::from_secs(46)
    );
    assert_eq!(night.to_string(), "01:00-03:00");
    let care = SourceCare::of(&source(&json!({ "window": "01:00-03:00" })), "prod");
    assert_eq!(care.window, Some(night));
}
