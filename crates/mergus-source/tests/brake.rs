//! MG-REQ-0428: the automatic brake, checked on a scripted `Database` port whose chunks
//! take a set latency on a simulated clock.

mod scripted;

use std::error::Error;
use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mergus_ports::{Database, DatabaseLocation, PortError, SqlValue};
use mergus_sim::SimClock;
use mergus_source::{
    AttachedSource, Brake, Budget, DatabaseEngine, EngineIdentity, Load, read_with_brake,
};
use scripted::{Recorded, START, Scripted, WHOLE, clock_at, ids, orders};

/// A server whose `n`-th chunk statement takes `latencies[n]`, and fails when it is `None`.
fn timed(clock: Arc<SimClock>, latencies: Vec<Option<u64>>) -> Scripted {
    let sent = Arc::new(Mutex::new(0_usize));
    Scripted::new(move |_sql, params| {
        let Ok(mut count) = sent.lock() else {
            return Err(PortError::Failed("the script is poisoned".to_owned()));
        };
        let latency = latencies.get(*count).copied().flatten();
        *count += 1;
        let Some(millis) = latency else {
            return Err(PortError::Failed(
                "canceling statement due to statement timeout".to_owned(),
            ));
        };
        clock.advance(Duration::from_millis(millis));
        let after = match params.first() {
            Some(SqlValue::Integer(after)) if params.len() == 2 => *after,
            _ => 0,
        };
        Ok(ids(after + 1, 1))
    })
}

fn attached_to(database: &Scripted) -> Result<AttachedSource, PortError> {
    Ok(AttachedSource {
        connection: database.connect(&DatabaseLocation::InMemory)?,
        alias: "erp".to_owned(),
        identity: EngineIdentity {
            engine: DatabaseEngine::PostgreSql,
            version: "PostgreSQL 17.2".to_owned(),
        },
    })
}

#[test]
fn mg_req_0428_brake_slows_and_stops() {
    pace_halves_and_recovers();
    three_failures_stop_the_source().unwrap();
    a_slow_chunk_is_not_a_failure().unwrap();
}

/// MG-REQ-0428.1 and MG-REQ-0428.2.
fn pace_halves_and_recovers() {
    let high = Load::High.table();
    let mut brake = Brake::new(&high);
    for latency in [10, 10, 10] {
        brake.chunk_read(Duration::from_millis(latency));
    }
    assert_eq!(brake.pause(), Duration::ZERO);
    brake.chunk_read(Duration::from_millis(30));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(30),
        "slow chunk, pause from its latency"
    );
    brake.chunk_read(Duration::from_millis(40));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(60),
        "slow again, pause doubled"
    );
    brake.chunk_read(Duration::from_millis(15));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(60),
        "between median and twice: unchanged"
    );
    brake.chunk_read(Duration::from_millis(10));
    assert_eq!(
        brake.pause(),
        Duration::from_millis(30),
        "recovered: extra halved"
    );
    for _ in 0..10 {
        brake.chunk_read(Duration::from_millis(10));
    }
    assert_eq!(brake.pause(), Duration::ZERO, "back to the budget's pause");

    let low = Load::Low.table();
    let mut brake = Brake::new(&low);
    for latency in [10, 10, 10, 50] {
        brake.chunk_read(Duration::from_millis(latency));
    }
    assert_eq!(
        brake.pause(),
        Duration::from_secs(2),
        "the budget's pause doubled"
    );
}

/// A budget of one-row chunks without a pause, so that each scripted answer is one chunk.
fn one_row_chunks() -> Budget {
    Budget {
        connections: 4,
        chunk_rows: NonZeroU32::MIN,
        pause: std::time::Duration::ZERO,
    }
}

/// MG-REQ-0428.3 and MG-REQ-0428.4: two failures then a success, the chunk is sent again
/// and the read goes on; three consecutive failures stop the source.
fn three_failures_stop_the_source() -> Result<(), Box<dyn Error>> {
    let high = one_row_chunks();
    let clock = clock_at(START);
    let database = timed(Arc::clone(&clock), vec![Some(10), None, None, Some(10)]);
    let mut attached = attached_to(&database)?;
    let recorded = Recorded::new(Arc::clone(&clock));
    let mut brake = Brake::new(&high);
    let incident = read_with_brake(
        &mut attached,
        "erp",
        &orders(),
        WHOLE,
        &mut brake,
        &recorded,
    )
    .err();
    let incident = incident.ok_or("three consecutive failures stop the source")?;
    // Chunks 5, 6, 7 have no script: three consecutive failures stop the source.
    assert_eq!(incident.code(), "MG-0405");
    assert_eq!(
        (incident.source_name.as_str(), incident.table.as_str()),
        ("erp", "orders")
    );
    assert_eq!(incident.failures, 3);
    assert_eq!(
        database.statements().len(),
        7,
        "4 scripted, then 3 failures and no more"
    );
    let message = incident.to_string();
    assert!(message.contains("source `erp`"), "{message}");
    assert!(message.contains("statement timeout"), "{message}");
    Ok(())
}

/// MG-REQ-0428.3: a slow chunk is not a failure; it slows the pace.
fn a_slow_chunk_is_not_a_failure() -> Result<(), Box<dyn Error>> {
    let high = one_row_chunks();
    let clock = clock_at(START);
    let database = timed(
        Arc::clone(&clock),
        vec![Some(10), Some(10), Some(500), Some(10), Some(10)],
    );
    let mut attached = attached_to(&database)?;
    let recorded = Recorded::new(Arc::clone(&clock));
    let mut brake = Brake::new(&high);
    let _ = read_with_brake(
        &mut attached,
        "erp",
        &orders(),
        WHOLE,
        &mut brake,
        &recorded,
    );
    let sleeps = recorded.sleeps();
    assert_eq!(
        sleeps.first(),
        Some(&Duration::from_millis(500)),
        "{sleeps:?}"
    );
    assert_eq!(
        sleeps.get(1),
        Some(&Duration::from_millis(250)),
        "{sleeps:?}"
    );
    Ok(())
}
