use super::diagnostic_message::AsyncUdsPayload;
use doip_rw::{message::*, read_header, read_payload};
use doip_rw::{write_message, DoIpError};
use doip_rw::{BorrowedPayload, LogicalAddress, Payload, PayloadType};
use log::trace;
use std::io::Cursor;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{message::DoIpMessage, DoIpCnxError, DoIpTcpMessage, DoIpUdpMessage};
pub fn decode_header(buffer: &[u8]) -> Result<(PayloadType, usize), DoIpCnxError> {
    let hdr =
        read_header(&mut Cursor::new(&buffer)).map_err(|_| DoIpCnxError::InvalidMessageReceived)?;
    let plen = hdr.payload_length as usize;
    Ok((hdr.payload_type, plen))
}

fn is_diagnostic_kind_message(payload_type: PayloadType) -> bool {
    matches!(
        payload_type,
        PayloadType::DiagnosticMessage
            | PayloadType::DiagnosticMessageNegativeAcknowledgement
            | PayloadType::DiagnosticMessagePositiveAcknowledgement
    )
}

pub fn decode_doip_payload(
    buffer: &mut [u8],
    payload_type: PayloadType,
) -> Result<DoIpMessage, DoIpCnxError> {
    if is_diagnostic_kind_message(payload_type) {
        decode_diagnostic_message(buffer, payload_type)
    } else {
        decode_doip_not_diagnostic(buffer, payload_type)
    }
}

pub async fn read_doip_payload<R>(
    reader: &mut R,
    buffer: &mut [u8],
    payload_type: PayloadType,
    allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
) -> Result<DoIpMessage<'static>, DoIpCnxError>
where
    R: AsyncReadExt + Unpin,
{
    if is_diagnostic_kind_message(payload_type) {
        read_diagnostickind_message(reader, payload_type, buffer.len(), allocate_uds).await
    } else {
        reader.read_exact(buffer).await?;
        decode_doip_not_diagnostic(buffer, payload_type)
    }
}

async fn read_diagnostickind_message<'a, R>(
    reader: &mut R,
    payload_type: PayloadType,
    plen: usize,
    allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
) -> Result<DoIpMessage<'a>, DoIpCnxError>
where
    R: AsyncReadExt + Unpin,
{
    match payload_type {
        PayloadType::DiagnosticMessage => {
            let msg = DiagnosticMessage::async_read(reader, plen, allocate_uds).await?;
            Ok(DoIpMessage::Tcp(DoIpTcpMessage::DiagnosticMessage(msg)))
        }
        PayloadType::DiagnosticMessagePositiveAcknowledgement => {
            let ack = DiagnosticMessagePositiveAck::async_read(reader, plen, allocate_uds).await?;
            Ok(DoIpMessage::Tcp(
                DoIpTcpMessage::DiagnosticMessagePositiveAck(ack),
            ))
        }
        PayloadType::DiagnosticMessageNegativeAcknowledgement => {
            let nack = DiagnosticMessageNegativeAck::async_read(reader, plen, allocate_uds).await?;
            Ok(DoIpMessage::Tcp(
                DoIpTcpMessage::DiagnosticMessageNegativeAck(nack),
            ))
        }
        _ => panic!(
            "This function read_diagnostic_message({:?}) should never Be called that way.",
            payload_type
        ),
    }
}

fn decode_diagnostic_message(
    buffer: &mut [u8],
    payload_type: PayloadType,
) -> Result<DoIpMessage, DoIpCnxError> {
    match payload_type {
        PayloadType::DiagnosticMessage => DiagnosticMessage::read_borrowed(buffer)
            .map(DoIpTcpMessage::DiagnosticMessage)
            .map(DoIpMessage::Tcp),
        PayloadType::DiagnosticMessagePositiveAcknowledgement => {
            DiagnosticMessagePositiveAck::read_borrowed(buffer)
                .map(DoIpTcpMessage::DiagnosticMessagePositiveAck)
                .map(DoIpMessage::Tcp)
        }
        PayloadType::DiagnosticMessageNegativeAcknowledgement => {
            DiagnosticMessageNegativeAck::read_borrowed(buffer)
                .map(DoIpTcpMessage::DiagnosticMessageNegativeAck)
                .map(DoIpMessage::Tcp)
        }
        _ => panic!(
            "This function read_diagnostic_message({:?}) should never Be called that way.",
            payload_type
        ),
    }
    .map_err(DoIpCnxError::from)
}

fn decode_doip_not_diagnostic(
    payload: &[u8],
    payload_type: PayloadType,
) -> Result<DoIpMessage<'static>, DoIpCnxError> {
    let plen = payload.len();
    let mut reader = Cursor::new(payload);

    match payload_type {
        PayloadType::GenericDoIpHeaderNegativeAcknowledge => Err(DoIpError::BufferTooSmall),
        PayloadType::VehicleIdentificationRequest => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::VehicleIdentificationRequest)
            .map(DoIpMessage::Udp),
        PayloadType::VehicleIdentificationRequestWithEid => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::VehicleIdentificationRequestWithEid)
            .map(DoIpMessage::Udp),
        PayloadType::VehicleIdentificationRequestWithVin => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::VehicleIdentificationRequestWithVin)
            .map(DoIpMessage::Udp),
        PayloadType::VehicleIdentificationResponse => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::VehicleIdentificationResponse)
            .map(DoIpMessage::Udp),
        PayloadType::RoutingActivationRequest => read_payload(&mut reader, plen)
            .map(DoIpTcpMessage::RoutingActivationRequest)
            .map(DoIpMessage::Tcp),
        PayloadType::RoutingActivationResponse => read_payload(&mut reader, plen)
            .map(DoIpTcpMessage::RoutingActivationResponse)
            .map(DoIpMessage::Tcp),
        PayloadType::AliveCheckRequest => read_payload(&mut reader, plen)
            .map(DoIpTcpMessage::AliveCheckRequest)
            .map(DoIpMessage::Tcp),
        PayloadType::AliveCheckResponse => read_payload(&mut reader, plen)
            .map(DoIpTcpMessage::AliveCheckResponse)
            .map(DoIpMessage::Tcp),
        PayloadType::DoIpEntityStatusRequest => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::EntityStatusRequest)
            .map(DoIpMessage::Udp),
        PayloadType::DoIpEntityStatusResponse => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::EntityStatusResponse)
            .map(DoIpMessage::Udp),
        PayloadType::DiagnosticPowerModeInformationRequest => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::PowerModeRequest)
            .map(DoIpMessage::Udp),
        PayloadType::DiagnosticPowerModeInformationResponse => read_payload(&mut reader, plen)
            .map(DoIpUdpMessage::PowerModeResponse)
            .map(DoIpMessage::Udp),
        PayloadType::DiagnosticMessage => {
            panic!("read_doip_payload(): shouldn't be called with diagnostic message")
        }
        PayloadType::DiagnosticMessagePositiveAcknowledgement => {
            panic!("read_doip_payload(): shouldn't be called with diagnostic message")
        }
        PayloadType::DiagnosticMessageNegativeAcknowledgement => {
            panic!("read_doip_payload(): shouldn't be called with diagnostic message")
        }
        _ => Err(DoIpError::BufferTooSmall),
    }
    .map_err(|_| DoIpCnxError::InvalidMessageReceived)
}

pub async fn receive_message<'a, R>(
    reader: &mut R,
    buffer: &'a mut Vec<u8>,
    allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
) -> Result<DoIpMessage<'static>, DoIpCnxError>
where
    R: AsyncReadExt + Unpin,
{
    read_one_doip_message(reader, buffer, allocate_uds).await
}

pub fn decode_message(buffer: &mut [u8]) -> Result<DoIpMessage, DoIpCnxError> {
    let (payload_type, plen) = decode_header(buffer)?;
    decode_doip_payload(
        &mut buffer[DOIP_HEADER_LENGTH..DOIP_HEADER_LENGTH + plen],
        payload_type,
    )
}

pub async fn send_alive_check_request<W>(
    writer: &mut W,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = AliveCheckRequest {};
    send_payload(writer, &msg, buffer).await
}

pub async fn send_alive_check_response<W>(
    writer: &mut W,
    la: LogicalAddress,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = AliveCheckResponse { source_address: la };
    send_payload(writer, &msg, buffer).await
}

pub async fn send_routing_activation_request<W>(
    writer: &mut W,
    la: LogicalAddress,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = RoutingActivationRequest {
        source_address: la,
        activation_type: ActivationType::Default,
        reserved: [0; 4],
        reserved_oem: Some([0; 4]),
    };
    send_payload(writer, &msg, buffer).await
}

pub async fn send_routing_activation_response<W>(
    writer: &mut W,
    ta: LogicalAddress,
    la: LogicalAddress,
    code: RoutingActivationResponseCode,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = RoutingActivationResponse {
        logical_address_tester: ta,
        logical_address_of_doip_entity: la,
        routing_activation_response_code: code,
        reserved_oem: [0; 4],
        oem_specific: Some([0; 4]),
    };
    send_payload(writer, &msg, buffer).await
}

pub async fn send_diagnostic_request<'a, W>(
    writer: &mut W,
    sa: LogicalAddress,
    ta: LogicalAddress,
    uds: UdsBuffer<'a>,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = DiagnosticMessage {
        source_address: sa,
        target_address: ta,
        user_data: uds,
    };
    trace!(target: "doip", "Writing out diagnostic message");
    let header = DoIpHeader::new(DiagnosticMessage::payload_type(), msg.length() as u32);
    buffer.clear();
    header.write(buffer).map_err(|e| match e {
        DoIpError::Io(io) => io,
        _ => panic!(),
    })?;
    writer.write_all(buffer).await?;
    msg.async_write(writer).await.map_err(|e| match e {
        DoIpError::Io(io) => io,
        _ => panic!(),
    })
}

pub async fn send_diagnostic_acknowledge<W>(
    writer: &mut W,
    sa: LogicalAddress,
    ta: LogicalAddress,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = DiagnosticMessagePositiveAck {
        source_address: sa,
        target_address: ta,
        ack_code: DiagnosticMessagePositiveAckCode::RoutingConfirmationAck,
        previous_diagnostic_message_data: UdsBuffer::Owned(vec![]),
    };
    send_payload(writer, &msg, buffer).await
}

pub async fn send_diagnostic_deny<W>(
    writer: &mut W,
    sa: LogicalAddress,
    ta: LogicalAddress,
    code: DiagnosticMessageNegativeAckCode,
    buffer: &mut Vec<u8>,
) -> tokio::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let msg = DiagnosticMessageNegativeAck {
        source_address: sa,
        target_address: ta,
        ack_code: code,
        previous_diagnostic_message_data: UdsBuffer::Owned(vec![]),
    };
    send_payload(writer, &msg, buffer).await
}

async fn read_one_doip_message<'a, R>(
    reader: &mut R,
    buffer: &'a mut Vec<u8>,
    allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
) -> Result<DoIpMessage<'static>, DoIpCnxError>
where
    R: AsyncReadExt + Unpin,
{
    buffer.resize(DOIP_HEADER_LENGTH, 0);
    reader
        .read_exact(&mut buffer[0..DOIP_HEADER_LENGTH])
        .await?;
    let (payload_type, plen) = decode_header(&buffer[0..DOIP_HEADER_LENGTH])?;
    trace!(target: "doip", "Reading in {} input bytes", plen);
    buffer.resize(plen, 0);
    read_doip_payload(reader, buffer, payload_type, allocate_uds).await
}

async fn send_payload<W, P>(writer: &mut W, p: &P, buffer: &mut Vec<u8>) -> tokio::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
    P: Payload,
{
    buffer.clear();
    write_message(p, buffer).unwrap();
    trace!(target: "doip", "Writing out {:02x?}", buffer);
    writer.write_all(buffer).await?;
    Ok(())
}

pub fn encode_message<P>(p: &P, buffer: &mut Vec<u8>) -> Result<(), DoIpError>
where
    P: Payload,
{
    buffer.clear();
    write_message(p, buffer)
}

impl Default for crate::Timings {
    fn default() -> Self {
        Self {
            tcp_connect: Duration::from_secs(1),
            routing_activation_rsp: Duration::from_secs(1),
        }
    }
}

impl From<tokio::io::Error> for DoIpCnxError {
    fn from(value: tokio::io::Error) -> Self {
        DoIpCnxError::ConnectionError(value)
    }
}

impl From<DoIpError> for DoIpCnxError {
    fn from(value: DoIpError) -> Self {
        match value {
            DoIpError::Io(io) => DoIpCnxError::ConnectionError(io),
            _ => DoIpCnxError::InvalidMessageReceived,
        }
    }
}
