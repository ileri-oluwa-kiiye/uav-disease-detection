//! Lightweight binary protocol for STM32 <-> ESP32 communication
//!
//! Frame: [0xAA] [msg_id] [len] [payload...] [crc8]
//!
//! ESP32 → STM32:
//!   0x01 RC_COMMAND:  f32 throttle, f32 roll, f32 pitch (12 bytes)
//!   0x02 ARM_COMMAND: u8 armed (1 byte)
//!
//! STM32 → ESP32:
//!   0x10 TELEMETRY:   f32 roll, f32 pitch, f32 yaw, f32 alt,
//!                     u16 motor[4], u8 armed, u8 pad (28 bytes)
//!   0x11 HEARTBEAT:   u32 uptime_ms (4 bytes)

const SYNC_BYTE: u8 = 0xAA;

// Message IDs
pub const MSG_RC_COMMAND: u8 = 0x01;
pub const MSG_ARM_COMMAND: u8 = 0x02;
pub const MSG_TELEMETRY: u8 = 0x10;
pub const MSG_HEARTBEAT: u8 = 0x11;

// Max payload size
pub const MAX_PAYLOAD_SIZE: usize = 32;

/// Parsed message from ESP32
#[derive(Debug, Clone, Copy)]
pub enum RxMessage {
    RcCommand {
        throttle: f32,
        roll: f32,
        pitch: f32,
    },
    ArmCommand {
        armed: bool,
    },
}

/// Frame parser state machine
enum ParseState {
    WaitSync,
    GotSync,
    GotId { id: u8 },
    GotLen { id: u8, len: u8 },
    Payload { id: u8, len: u8, received: u8 },
}

pub struct Parser {
    state: ParseState,
    payload: [u8; MAX_PAYLOAD_SIZE],
}

impl Parser {
    pub const fn new() -> Self {
        Self {
            state: ParseState::WaitSync,
            payload: [0u8; MAX_PAYLOAD_SIZE],
        }
    }

    /// Feed one byte. Returns Some(msg) when a complete valid frame is parsed.
    pub fn feed(&mut self, byte: u8) -> Option<RxMessage> {
        match self.state {
            ParseState::WaitSync => {
                if byte == SYNC_BYTE {
                    self.state = ParseState::GotSync;
                }
                None
            }
            ParseState::GotSync => {
                self.state = ParseState::GotId { id: byte };
                None
            }
            ParseState::GotId { id } => {
                self.state = if byte as usize > MAX_PAYLOAD_SIZE {
                    ParseState::WaitSync
                } else {
                    ParseState::GotLen { id, len: byte }
                };

                None
            }
            ParseState::GotLen { id, len } => {
                if len == 0 {
                    // This byte is the CRC
                    let expected = crc8(&[id, 0]);
                    self.state = ParseState::WaitSync;
                    if byte == expected {
                        return self.decode(id, 0);
                    }
                    return None;
                }

                self.payload[0] = byte;
                self.state = ParseState::Payload {
                    id,
                    len,
                    received: 1,
                };
                None
            }
            ParseState::Payload { id, len, received } => {
                if received < len {
                    self.payload[received as usize] = byte;
                    self.state = ParseState::Payload {
                        id,
                        len,
                        received: received + 1,
                    };
                    None
                } else {
                    // This byte is the CRC
                    let mut crc_data = [0u8; MAX_PAYLOAD_SIZE + 1];
                    crc_data[0] = id;
                    crc_data[1] = len;
                    crc_data[2..2 + len as usize].copy_from_slice(&self.payload[..len as usize]);
                    let expected = crc8(&crc_data[..2 + len as usize]);
                    self.state = ParseState::WaitSync;
                    if byte == expected {
                        return self.decode(id, len as usize);
                    } else {
                        None
                    }
                }
            }
        }
    }

    fn decode(&self, id: u8, len: usize) -> Option<RxMessage> {
        match id {
            MSG_RC_COMMAND if len == 12 => {
                let throttle = f32::from_le_bytes(self.payload[0..4].try_into().ok()?);
                let roll = f32::from_le_bytes(self.payload[4..8].try_into().ok()?);
                let pitch = f32::from_le_bytes(self.payload[8..12].try_into().ok()?);
                Some(RxMessage::RcCommand {
                    throttle,
                    roll,
                    pitch,
                })
            }
            MSG_ARM_COMMAND if len == 1 => Some(RxMessage::ArmCommand {
                armed: self.payload[0] != 0,
            }),
            _ => None,
        }
    }

    /// Reset the parser state
    pub fn reset(&mut self) {
        self.state = ParseState::WaitSync;
    }
}

/// Encode a telemetry frame into a buffer. Returns number of bytes written.
pub fn encode_telemetry(
    buf: &mut [u8],
    roll: f32,
    pitch: f32,
    yaw: f32,
    altitude: f32,
    motors: [u16; 4],
    armed: bool,
) -> usize {
    let len: u8 = 28;
    let mut payload = [0u8; 28];

    payload[0..4].copy_from_slice(&roll.to_le_bytes());
    payload[4..8].copy_from_slice(&pitch.to_le_bytes());
    payload[8..12].copy_from_slice(&yaw.to_le_bytes());
    payload[12..16].copy_from_slice(&altitude.to_le_bytes());
    payload[16..18].copy_from_slice(&motors[0].to_le_bytes());
    payload[18..20].copy_from_slice(&motors[1].to_le_bytes());
    payload[20..22].copy_from_slice(&motors[2].to_le_bytes());
    payload[22..24].copy_from_slice(&motors[3].to_le_bytes());
    payload[24] = armed as u8;
    payload[25] = 0; // padding

    // Build frame
    let total = 3 + len as usize + 1; // sync + id + len + payload + crc
    if buf.len() < total {
        return 0;
    }

    buf[0] = SYNC_BYTE;
    buf[1] = MSG_TELEMETRY;
    buf[2] = len;
    buf[3..3 + len as usize].copy_from_slice(&payload[..len as usize]);

    let mut crc_data = [0u8; 30];
    crc_data[0] = MSG_TELEMETRY;
    crc_data[1] = len;
    crc_data[2..2 + len as usize].copy_from_slice(&payload[..len as usize]);
    buf[3 + len as usize] = crc8(&crc_data[..2 + len as usize]);

    total
}

/// Encode a heartbeat frame. Returns number of bytes written.
pub fn encode_heartbeat(buf: &mut [u8], uptime_ms: u32) -> usize {
    let len: u8 = 4;
    let payload = uptime_ms.to_le_bytes();

    let total = 3 + 4 + 1; // 8 bytes
    if buf.len() < total {
        return 0;
    }

    buf[0] = SYNC_BYTE;
    buf[1] = MSG_HEARTBEAT;
    buf[2] = len;
    buf[3..7].copy_from_slice(&payload);

    let mut crc_data = [0u8; 6];
    crc_data[0] = MSG_HEARTBEAT;
    crc_data[1] = len;
    crc_data[2..6].copy_from_slice(&payload);
    buf[7] = crc8(&crc_data);

    total
}

/// CRC-8 (DVB-S2 polynomial 0xD5)
fn crc8(data: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    for &byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if crc & 0x80 != 0 {
                crc = (crc << 1) ^ 0xD5;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
