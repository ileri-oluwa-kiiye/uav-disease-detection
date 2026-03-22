use embassy_net::tcp::ConnectError;
use embedded_io::ReadExactError;

pub mod client;
pub mod tasks;
mod utils;

mod packet_type {
    pub const CONNECT: u8 = 1 << 4;
    pub const CONNACK: u8 = 2 << 4;
    pub const PUBLISH: u8 = 3 << 4;
    pub const PUBACK: u8 = 4 << 4;
    pub const PUBREC: u8 = 5 << 4;
    pub const PUBREL: u8 = 6 << 4;
    pub const PUBCOMP: u8 = 7 << 4;
    pub const SUBSCRIBE: u8 = 8 << 4;
    pub const SUBACK: u8 = 9 << 4;
    pub const UNSUBSCRIBE: u8 = 10 << 4;
    pub const UNSUBACK: u8 = 11 << 4;
    pub const PINGREQ: u8 = 12 << 4;
    pub const PINGRESP: u8 = 13 << 4;
    pub const DISCONNECT: u8 = 14 << 4;
}

mod flags {
    // Fixed header byte = (packet_type << 4) | flags
    // Most packet types have flags = 0, except these:
    pub const SUBSCRIBE_FLAGS: u8 = 0x02; // spec mandates bit 1 set
    pub const UNSUBSCRIBE_FLAGS: u8 = 0x02; // same
    pub const PUBREL_FLAGS: u8 = 0x02; // same

    // PUBLISH flags (bottom 4 bits)
    pub const PUBLISH_DUP: u8 = 0x08; // bit 3
    pub const PUBLISH_QOS1: u8 = 0x02; // bit 1
    pub const PUBLISH_QOS2: u8 = 0x04; // bit 2
    pub const PUBLISH_RETAIN: u8 = 0x01; // bit 0

    // Connect flags byte
    pub const CONNECT_CLEAN_SESSION: u8 = 0x02;
    pub const CONNECT_WILL_FLAG: u8 = 0x04;
    pub const CONNECT_WILL_QOS1: u8 = 0x08;
    pub const CONNECT_WILL_QOS2: u8 = 0x10;
    pub const CONNECT_WILL_RETAIN: u8 = 0x20;
    pub const CONNECT_PASSWORD: u8 = 0x40;
    pub const CONNECT_USERNAME: u8 = 0x80;

    // CONNACK return codes
    pub const CONNACK_ACCEPTED: u8 = 0x00;
    pub const CONNACK_BAD_PROTOCOL: u8 = 0x01;
    pub const CONNACK_ID_REJECTED: u8 = 0x02;
    pub const CONNACK_SERVER_UNAVAILABLE: u8 = 0x03;
    pub const CONNACK_BAD_CREDENTIALS: u8 = 0x04;
    pub const CONNACK_NOT_AUTHORIZED: u8 = 0x05;
}

const PROTOCOL_NAME: &[u8] = &[0x00, 0x04, b'M', b'Q', b'T', b'T'];
const PROTOCOL_LEVEL_3_1_1: u8 = 0x04;
const VARIABLE_HEADER_LEN: usize = 10;

#[derive(Debug)]
pub enum MqttError {
    Network,
    Protocol,
    ConnectRejected(u8),
}

impl<E> From<ReadExactError<E>> for MqttError {
    fn from(_: ReadExactError<E>) -> Self {
        MqttError::Network
    }
}

impl From<embassy_net::tcp::Error> for MqttError {
    fn from(_: embassy_net::tcp::Error) -> Self {
        MqttError::Network
    }
}

impl From<ConnectError> for MqttError {
    fn from(_: ConnectError) -> Self {
        MqttError::Network
    }
}
