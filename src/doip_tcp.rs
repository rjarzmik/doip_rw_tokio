use std::net::SocketAddr;
use std::time::Duration;

use crate::connection as cnx;
use crate::{message::DoIpMessage, DoIpCnxError, DoIpTcpMessage};
use doip_rw::{message, message::UdsBuffer, LogicalAddress, PayloadType};
use log::info;
use tokio::{
    io,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpSocket,
    },
    time,
};

/// DoIpTcpConnection
///
/// The connected DoIP stream, supporting mainly DoIP diagnostic messages
/// through [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage).
pub struct DoIpTcpConnection {
    /// The internal connection.
    inner: Inner,
}

#[derive(Clone)]
/// Timings
///
/// Timings for connection establishement, and more specifically timeouts.
pub struct Timings {
    /// Maximum time for the TCP connection to be established. The caller will
    /// return [`ConnectionTimeout`](DoIpCnxError::ConnectionTimeout) if
    /// connection times out.
    pub tcp_connect: Duration,
    /// Maximum time for the TCP connection to be established.
    pub routing_activation_rsp: Duration,
}

#[derive(Clone)]
struct Addresses {
    pub local_addr: SocketAddr,
    pub remote_addr: SocketAddr,
    pub la: LogicalAddress,
}

impl DoIpTcpConnection {
    /// Create a client DoIP over TCP connection.
    ///
    /// This is the client DoIP way of submitting and aswering to
    /// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage)
    /// messages, used by DoIP external tester.
    pub async fn connect_doip_tcp(
        local_addr: SocketAddr,
        remote_addr: SocketAddr,
        la: LogicalAddress,
        timings: Timings,
    ) -> Result<DoIpTcpConnection, DoIpCnxError> {
        let inner = connect_doip(local_addr, remote_addr, la, timings).await?;
        Ok(DoIpTcpConnection { inner })
    }

    /// Reconnect a deconnected DoIP over TCP connection.
    ///
    /// Reuses the same parameters as previously provided to
    /// [`connect_doip_tcp()`](Self::connect_doip_tcp), and reconnects in the
    /// same way as before.
    pub async fn reconnect_doip_tcp(&self) -> Result<DoIpTcpConnection, DoIpCnxError> {
        let inner = connect_doip(
            self.inner.addrs.local_addr,
            self.inner.addrs.remote_addr,
            self.inner.addrs.la,
            self.inner.timings.clone(),
        )
        .await?;
        Ok(DoIpTcpConnection { inner })
    }

    /// Create a server DoIP over TCP connection.
    ///
    /// Accepts the first TCP connection on a TCP listening socket.
    ///
    pub async fn accept_doip_tcp<F>(
        listener: &TcpListener,
        la: LogicalAddress,
        timings: Timings,
        should_accept: F,
    ) -> Result<DoIpTcpConnection, DoIpCnxError>
    where
        F: Fn(&message::RoutingActivationRequest) -> bool,
    {
        let addrs = Addresses {
            local_addr: "0.0.0.0:0".parse().unwrap(),
            remote_addr: "0.0.0.0:0".parse().unwrap(),
            la,
        };
        let inner = accept_doip(listener, addrs, timings, should_accept).await?;
        Ok(DoIpTcpConnection { inner })
    }

    /// Main and only receiving function.
    ///
    /// Receive a whole DoIP message from a DoIP stream, decode it and create
    /// the associated [`DoIpTcpMessage`].
    ///
    /// The specific case of a diagnostic requires an allocator `allocate_uds`,
    /// which will be called at most once if a received message is either :
    /// - [`DoIpTcpMessage::DiagnosticMessage`]
    /// - [`DoIpTcpMessage::DiagnosticMessagePositiveAck`]
    /// - [`DoIpTcpMessage::DiagnosticMessageNegativeAck`]
    pub async fn receive_message(
        &mut self,
        allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
    ) -> Result<DoIpTcpMessage<'static>, DoIpCnxError> {
        let (reader, buffer) = (&mut self.inner.tcp_reader, &mut self.inner.receive_buffer);
        match cnx::receive_message(reader, buffer, allocate_uds).await? {
            DoIpMessage::Tcp(t) => Ok(t),
            DoIpMessage::Udp(_) => Err(DoIpCnxError::InvalidMessageReceived),
        }
    }

    /// Send alive check request.
    ///
    /// Send a [`AliveCheckRequest`](doip_rw::message::AliveCheckRequest) DoIP message.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: [`AliveCheckResponse`](doip_rw::message::AliveCheckResponse).
    pub async fn send_alive_check_request(&mut self) -> io::Result<()> {
        let (writer, buffer) = (&mut self.inner.tcp_writer, &mut self.inner.send_buffer);
        cnx::send_alive_check_request(writer, buffer).await
    }

    /// Send alive check response.
    ///
    /// Send a [`AliveCheckResponse`](doip_rw::message::AliveCheckResponse) DoIP message.
    ///
    /// Caller: only a DoIP external tester can issue it.
    /// Expected answer: None
    pub async fn send_alive_check_response(&mut self, la: LogicalAddress) -> io::Result<()> {
        let (writer, buffer) = (&mut self.inner.tcp_writer, &mut self.inner.send_buffer);
        cnx::send_alive_check_response(writer, la, buffer).await
    }

    /// Send a diagnostic request or diagnostic response.
    ///
    /// Send a [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage)
    /// DoIP message.
    ///
    /// Caller:
    /// - a DoIP external tester can issue it (as a request).
    /// - a DoIP entity can issue it (as a response).
    ///
    /// Expected answer:
    /// - when it is the diagnostic request:
    /// [`DiagnosticMessagePositiveAck`](struct@doip_rw::message::DiagnosticMessagePositiveAck)
    /// followed by a
    /// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage).
    /// - when it is the diagnostic response: None.
    pub async fn send_diagnostic_request<'a>(
        &mut self,
        ta: LogicalAddress,
        uds: UdsBuffer<'a>,
    ) -> io::Result<()> {
        let (sa, writer, buffer) = (
            self.inner.addrs.la,
            &mut self.inner.tcp_writer,
            &mut self.inner.send_buffer,
        );
        cnx::send_diagnostic_request(writer, sa, ta, uds, buffer).await
    }

    /// Send a diagnostic message acknowledgement.
    ///
    /// Send a
    /// [`DiagnosticMessagePositiveAck`](struct@doip_rw::message::DiagnosticMessagePositiveAck)
    /// DoIP message, as a positive response to a
    /// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage). It
    /// will then be followed by a
    /// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage) for
    /// the reply.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: None
    pub async fn send_diagnostic_acknowledge(&mut self, ta: LogicalAddress) -> io::Result<()> {
        let (sa, writer, buffer) = (
            self.inner.addrs.la,
            &mut self.inner.tcp_writer,
            &mut self.inner.send_buffer,
        );
        cnx::send_diagnostic_acknowledge(writer, sa, ta, buffer).await
    }

    /// Send a diagnostic message negative acknowledgement.
    ///
    /// Send a
    /// [`DiagnosticMessageNegativeAck`](struct@doip_rw::message::DiagnosticMessageNegativeAck)
    /// DoIP message, as a negative response to a
    /// [`DiagnosticMessage`](struct@doip_rw::message::DiagnosticMessage). This
    /// means the DoIP refuses the DoIP message, regardless of its UDS payload.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: None
    pub async fn send_diagnostic_deny(
        &mut self,
        ta: LogicalAddress,
        code: message::DiagnosticMessageNegativeAckCode,
    ) -> io::Result<()> {
        let (sa, writer, buffer) = (
            self.inner.addrs.la,
            &mut self.inner.tcp_writer,
            &mut self.inner.send_buffer,
        );
        cnx::send_diagnostic_deny(writer, sa, ta, code, buffer).await
    }
}

struct Inner {
    addrs: Addresses,
    timings: Timings,
    tcp_reader: OwnedReadHalf,
    tcp_writer: OwnedWriteHalf,
    send_buffer: Vec<u8>,
    receive_buffer: Vec<u8>,
}

async fn connect_doip(
    local_addr: SocketAddr,
    remote_addr: SocketAddr,
    la: LogicalAddress,
    timings: Timings,
) -> Result<Inner, DoIpCnxError> {
    let addrs = Addresses {
        local_addr,
        remote_addr,
        la,
    };
    let socket = TcpSocket::new_v4().map_err(DoIpCnxError::ConnectionError)?;
    socket
        .bind(addrs.local_addr)
        .map_err(DoIpCnxError::ConnectionError)?;
    let stream = time::timeout(timings.tcp_connect, socket.connect(addrs.remote_addr))
        .await
        .map_err(|_| DoIpCnxError::ConnectionTimeout)?
        .map_err(DoIpCnxError::ConnectionError)?;
    info!(
        target: "doip_tcp",
        "DoIP connection established {}->{}", addrs.local_addr, addrs.remote_addr
    );
    let (tcp_reader, tcp_writer) = stream.into_split();
    let mut inner = Inner {
        addrs,
        timings,
        tcp_reader,
        tcp_writer,
        send_buffer: vec![],
        receive_buffer: vec![],
    };

    match request_routing_activation(&mut inner).await {
        Ok(_) => {
            info!(
		target: "doip_tcp",
		"DoIP routing activation succeeded");
            Ok(())
        }
        Err(e) => {
            info!(
		target: "doip_tcp",
		"DoIP routing activation failed: {:?}", e);
            Err(e)
        }
    }?;

    Ok(inner)
}

async fn request_routing_activation(inner: &mut Inner) -> Result<(), DoIpCnxError> {
    cnx::send_routing_activation_request(
        &mut inner.tcp_writer,
        inner.addrs.la,
        &mut inner.send_buffer,
    )
    .await?;
    let rsp_timeout = inner.timings.routing_activation_rsp;
    fn allocate(_pt: PayloadType, size: usize) -> Vec<u8> {
        vec![0; size]
    }
    let msg = time::timeout(
        rsp_timeout,
        cnx::receive_message(&mut inner.tcp_reader, &mut inner.receive_buffer, allocate),
    )
    .await
    .map_err(|_| DoIpCnxError::ConnectionTimeout)?
    .map_err(|_| DoIpCnxError::RoutingActivationFailed)?;

    let rarsp = match msg {
        DoIpMessage::Tcp(t) => {
            if let DoIpTcpMessage::RoutingActivationResponse(rarsp) = t {
                Ok(rarsp)
            } else {
                Err(DoIpCnxError::RoutingActivationFailed)
            }
        }
        _ => Err(DoIpCnxError::RoutingActivationFailed),
    }?;

    if rarsp.routing_activation_response_code
        == message::RoutingActivationResponseCode::RoutingSuccessfullyActivated
    {
        Ok(())
    } else {
        Err(DoIpCnxError::RoutingActivationFailed)
    }
}

async fn accept_doip<F>(
    listener: &TcpListener,
    addrs: Addresses,
    timings: Timings,
    should_accept: F,
) -> Result<Inner, DoIpCnxError>
where
    F: Fn(&message::RoutingActivationRequest) -> bool,
{
    let (stream, client_addr) = time::timeout(timings.tcp_connect, listener.accept())
        .await
        .map_err(|_| DoIpCnxError::ConnectionTimeout)?
        .map_err(DoIpCnxError::ConnectionError)?;
    info!(
        target: "doip_tcp",
        "DoIP connection accepted {}->{}", client_addr, addrs.local_addr
    );
    let (tcp_reader, tcp_writer) = stream.into_split();
    let mut inner = Inner {
        addrs,
        timings,
        tcp_reader,
        tcp_writer,
        send_buffer: vec![],
        receive_buffer: vec![],
    };

    match respond_routing_activation(&mut inner, should_accept).await {
        Ok(_) => {
            info!(
		target: "doip_tcp",
		"DoIP routing activation accepted");
            Ok(())
        }
        Err(e) => {
            info!(
		target: "doip_tcp",
		"DoIP routing activation failed: {:?}", e);
            Err(e)
        }
    }?;

    Ok(inner)
}

async fn respond_routing_activation<F>(
    inner: &mut Inner,
    should_accept: F,
) -> Result<(), DoIpCnxError>
where
    F: Fn(&message::RoutingActivationRequest) -> bool,
{
    let rsp_timeout = inner.timings.routing_activation_rsp;
    fn allocate(_pt: PayloadType, _size: usize) -> Vec<u8> {
        vec![]
    }

    let msg = time::timeout(
        rsp_timeout,
        cnx::receive_message(&mut inner.tcp_reader, &mut inner.receive_buffer, allocate),
    )
    .await
    .map_err(|_| DoIpCnxError::ConnectionTimeout)?
    .map_err(|_| DoIpCnxError::RoutingActivationFailed)?;

    let rareq = match msg {
        DoIpMessage::Tcp(t) => {
            if let DoIpTcpMessage::RoutingActivationRequest(rareq) = t {
                Ok(rareq)
            } else {
                Err(DoIpCnxError::RoutingActivationFailed)
            }
        }
        _ => Err(DoIpCnxError::RoutingActivationFailed),
    }?;

    if should_accept(&rareq) {
        cnx::send_routing_activation_response(
            &mut inner.tcp_writer,
            rareq.source_address,
            inner.addrs.la,
            message::RoutingActivationResponseCode::RoutingSuccessfullyActivated,
            &mut inner.send_buffer,
        )
        .await?;
    } else {
        cnx::send_routing_activation_response(
            &mut inner.tcp_writer,
            rareq.source_address,
            inner.addrs.la,
            message::RoutingActivationResponseCode::RoutingActivationDeniedRejectedConfirmation,
            &mut inner.send_buffer,
        )
        .await?;
    }
    Ok(())
}
