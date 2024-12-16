use std::net::SocketAddr;

use crate::connection as cnx;
use crate::DoIpCnxError;
use crate::{message::DoIpMessage, DoIpUdpMessage};
use doip_rw::message::FurtherActionRequired;
use doip_rw::message::PowerModeResponse;
use doip_rw::message::VehicleIdentificationRequest;
use doip_rw::message::VehicleIdentificationRequestWithEid;
use doip_rw::message::VehicleIdentificationRequestWithVin;
use doip_rw::message::VehicleIdentificationResponse;
use doip_rw::message::VinGidSyncStatus;
use doip_rw::message::{EntityStatusRequest, EntityStatusResponse, PowerModeRequest};
use doip_rw::LogicalAddress;
use doip_rw::Payload;
use log::trace;
use tokio::net::UdpSocket;

/// DoIpUdpConnection
///
/// The datagram DoIP stream, supporting mainly DoIP very simple queries, and
/// mostly
/// [`VehicleIdentificationRequest`](doip_rw::message::VehicleIdentificationRequest)
/// and
/// [`VehicleIdentificationResponse`](doip_rw::message::VehicleIdentificationResponse).
pub struct DoIpUdpConnection {
    /// The internal connection.
    inner: Inner,
}

impl DoIpUdpConnection {
    /// Create a client DoIP over UDP local endpoint.
    ///
    /// This endpoint might be used to send both unicast and multicast/broadcast
    /// messages.
    pub fn new(local_socket: UdpSocket) -> DoIpUdpConnection {
        DoIpUdpConnection {
            inner: Inner {
                local_socket,
                send_buffer: vec![],
                receive_buffer: vec![0; 512],
            },
        }
    }

    /// Main and only receiving function.
    ///
    /// Receive a whole DoIP message from a DoIP datagram socket, decode it and
    /// create the associated [`DoIpUdpMessage`].
    pub async fn receive_message(&mut self) -> Result<DoIpUdpMessage, DoIpCnxError> {
        let (reader, buffer) = (&mut self.inner.local_socket, &mut self.inner.receive_buffer);
        buffer.resize(buffer.capacity(), 0);
        let _ = reader.recv(buffer).await?;
        trace!(target: "doip_udp", "Reading in {:02x?}", buffer);
        match cnx::decode_message(buffer)? {
            DoIpMessage::Tcp(_) => Err(DoIpCnxError::InvalidMessageReceived),
            DoIpMessage::Udp(u) => Ok(u),
        }
    }

    /// Send entity status request.
    ///
    /// Send a [`EntityStatusRequest`](doip_rw::message::EntityStatusRequest)
    /// DoIP message.
    ///
    /// Caller: only a DoIP external tester can issue it.
    ///
    /// Expected answer:
    /// [`EntityStatusResponse`](doip_rw::message::EntityStatusResponse).
    pub async fn send_entity_status_request(
        &mut self,
        remote: SocketAddr,
    ) -> Result<(), DoIpCnxError> {
        let req = EntityStatusRequest {};
        send_message(&mut self.inner, &req, remote).await
    }

    /// Send entity status response.
    ///
    /// Send a [`EntityStatusResponse`](doip_rw::message::EntityStatusResponse) DoIP message.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: None
    pub async fn send_entity_status_response(
        &mut self,
        remote: SocketAddr,
        node_type: u8,
        max_open_sockets: u8,
        cur_open_sockets: u8,
        max_data_size: u32,
    ) -> Result<(), DoIpCnxError> {
        let rsp = EntityStatusResponse {
            node_type,
            max_open_sockets,
            cur_open_sockets,
            max_data_size,
        };
        send_message(&mut self.inner, &rsp, remote).await
    }

    /// Send power mode request.
    ///
    /// Send a [`PowerModeRequest`](doip_rw::message::PowerModeRequest) DoIP message.
    ///
    /// Caller: only a DoIP external tester can issue it.
    ///
    /// Expected answer: [`PowerModeResponse`](doip_rw::message::PowerModeResponse).
    pub async fn send_power_mode_request(
        &mut self,
        remote: SocketAddr,
    ) -> Result<(), DoIpCnxError> {
        let req = PowerModeRequest {};
        send_message(&mut self.inner, &req, remote).await
    }

    /// Send power mode response.
    ///
    /// Send a [`PowerModeResponse`](doip_rw::message::PowerModeResponse) DoIP
    /// message.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: None
    pub async fn send_power_mode_response(
        &mut self,
        remote: SocketAddr,
        power_mode: u8,
    ) -> Result<(), DoIpCnxError> {
        let rsp = PowerModeResponse { power_mode };
        send_message(&mut self.inner, &rsp, remote).await
    }

    /// Send vehicle identification request.
    ///
    /// Send a
    /// [`VehicleIdentificationRequest`](doip_rw::message::VehicleIdentificationRequest)
    /// DoIP message.  This is a discovery query to know which DoIP entities are
    /// available on the network (when broadcasted).
    ///
    /// Caller: only a DoIP external tester can issue it.
    ///
    /// Expected answer: [`VehicleIdentificationResponse`](doip_rw::message::VehicleIdentificationResponse).
    pub async fn send_vehicle_identification_request(
        &mut self,
        remote: SocketAddr,
    ) -> Result<(), DoIpCnxError> {
        let req = VehicleIdentificationRequest {};
        send_message(&mut self.inner, &req, remote).await
    }

    /// Send vehicle identification request with EID.
    ///
    /// Send a
    /// [`VehicleIdentificationRequestWithEid`](doip_rw::message::VehicleIdentificationRequestWithEid)
    /// DoIP message.  This is a discovery query to know which DoIP entities are
    /// available on the network (when broadcasted). The answer is expected to
    /// contain an EID (such as a MAC address for example).
    ///
    /// Caller: only a DoIP external tester can issue it.
    ///
    /// Expected answer: [`VehicleIdentificationResponse`](doip_rw::message::VehicleIdentificationResponse).
    pub async fn send_vehicle_identification_with_eid_request(
        &mut self,
        remote: SocketAddr,
    ) -> Result<(), DoIpCnxError> {
        let req = VehicleIdentificationRequestWithEid {};
        send_message(&mut self.inner, &req, remote).await
    }

    /// Send vehicle identification request with VIN.
    ///
    /// Send a [`VehicleIdentificationRequestWithVin`](doip_rw::message::VehicleIdentificationRequestWithVin) DoIP message.  This is a
    /// discovery query to know which DoIP entities are available on the network
    /// (when broadcasted). The answer is expected to contain a VIN.
    ///
    /// Caller: only a DoIP external tester can issue it.
    ///
    /// Expected answer: [`VehicleIdentificationResponse`](doip_rw::message::VehicleIdentificationResponse).
    pub async fn send_vehicle_identification_with_vin_request(
        &mut self,
        remote: SocketAddr,
    ) -> Result<(), DoIpCnxError> {
        let req = VehicleIdentificationRequestWithVin {};
        send_message(&mut self.inner, &req, remote).await
    }

    #[allow(clippy::too_many_arguments)]
    /// Send a vehicle indentification response, aka. vehicle announcement.
    ///
    /// Send a
    /// [`VehicleIdentificationResponse`](doip_rw::message::VehicleIdentificationResponse)
    /// DoIP message.
    ///
    /// Caller: only a DoIP entity can issue it.
    ///
    /// Expected answer: None
    pub async fn send_vehicle_identification_response(
        &mut self,
        remote: SocketAddr,
        vin: [u8; 17],
        la: LogicalAddress,
        eid: [u8; 6],
        gid: Option<[u8; 6]>,
        further_action: FurtherActionRequired,
        vin_gid_sync_status: VinGidSyncStatus,
    ) -> Result<(), DoIpCnxError> {
        let req = VehicleIdentificationResponse {
            vin,
            logical_address: la,
            eid,
            gid,
            further_action,
            vin_gid_sync_status,
        };
        send_message(&mut self.inner, &req, remote).await
    }
}

struct Inner {
    local_socket: UdpSocket,
    send_buffer: Vec<u8>,
    receive_buffer: Vec<u8>,
}

async fn send_message<P>(
    inner: &mut Inner,
    msg: &P,
    remote_addr: SocketAddr,
) -> Result<(), DoIpCnxError>
where
    P: Payload,
{
    let send_buffer = &mut inner.send_buffer;
    cnx::encode_message(msg, send_buffer).map_err(|_| DoIpCnxError::InvalidMessageToSend)?;
    trace!(target: "doip_udp", "Writing out {:02x?}", send_buffer);
    let _ = inner.local_socket.send_to(send_buffer, remote_addr).await?;
    Ok(())
}
