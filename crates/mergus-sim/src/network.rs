//! A simulated network: errors, latency and partial responses.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mergus_ports::{Clock, Connection, HttpRequest, HttpResponse, Network, PortError};

use crate::SimClock;
use crate::rng::{Rate, SimRng, lock};

/// The network faults to inject.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkFaults {
    /// How often a request, a connection or a transfer fails.
    pub error: Rate,
    /// How often a call first waits `latency_duration` on the simulated clock.
    pub latency: Rate,
    /// How long a slow call waits.
    pub latency_duration: Duration,
    /// How often a response body or a received chunk is cut short.
    pub partial: Rate,
}

/// What a simulated TCP peer answers to the bytes it receives.
pub type TcpHandler = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

struct Shared {
    faults: NetworkFaults,
    clock: Arc<SimClock>,
    rng: Mutex<SimRng>,
}

impl Shared {
    /// Draws the latency and error faults of one call, then whether it is cut short.
    fn enter(&self, what: &str) -> Result<bool, PortError> {
        let mut rng = lock(&self.rng);
        if rng.hits(self.faults.latency) {
            self.clock.sleep(self.faults.latency_duration);
        }
        let fails = rng.hits(self.faults.error);
        let partial = rng.hits(self.faults.partial);
        if fails {
            return Err(PortError::Failed(format!(
                "simulated network error on {what}"
            )));
        }
        Ok(partial)
    }

    /// Returns a random length shorter than `length`, or 0 when `length` is 0.
    fn shorter(&self, length: usize) -> usize {
        let bound = u64::try_from(length).unwrap_or(u64::MAX);
        usize::try_from(lock(&self.rng).below(bound)).unwrap_or(0)
    }
}

/// HTTP routes and TCP peers registered by the test, with injected faults. Latency advances
/// the shared [`SimClock`]. Unknown URLs and peers are not found.
pub struct SimNetwork {
    shared: Arc<Shared>,
    routes: Mutex<BTreeMap<String, HttpResponse>>,
    peers: Mutex<BTreeMap<(String, u16), TcpHandler>>,
}

impl fmt::Debug for SimNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SimNetwork")
            .field("faults", &self.shared.faults)
            .finish_non_exhaustive()
    }
}

impl SimNetwork {
    /// Returns a network with no route and no peer.
    #[must_use]
    pub fn new(seed: u64, faults: NetworkFaults, clock: Arc<SimClock>) -> Self {
        Self {
            shared: Arc::new(Shared {
                faults,
                clock,
                rng: Mutex::new(SimRng::new(seed)),
            }),
            routes: Mutex::new(BTreeMap::new()),
            peers: Mutex::new(BTreeMap::new()),
        }
    }

    /// Makes every later request to `url` return `response`.
    pub fn route(&self, url: &str, response: HttpResponse) {
        lock(&self.routes).insert(url.to_owned(), response);
    }

    /// Makes `handler` answer the TCP connections to `host` on `port`.
    pub fn listen(&self, host: &str, port: u16, handler: TcpHandler) {
        lock(&self.peers).insert((host.to_owned(), port), handler);
    }
}

impl Network for SimNetwork {
    fn http(&self, request: &HttpRequest) -> Result<HttpResponse, PortError> {
        let partial = self.shared.enter(&request.url)?;
        let mut response = lock(&self.routes)
            .get(&request.url)
            .cloned()
            .ok_or_else(|| PortError::NotFound(request.url.clone()))?;
        if partial {
            let kept = self.shared.shorter(response.body.len());
            response.body.truncate(kept);
        }
        Ok(response)
    }

    fn tcp_connect(&self, host: &str, port: u16) -> Result<Box<dyn Connection>, PortError> {
        self.shared.enter(&format!("{host}:{port}"))?;
        let handler = lock(&self.peers)
            .get(&(host.to_owned(), port))
            .cloned()
            .ok_or_else(|| PortError::NotFound(format!("{host}:{port}")))?;
        Ok(Box::new(SimConnection {
            shared: Arc::clone(&self.shared),
            handler,
            pending: VecDeque::new(),
        }))
    }
}

/// A connection to a simulated peer: each send queues the peer's answer, and a partial
/// receive keeps the rest of the answer for the next one.
struct SimConnection {
    shared: Arc<Shared>,
    handler: TcpHandler,
    pending: VecDeque<u8>,
}

impl Connection for SimConnection {
    fn send(&mut self, bytes: &[u8]) -> Result<(), PortError> {
        self.shared.enter("send")?;
        self.pending.extend((self.handler)(bytes));
        Ok(())
    }

    fn receive(&mut self, max: usize) -> Result<Vec<u8>, PortError> {
        let partial = self.shared.enter("receive")?;
        let mut chunk: Vec<u8> = self.pending.drain(..max.min(self.pending.len())).collect();
        if partial && chunk.len() > 1 {
            let kept = self.shared.shorter(chunk.len()).max(1);
            for byte in chunk.drain(kept..).rev() {
                self.pending.push_front(byte);
            }
        }
        Ok(chunk)
    }
}
