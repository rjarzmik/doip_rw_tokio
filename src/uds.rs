use std::time::Duration;

use crate::DoIpTcpMessage::{self, *};
use crate::{DoIpCnxError, DoIpTcpConnection};
use doip_rw::{message::UdsBuffer, LogicalAddress, PayloadType};

use tokio::time;

/// Send an UDS diagnostic message.
///
/// Send a [`DiagnosticMessage`] containing the UDS provided request, and
/// returns the acknowledge received. The function is only to be used by a DoIP
/// external tester.
///
/// The provided allocator `allocate_uds` is used to store the repeated UDS
/// message in the to be received [`DiagnosticMessagePositiveAck`] or
/// [`DiagnosticMessageNegativeAck`].  A duration `ack_timeout` can be provided
/// to limit the wait time.
///
/// This function should be followed by another `receive_message()` call to
/// receive the UDS reply.
pub async fn send_uds<'a, F>(
    cnx: &'a mut DoIpTcpConnection,
    ta: LogicalAddress,
    uds: UdsBuffer<'a>,
    allocate_uds: F,
    ack_timeout: Duration,
) -> Result<DoIpTcpMessage<'static>, DoIpCnxError>
where
    F: FnOnce(PayloadType, usize) -> Vec<u8>,
{
    cnx.send_diagnostic_request(ta, uds).await?;
    let msg = time::timeout(ack_timeout, cnx.receive_message(allocate_uds))
        .await
        .map_err(|_| DoIpCnxError::SendTimeout)??;
    match &msg {
        DiagnosticMessagePositiveAck(_) => Ok(msg),
        DiagnosticMessageNegativeAck(_) => Ok(msg),
        _ => Err(DoIpCnxError::InvalidMessageReceived),
    }
}

/// Receive an UDS diagnostic message.
///
/// Receives a DoIP tcp message, "guessing" it will be a
/// [`DiagnosticMessage`]. If it is, extract the UDS paylaod and return it. If
/// it's not, discard the message and return an error.
///
/// # Warning
///
/// This function is extremely dangerous to use and shouldn't in fact be used,
/// the correct usage would be to use
/// [`DoIpTcpConnection::receive_message()`]. The reason is that if either of
/// these message arrives, it will be lost and reported as an eror :
/// - [`DiagnosticMessagePositiveAck`]
/// - [`DiagnosticMessageNegativeAck`]
/// - [`AliveCheckRequest`]
/// - [`AliveCheckResponse`]
///
/// The provided allocator `allocate_uds` is used to store the received UDS
/// message.
pub async fn receive_uds<'a, F>(
    cnx: &'a mut DoIpTcpConnection,
    allocate_uds: F,
) -> Result<UdsBuffer<'static>, DoIpCnxError>
where
    F: FnOnce(PayloadType, usize) -> Vec<u8>,
{
    match cnx.receive_message(allocate_uds).await? {
        DiagnosticMessage(msg) => Ok(msg.user_data),
        _ => Err(DoIpCnxError::InvalidMessageReceived),
    }
}
