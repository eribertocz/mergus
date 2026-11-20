//! The `Network` port: HTTP and TCP connections.

use crate::PortError;

/// An HTTP request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    /// The method, such as `GET`.
    pub method: String,
    /// The absolute URL.
    pub url: String,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// An HTTP response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpResponse {
    /// The status code.
    pub status: u16,
    /// The headers, in order.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

/// An open TCP connection.
pub trait Connection: Send {
    /// Sends `bytes`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the connection fails.
    fn send(&mut self, bytes: &[u8]) -> Result<(), PortError>;

    /// Receives at most `max` bytes; an empty result means the peer closed the connection.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the connection fails.
    fn receive(&mut self, max: usize) -> Result<Vec<u8>, PortError>;
}

/// HTTP and TCP connections.
pub trait Network: Send + Sync {
    /// Sends an HTTP request and returns its response.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the host does not exist, and with
    /// [`PortError::Failed`] when the request fails.
    fn http(&self, request: &HttpRequest) -> Result<HttpResponse, PortError>;

    /// Opens a TCP connection to `host` on `port`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the host does not exist, and with
    /// [`PortError::Failed`] when the connection cannot be opened.
    fn tcp_connect(&self, host: &str, port: u16) -> Result<Box<dyn Connection>, PortError>;
}
