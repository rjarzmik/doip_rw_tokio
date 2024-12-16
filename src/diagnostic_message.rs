//
// This module adds async calls to read and write a Diagnostic Message

use byteorder::{BigEndian, ByteOrder};
use doip_rw::{
    message::{
        DiagnosticMessage, DiagnosticMessageNegativeAck, DiagnosticMessageNegativeAckCode,
        DiagnosticMessagePositiveAck, DiagnosticMessagePositiveAckCode, UdsBuffer,
    },
    DoIpError, LogicalAddress, PayloadType,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) trait AsyncUdsPayload<T> {
    async fn async_read<R>(
        reader: &mut R,
        payload_length: usize,
        allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
    ) -> Result<T, DoIpError>
    where
        R: AsyncReadExt + Unpin;
    async fn async_write<W>(&self, writer: &mut W) -> Result<(), DoIpError>
    where
        W: AsyncWriteExt + Unpin;
}

fn ensure_payload_length(payload_length: usize, min: usize) -> Result<(), DoIpError> {
    use DoIpError::*;
    if payload_length < min {
        return Err(PayloadLengthTooShort {
            value: payload_length as u32,
            expected: min as u32,
        });
    }
    Ok(())
}

async fn read_addrs<R: AsyncReadExt + Unpin>(
    reader: &mut R,
) -> Result<(LogicalAddress, LogicalAddress), DoIpError> {
    let mut buffer = [0u8; 4];
    reader.read_exact(&mut buffer).await?;
    let source_address = BigEndian::read_u16(&buffer[0..2]);
    let target_address = BigEndian::read_u16(&buffer[2..4]);
    Ok((source_address, target_address))
}

async fn write_addrs<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    source_address: &LogicalAddress,
    target_address: &LogicalAddress,
) -> Result<(), DoIpError> {
    let mut buffer = [0u8; 4];
    BigEndian::write_u16(&mut buffer[0..2], *source_address);
    BigEndian::write_u16(&mut buffer[2..4], *target_address);
    writer.write_all(&buffer).await?;
    Ok(())
}

async fn read_uds_payload<R>(
    reader: &mut R,
    payload_length: usize,
    allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
) -> Result<UdsBuffer<'static>, DoIpError>
where
    R: AsyncReadExt + Unpin,
{
    let mut buffer = allocate_uds(PayloadType::DiagnosticMessage, payload_length);
    if buffer.len() != payload_length {
        return Err(DoIpError::BufferTooSmall);
    }
    let _ = reader.read_exact(&mut buffer).await?;
    Ok(UdsBuffer::Owned(buffer))
}

impl<'a> AsyncUdsPayload<DiagnosticMessage<'a>> for DiagnosticMessage<'a> {
    async fn async_read<'b, R>(
        reader: &'b mut R,
        payload_length: usize,
        allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
    ) -> Result<DiagnosticMessage<'a>, DoIpError>
    where
        R: AsyncReadExt + Unpin,
    {
        ensure_payload_length(payload_length, 4)?;
        let (source_address, target_address) = read_addrs(reader).await?;
        Ok(DiagnosticMessage {
            source_address,
            target_address,
            user_data: read_uds_payload(reader, payload_length - 4, allocate_uds).await?,
        })
    }

    async fn async_write<W>(&self, writer: &mut W) -> Result<(), DoIpError>
    where
        W: AsyncWriteExt + Unpin,
    {
        write_addrs(writer, &self.source_address, &self.target_address).await?;
        writer.write_all(self.user_data.get_ref()).await?;
        Ok(())
    }
}

impl<'a> AsyncUdsPayload<DiagnosticMessagePositiveAck<'a>> for DiagnosticMessagePositiveAck<'a> {
    async fn async_read<'b, R>(
        reader: &'b mut R,
        payload_length: usize,
        allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
    ) -> Result<DiagnosticMessagePositiveAck<'a>, DoIpError>
    where
        R: AsyncReadExt + Unpin,
    {
        ensure_payload_length(payload_length, 5)?;
        let (source_address, target_address) = read_addrs(reader).await?;
        let ack_code_raw = reader.read_u8().await?;
        let ack_code = DiagnosticMessagePositiveAckCode::from(ack_code_raw);
        Ok(DiagnosticMessagePositiveAck {
            source_address,
            target_address,
            ack_code,
            previous_diagnostic_message_data: read_uds_payload(
                reader,
                payload_length - 5,
                allocate_uds,
            )
            .await?,
        })
    }

    async fn async_write<W>(&self, writer: &mut W) -> Result<(), DoIpError>
    where
        W: AsyncWriteExt + Unpin,
    {
        write_addrs(writer, &self.source_address, &self.target_address).await?;
        writer.write_u8(self.ack_code.into()).await?;
        writer
            .write_all(self.previous_diagnostic_message_data.get_ref())
            .await?;
        Ok(())
    }
}

impl<'a> AsyncUdsPayload<DiagnosticMessageNegativeAck<'a>> for DiagnosticMessageNegativeAck<'a> {
    async fn async_read<'b, R>(
        reader: &'b mut R,
        payload_length: usize,
        allocate_uds: impl FnOnce(PayloadType, usize) -> Vec<u8>,
    ) -> Result<DiagnosticMessageNegativeAck<'a>, DoIpError>
    where
        R: AsyncReadExt + Unpin,
    {
        ensure_payload_length(payload_length, 5)?;
        let (source_address, target_address) = read_addrs(reader).await?;
        let ack_code_raw = reader.read_u8().await?;
        let ack_code = DiagnosticMessageNegativeAckCode::from(ack_code_raw);
        Ok(DiagnosticMessageNegativeAck {
            source_address,
            target_address,
            ack_code,
            previous_diagnostic_message_data: read_uds_payload(
                reader,
                payload_length - 5,
                allocate_uds,
            )
            .await?,
        })
    }

    async fn async_write<W>(&self, writer: &mut W) -> Result<(), DoIpError>
    where
        W: AsyncWriteExt + Unpin,
    {
        write_addrs(writer, &self.source_address, &self.target_address).await?;
        writer.write_u8(self.ack_code.into()).await?;
        writer
            .write_all(self.previous_diagnostic_message_data.get_ref())
            .await?;
        Ok(())
    }
}
