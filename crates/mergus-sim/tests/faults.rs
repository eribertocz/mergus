//! MG-REQ-0131: the simulated ports inject the faults of MG-REQ-0161 that belong to a port.

use std::sync::Arc;
use std::time::Duration;

use mergus_ports::{
    Clock, HttpRequest, HttpResponse, Network, ObjectStore, PortError, SourceConnector, Timestamp,
};
use mergus_sim::{
    ClockFaults, NetworkFaults, ObjectStoreFaults, Rate, SimClock, SimNetwork, SimObjectStore,
    SimSourceConnector, SourceFaults,
};

const START: Timestamp = Timestamp::from_unix_nanos(1_800_000_000_000_000_000);

fn clock() -> Arc<SimClock> {
    Arc::new(SimClock::new(1, START, ClockFaults::default()))
}

fn is_injected(result: Result<impl std::fmt::Debug, PortError>) -> bool {
    matches!(result, Err(PortError::Failed(message)) if message.starts_with("simulated"))
}

#[test]
fn mg_req_0131_object_store_injects_errors_latency_partial_responses_and_lost_writes() {
    let store = |faults| SimObjectStore::new(7, faults, clock());
    let failing = store(ObjectStoreFaults {
        error: Rate::ALWAYS,
        ..ObjectStoreFaults::default()
    });
    assert!(is_injected(failing.put("a", b"x")));
    assert!(is_injected(failing.get("a")));

    let shared = clock();
    let slow = SimObjectStore::new(
        7,
        ObjectStoreFaults {
            latency: Rate::ALWAYS,
            latency_duration: Duration::from_secs(2),
            ..ObjectStoreFaults::default()
        },
        Arc::clone(&shared),
    );
    slow.put("a", b"x").unwrap();
    assert_eq!(shared.monotonic().elapsed(), Duration::from_secs(2));

    let partial = store(ObjectStoreFaults {
        partial: Rate::ALWAYS,
        ..ObjectStoreFaults::default()
    });
    partial.put("a", b"0123456789").unwrap();
    assert!(partial.get("a").unwrap().len() < 10);

    let lossy = store(ObjectStoreFaults {
        lost_write: Rate::ALWAYS,
        ..ObjectStoreFaults::default()
    });
    lossy.put("a", b"x").unwrap();
    assert_eq!(lossy.get("a"), Err(PortError::NotFound("a".to_owned())));
}

#[test]
fn mg_req_0131_clock_injects_jumps_and_skew() {
    let jumping = SimClock::new(
        3,
        START,
        ClockFaults {
            jump: Rate::ALWAYS,
            jump_size: Duration::from_secs(60),
            ..ClockFaults::default()
        },
    );
    let jump = jumping.now().unix_nanos() - START.unix_nanos();
    assert_eq!(jump.abs(), 60_000_000_000);
    assert_eq!(jumping.monotonic().elapsed(), Duration::ZERO);

    let skewed = SimClock::new(
        3,
        START,
        ClockFaults {
            skew_per_million: 1_000,
            ..ClockFaults::default()
        },
    );
    skewed.sleep(Duration::from_secs(1));
    assert_eq!(skewed.monotonic().elapsed(), Duration::from_secs(1));
    assert_eq!(
        skewed.now().unix_nanos() - START.unix_nanos(),
        1_001_000_000
    );
}

fn request() -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        url: "https://api.example/".to_owned(),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

#[test]
fn mg_req_0131_network_injects_errors_latency_and_partial_responses() {
    let network = |faults, clock| {
        let network = SimNetwork::new(5, faults, clock);
        network.route(
            "https://api.example/",
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: b"0123456789".to_vec(),
            },
        );
        network.listen("db", 1, Arc::new(|bytes: &[u8]| bytes.to_vec()));
        network
    };
    let failing = network(
        NetworkFaults {
            error: Rate::ALWAYS,
            ..NetworkFaults::default()
        },
        clock(),
    );
    assert!(is_injected(failing.http(&request())));
    assert!(is_injected(failing.tcp_connect("db", 1).map(|_| ())));

    let shared = clock();
    let slow = network(
        NetworkFaults {
            latency: Rate::ALWAYS,
            latency_duration: Duration::from_millis(300),
            ..NetworkFaults::default()
        },
        Arc::clone(&shared),
    );
    slow.http(&request()).unwrap();
    assert_eq!(shared.monotonic().elapsed(), Duration::from_millis(300));

    let partial = network(
        NetworkFaults {
            partial: Rate::ALWAYS,
            ..NetworkFaults::default()
        },
        clock(),
    );
    assert!(partial.http(&request()).unwrap().body.len() < 10);
    let mut connection = partial.tcp_connect("db", 1).unwrap();
    connection.send(b"0123456789").unwrap();
    assert!(connection.receive(64).unwrap().len() < 10);
}

#[test]
fn mg_req_0131_sources_change_fail_or_return_partial_data_mid_extraction() {
    let connector = |faults| {
        let connector = SimSourceConnector::new(9, faults);
        let batches = |tag: &str| (0..3).map(|i| format!("{tag}{i}").into_bytes()).collect();
        connector.register("orders", batches("v1-"), Some(batches("v2-")));
        connector
    };
    let read_all = |connector: &SimSourceConnector| {
        let mut reader = connector.open("orders").unwrap();
        let mut batches = Vec::new();
        while let Some(batch) = reader.next_batch()? {
            batches.push(String::from_utf8(batch).unwrap());
        }
        Ok::<_, PortError>(batches)
    };
    let clean = connector(SourceFaults::default());
    assert_eq!(read_all(&clean).unwrap(), ["v1-0", "v1-1", "v1-2"]);

    let failing = connector(SourceFaults {
        fail: Rate::ALWAYS,
        ..SourceFaults::default()
    });
    assert!(is_injected(read_all(&failing)));

    let partial = connector(SourceFaults {
        partial: Rate::ALWAYS,
        ..SourceFaults::default()
    });
    assert_eq!(read_all(&partial).unwrap(), Vec::<String>::new());

    let changing = connector(SourceFaults {
        change: Rate::ALWAYS,
        ..SourceFaults::default()
    });
    assert_eq!(read_all(&changing).unwrap(), ["v2-0", "v2-1", "v2-2"]);
}
