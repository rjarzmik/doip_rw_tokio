use doip_rw::message::*;

#[derive(Debug)]
/// DoIpMessage on TCP stream.
pub enum DoIpTcpMessage<'a> {
    /// DoIP TCP message [`AliveCheckRequest`].
    AliveCheckRequest(AliveCheckRequest),
    /// DoIP TCP message [`AliveCheckResponse`].
    AliveCheckResponse(AliveCheckResponse),
    /// DoIP TCP message [`RoutingActivationRequest`].
    RoutingActivationRequest(RoutingActivationRequest),
    /// DoIP TCP message [`RoutingActivationResponse`].
    RoutingActivationResponse(RoutingActivationResponse),
    /// DoIP TCP message [`DiagnosticMessage`].
    DiagnosticMessage(DiagnosticMessage<'a>),
    /// DoIP TCP message [`DiagnosticMessagePositiveAck`].
    DiagnosticMessagePositiveAck(DiagnosticMessagePositiveAck<'a>),
    /// DoIP TCP message [`DiagnosticMessageNegativeAck`].
    DiagnosticMessageNegativeAck(DiagnosticMessageNegativeAck<'a>),
}

#[derive(Debug)]
/// DoIpMessage on UDP stream.
pub enum DoIpUdpMessage {
    /// DoIP UDP message [`EntityStatusRequest`].
    EntityStatusRequest(EntityStatusRequest),
    /// DoIP UDP message [`EntityStatusResponse`].
    EntityStatusResponse(EntityStatusResponse),
    /// DoIP UDP message [`PowerModeRequest`].
    PowerModeRequest(PowerModeRequest),
    /// DoIP UDP message [`PowerModeResponse`].
    PowerModeResponse(PowerModeResponse),
    /// DoIP UDP message [`VehicleIdentificationRequest`].
    VehicleIdentificationRequest(VehicleIdentificationRequest),
    /// DoIP UDP message [`VehicleIdentificationRequestWithEid`].
    VehicleIdentificationRequestWithEid(VehicleIdentificationRequestWithEid),
    /// DoIP UDP message [`VehicleIdentificationRequestWithVin`].
    VehicleIdentificationRequestWithVin(VehicleIdentificationRequestWithVin),
    /// DoIP UDP message [`VehicleIdentificationResponse`].
    VehicleIdentificationResponse(VehicleIdentificationResponse),
}

pub enum DoIpMessage<'a> {
    Tcp(DoIpTcpMessage<'a>),
    Udp(DoIpUdpMessage),
}
