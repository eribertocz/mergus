//! MG-REQ-0131: every port has a simulated implementation, deterministic for a seed.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use mergus_ports::{
    Clock, Environment, FileSystem, HttpRequest, HttpResponse, Network, ObjectStore, PortError,
    Process, ProcessCommand, ProcessOutput, Random, SourceConnector, SqlParser, Timestamp,
};
use mergus_sim::{
    ClockFaults, NetworkFaults, ObjectStoreFaults, Rate, SimClock, SimEnvironment, SimFileSystem,
    SimNetwork, SimObjectStore, SimProcess, SimRandom, SimSourceConnector, SimSqlParser,
    SourceFaults,
};

/// One value of every port, as the code under simulation receives them.
struct Ports {
    clock: Arc<dyn Clock>,
    file_system: Arc<dyn FileSystem>,
    object_store: Arc<dyn ObjectStore>,
    network: Arc<dyn Network>,
    source: Arc<dyn SourceConnector>,
    random: Arc<dyn Random>,
    process: Arc<dyn Process>,
    environment: Arc<dyn Environment>,
    sql_parser: Arc<dyn SqlParser>,
}

const SOMETIMES: Rate = Rate::per_million(300_000);

fn simulated(seed: u64) -> Ports {
    let clock = Arc::new(SimClock::new(
        seed,
        Timestamp::from_unix_nanos(1_800_000_000_000_000_000),
        ClockFaults {
            jump: SOMETIMES,
            jump_size: Duration::from_secs(3_600),
            skew_per_million: 250,
        },
    ));
    let latency = Duration::from_millis(40);
    let object_store = SimObjectStore::new(
        seed + 1,
        ObjectStoreFaults {
            error: SOMETIMES,
            latency: SOMETIMES,
            latency_duration: latency,
            partial: SOMETIMES,
            lost_write: SOMETIMES,
        },
        Arc::clone(&clock),
    );
    let network = network(seed + 2, latency, Arc::clone(&clock));
    let process = SimProcess::new();
    process.register("duckdb", output(b"v1.4.0"));
    let sql_parser = SimSqlParser::new();
    sql_parser.record("SELECT 1", r#"{"error":false,"statements":[]}"#);
    Ports {
        clock,
        file_system: Arc::new(SimFileSystem::new()),
        object_store: Arc::new(object_store),
        network: Arc::new(network),
        source: Arc::new(source(seed + 3)),
        random: Arc::new(SimRandom::new(seed + 4)),
        process: Arc::new(process),
        environment: Arc::new(
            SimEnvironment::new([("MERGUS_PROFILE", "dev")], None)
                .with_args(["validate", "--json"])
                .with_current_dir("/project"),
        ),
        sql_parser: Arc::new(sql_parser),
    }
}

fn network(seed: u64, latency: Duration, clock: Arc<SimClock>) -> SimNetwork {
    let network = SimNetwork::new(
        seed,
        NetworkFaults {
            error: SOMETIMES,
            latency: SOMETIMES,
            latency_duration: latency,
            partial: SOMETIMES,
        },
        clock,
    );
    network.route("https://api.example/items", response(b"[1,2,3,4,5,6,7,8]"));
    network.listen("db.example", 5432, Arc::new(|bytes: &[u8]| bytes.to_vec()));
    network
}

fn source(seed: u64) -> SimSourceConnector {
    let source = SimSourceConnector::new(
        seed,
        SourceFaults {
            fail: SOMETIMES,
            partial: SOMETIMES,
            change: SOMETIMES,
        },
    );
    let batches = |tag: &str| (0..4).map(|i| format!("{tag}{i}").into_bytes()).collect();
    source.register("orders", batches("v1-"), Some(batches("v2-")));
    source
}

fn response(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: body.to_vec(),
    }
}

fn output(stdout: &[u8]) -> ProcessOutput {
    ProcessOutput {
        code: Some(0),
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

/// Sends one HTTP request and one TCP exchange.
fn network_round(
    network: &dyn Network,
) -> (Result<HttpResponse, PortError>, Result<Vec<u8>, PortError>) {
    let request = HttpRequest {
        method: "GET".to_owned(),
        url: "https://api.example/items".to_owned(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let tcp = network
        .tcp_connect("db.example", 5432)
        .and_then(|mut connection| {
            connection.send(b"ping-pong")?;
            connection.receive(64)
        });
    (network.http(&request), tcp)
}

/// Uses every port many times and records what each call returned.
fn trace(ports: &Ports) -> Result<String, PortError> {
    let mut trace = String::new();
    let dir = Path::new("/project");
    ports.file_system.create_dir_all(dir)?;
    for round in 0..50_u32 {
        let file = dir.join(format!("{}.txt", round % 3));
        let write = ports.file_system.write(&file, &round.to_le_bytes());
        let read = ports.file_system.read(&file);
        let key = format!("data/{}", round % 5);
        let put = ports.object_store.put(&key, b"0123456789");
        let get = ports.object_store.get(&key);
        let list = ports.object_store.list("data/");
        let (http, tcp) = network_round(ports.network.as_ref());
        let mut batches = Vec::new();
        if let Ok(mut reader) = ports.source.open("orders") {
            while let Ok(Some(batch)) = reader.next_batch() {
                batches.push(batch);
            }
        }
        ports.clock.sleep(Duration::from_millis(10));
        let mut bytes = [0_u8; 5];
        ports.random.fill(&mut bytes);
        let command = ProcessCommand {
            program: "duckdb".to_owned(),
            ..ProcessCommand::default()
        };
        trace.push_str(&format!(
            "{write:?} {read:?} {put:?} {get:?} {list:?} {http:?} {tcp:?} {batches:?} \
             {:?} {:?} {:?} {bytes:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}\n",
            ports.clock.now(),
            ports.clock.monotonic(),
            ports.random.next_u64(),
            ports.process.run(&command),
            ports.environment.var("MERGUS_PROFILE"),
            ports.environment.args(),
            ports.environment.current_dir(),
            ports.file_system.list_dir(dir),
            ports.sql_parser.parse("SELECT 1"),
            ports.sql_parser.parse("SELECT 2"),
        ));
    }
    Ok(trace)
}

#[test]
fn mg_req_0131_every_port_is_simulated() {
    let first = trace(&simulated(42)).unwrap();
    assert_eq!(
        first,
        trace(&simulated(42)).unwrap(),
        "same seed, same results"
    );
    assert_ne!(
        first,
        trace(&simulated(43)).unwrap(),
        "another seed, other results"
    );
    assert!(first.contains("simulated object-store error"), "{first}");
    assert!(first.contains("simulated network error"), "{first}");
}
