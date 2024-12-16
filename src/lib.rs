#![warn(missing_docs)]
//! DoIP tokio messaging library
//!
//! This library offers an API to connect, receive connection, send and receive
//! DoIP messages, both on UDP and TCP.
//!
//! It enables async operations thanks to tokio framework.
//!
//! An example is provided in the examples directory, implementing a very simple
//! DoIP entity accepting connections, and replying to DoIP requests.
//!
//! The API is thread agnostics and memory allocation agnostic. When a buffer is
//! required, the API will require an allocator from the caller. This way, the
//! caller might have a ring buffer, and reuse buffers, preventing any memory
//! allocation.
//!
//! The first usage of the API should always be either :
//! - [`connect_doip_tcp()`] for the TCP DoIP stream for an external tester.
//! - [`accept_doip_tcp()`] for the TCP DoIP stream for a DoIP entity.
//! - [`create_doip_udp()`] for the UDP DoIP stream.

mod connection;
mod diagnostic_message;
mod doip_tcp;
mod doip_udp;
mod message;
mod tests;
mod uds;

use std::net::SocketAddr;

#[derive(Debug)]
/// A DoIP API error
pub enum DoIpCnxError {
    /// A TCP connection error.
    ConnectionError(tokio::io::Error),
    /// A TCP connection timeout.
    ConnectionTimeout,
    /// If a message encoding fails (which it shouldn't), this error is returned.
    InvalidMessageToSend,
    /// If a received message is inconsistent (for example the payload_length
    /// doesn't match the payload_type, this error is raised.
    InvalidMessageReceived,
    /// If a DoIP connection has its initial RoutingActivation denied.
    RoutingActivationFailed,
    /// If a send timeouts, for example if network buffers are full.
    SendTimeout,
}

use doip_rw::LogicalAddress;
use tokio::net::{TcpListener, UdpSocket};

pub use crate::doip_tcp::{DoIpTcpConnection, Timings};
pub use crate::doip_udp::DoIpUdpConnection;
pub use crate::message::DoIpTcpMessage;
pub use crate::message::DoIpUdpMessage;
pub use crate::uds::{send_uds, receive_uds};

/// Create a client DoIP over TCP connection.
///
/// This is the client DoIP way of submitting and aswering to
/// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage)
/// messages, used by DoIP external tester.
///
/// This is the main way to access the diagnostic over IP stream.
///
/// The call creates a TCP connection, connects the TCP socket, sends the
/// routing activation, and waits for an answer. Upon receiving the answer, if
/// the routing response is positive, a [`DoIpTcpConnection`] is returned.
///
/// # Errors
///
/// - [`DoIpCnxError::RoutingActivationFailed] is the routing returned a negative
/// response.
/// - [`DoIpCnxError::ConnectionTimeout`] if the TCP connection cannot be
/// established in the provided duration.
/// - [`DoIpCnxError::ConnectionError`] if the TCP connection cannot be
/// established (think network error here).
pub async fn connect_doip_tcp(
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
    la: LogicalAddress,
    timings: Timings,
) -> Result<DoIpTcpConnection, DoIpCnxError> {
    DoIpTcpConnection::connect_doip_tcp(local_addr, remote_addr, la, timings).await
}

/// Accept a connection from a client for a DoIP over TCP connection.
///
/// This is the server side DoIP way to aswer to
/// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage) messages,
/// used by DoIP entity.
///
/// This is the main way to access the diagnostic over IP stream as a DoIP
/// entity.
///
/// The call accepts an incomming TCP connection, sends the routing activation
/// response according to `should_accept`, and returns a [`DoIpTcpConnection`].
/// Depending on `should_accept()` result, the
/// [`RoutingActivationResponse`](doip_rw::message::RoutingActivationResponse)
/// will either convey a
/// [`RoutingSuccessfullyActivated`](doip_rw::message::RoutingActivationResponseCode::RoutingSuccessfullyActivated)
/// or a
/// [`RoutingActivationDeniedRejectedConfirmation`](doip_rw::message::RoutingActivationResponseCode::RoutingActivationDeniedRejectedConfirmation).
///
/// # Errors
///
/// - [`DoIpCnxError::RoutingActivationFailed] is the first incoming routing
/// activation request is either malformed or is another DoIP message.
/// - [`DoIpCnxError::ConnectionTimeout`] if the TCP connection cannot be
/// established in the provided duration, or the routing activation request
/// doesn't come before the timeout.
/// - [`DoIpCnxError::ConnectionError`] if the TCP connection cannot be
/// established (think network error here).
pub async fn accept_doip_tcp<F>(
    listener: &TcpListener,
    la: LogicalAddress,
    timings: Timings,
    should_accept: F,
) -> Result<DoIpTcpConnection, DoIpCnxError>
where
    F: Fn(&doip_rw::message::RoutingActivationRequest) -> bool,
{
    DoIpTcpConnection::accept_doip_tcp(listener, la, timings, should_accept).await
}

/// Create a client DoIP over UDP local endpoint.
///
/// This endpoint might be used to send both unicast and multicast/broadcast
/// messages.
///
/// This function expects the caller to have created beforehand the source
/// socket, and set the broadcast capability if broadcasts have to be sent.  It
/// doesn't do any IO.
pub fn create_doip_udp(local_socket: UdpSocket) -> DoIpUdpConnection {
    DoIpUdpConnection::new(local_socket)
}
