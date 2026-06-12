//! Shared binary protocol for the STM32 <-> ESP32 link.
//!
//! Frame: [0xAA] [id] [len] [payload (len bytes)] [crc8]
//!   - crc8 covers id + len + payload (NOT the sync byte), DVB-S2 poly 0xD5.
//!   - len is bounded by MAX_PAYLOAD_SIZE; oversized frames are dropped.
//!
//! Direction is a convention, not enforced by the wire:
//!   ESP -> STM : RcCommand (0x01), ArmCommand (0x02)
//!   STM -> ESP : Telemetry (0x10), Heartbeat (0x11)
//!
//! Each side encodes the variants it sends and matches the variants it
//! receives; unknown IDs and wrong-length frames decode to `None` (so adding
//! a message later won't break an old peer).

#![cfg_attr(not(test), no_std)]

pub const SYNC_BYTE: u8 = 0xAA;

pub const MSG_RC_COMMAND: u8 = 0x01;
pub const MSG_ARM_COMMAND: u8 = 0x02;
pub const MSG_TELEMETRY: u8 = 0x10;
pub const MSG_HEARTBEAT: u8 = 0x11;

/// Largest payload the parser will accept. Sizes the parser's scratch buffer.
pub const MAX_PAYLOAD_SIZE: usize = core::mem::size_of::<Message>() + 3;
/// Largest possible full frame: sync + id + len + payload + crc.
pub const MAX_FRAME_SIZE: usize = 3 + MAX_PAYLOAD_SIZE + 1;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct RcCommand {
    pub throttle: f32,
    pub roll: f32,
    pub pitch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Telemetry {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub throttle: f32,
    pub motor_duties: [u16; 4],
    pub armed: bool,
    pub tick: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    RcCommand(RcCommand),
    ArmCommand(bool),
    Telemetry(Telemetry),
    Heartbeat { uptime_ms: u32 },
}

impl Message {
    pub const fn id(&self) -> u8 {
        match self {
            Message::RcCommand { .. } => MSG_RC_COMMAND,
            Message::ArmCommand { .. } => MSG_ARM_COMMAND,
            Message::Telemetry { .. } => MSG_TELEMETRY,
            Message::Heartbeat { .. } => MSG_HEARTBEAT,
        }
    }

    pub const fn payload_len(&self) -> usize {
        match self {
            Message::RcCommand { .. } => core::mem::size_of::<RcCommand>(),
            Message::ArmCommand { .. } => core::mem::size_of::<bool>(),
            Message::Telemetry { .. } => core::mem::size_of::<Telemetry>(),
            Message::Heartbeat { .. } => core::mem::size_of::<u32>(),
        }
    }

    /// Serialize a full frame into `buf`. Returns the number of bytes written,
    /// or `None` if `buf` is too small (needs `3 + payload_len + 1`).
    pub fn encode(&self, buf: &mut [u8]) -> Option<usize> {
        let len = self.payload_len();
        let total = 3 + len + 1;
        if buf.len() < total {
            return None;
        }

        buf[0] = SYNC_BYTE;
        buf[1] = self.id();
        buf[2] = len as u8;

        let p = &mut buf[3..3 + len];
        match *self {
            Message::RcCommand(rc) => {
                p[0..4].copy_from_slice(&rc.throttle.to_le_bytes());
                p[4..8].copy_from_slice(&rc.roll.to_le_bytes());
                p[8..12].copy_from_slice(&rc.pitch.to_le_bytes());
            }
            Message::ArmCommand(armed) => p[0] = armed as u8,
            Message::Telemetry(tele) => {
                p[0..4].copy_from_slice(&tele.roll.to_le_bytes());
                p[4..8].copy_from_slice(&tele.pitch.to_le_bytes());
                p[8..12].copy_from_slice(&tele.yaw.to_le_bytes());
                p[12..16].copy_from_slice(&tele.throttle.to_le_bytes());
                p[16..18].copy_from_slice(&tele.motor_duties[0].to_le_bytes());
                p[18..20].copy_from_slice(&tele.motor_duties[1].to_le_bytes());
                p[20..22].copy_from_slice(&tele.motor_duties[2].to_le_bytes());
                p[22..24].copy_from_slice(&tele.motor_duties[3].to_le_bytes());
                p[24] = tele.armed as u8;
                p[25..29].copy_from_slice(&tele.tick.to_le_bytes());
            }
            Message::Heartbeat { uptime_ms } => {
                p[0..4].copy_from_slice(&uptime_ms.to_le_bytes());
            }
        }

        // CRC the contiguous [id, len, payload] we just wrote — no scratch buffer.
        let crc = crc8(&buf[1..3 + len]);
        buf[3 + len] = crc;
        Some(total)
    }

    /// Decode a CRC-validated payload. Normally you go through `Parser`;
    /// this is exposed for callers that already have a deframed payload.
    pub fn decode(id: u8, payload: &[u8]) -> Option<Message> {
        Some(match (id, payload.len()) {
            (MSG_RC_COMMAND, 12) => Message::RcCommand(RcCommand {
                throttle: f32::from_le_bytes(payload[0..4].try_into().ok()?),
                roll: f32::from_le_bytes(payload[4..8].try_into().ok()?),
                pitch: f32::from_le_bytes(payload[8..12].try_into().ok()?),
            }),
            (MSG_ARM_COMMAND, 1) => Message::ArmCommand(payload[0] != 0),
            (MSG_TELEMETRY, 28) => Message::Telemetry(Telemetry {
                roll: f32::from_le_bytes(payload[0..4].try_into().ok()?),
                pitch: f32::from_le_bytes(payload[4..8].try_into().ok()?),
                yaw: f32::from_le_bytes(payload[8..12].try_into().ok()?),
                throttle: f32::from_le_bytes(payload[12..16].try_into().ok()?),
                motor_duties: [
                    u16::from_le_bytes(payload[16..18].try_into().ok()?),
                    u16::from_le_bytes(payload[18..20].try_into().ok()?),
                    u16::from_le_bytes(payload[20..22].try_into().ok()?),
                    u16::from_le_bytes(payload[22..24].try_into().ok()?),
                ],
                armed: payload[24] != 0,
                tick: u32::from_le_bytes(payload[25..29].try_into().ok()?),
            }),
            (MSG_HEARTBEAT, 4) => Message::Heartbeat {
                uptime_ms: u32::from_le_bytes(payload[0..4].try_into().ok()?),
            },
            _ => return None,
        })
    }
}

/// Streaming, byte-at-a-time frame parser. CRC is accumulated incrementally,
/// so there is no scratch buffer and no oversized-payload panic path.
pub struct Parser {
    state: State,
    payload: [u8; MAX_PAYLOAD_SIZE],
    crc: u8,
}

enum State {
    WaitSync,
    Id,
    Len { id: u8 },
    Payload { id: u8, len: u8, received: u8 },
}

impl Parser {
    pub const fn new() -> Self {
        Self {
            state: State::WaitSync,
            payload: [0u8; MAX_PAYLOAD_SIZE],
            crc: 0,
        }
    }

    /// Feed one byte. Returns `Some(msg)` once a complete, CRC-valid,
    /// recognized frame is parsed.
    pub fn feed(&mut self, byte: u8) -> Option<Message> {
        match self.state {
            State::WaitSync => {
                if byte == SYNC_BYTE {
                    self.state = State::Id;
                }
                None
            }
            State::Id => {
                self.crc = crc8_update(0, byte);
                self.state = State::Len { id: byte };
                None
            }
            State::Len { id } => {
                if byte as usize > MAX_PAYLOAD_SIZE {
                    self.state = State::WaitSync;
                    return None;
                }
                self.crc = crc8_update(self.crc, byte);
                self.state = State::Payload {
                    id,
                    len: byte,
                    received: 0,
                };
                None
            }
            State::Payload { id, len, received } => {
                if received < len {
                    self.payload[received as usize] = byte;
                    self.crc = crc8_update(self.crc, byte);
                    self.state = State::Payload {
                        id,
                        len,
                        received: received + 1,
                    };
                    None
                } else {
                    // This byte is the CRC (covers len == 0 too: received starts at 0).
                    let ok = byte == self.crc;
                    self.state = State::WaitSync;
                    if ok {
                        Message::decode(id, &self.payload[..len as usize])
                    } else {
                        None
                    }
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.state = State::WaitSync;
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// CRC-8, DVB-S2 polynomial 0xD5, init 0x00.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        crc = crc8_update(crc, b);
    }
    crc
}

const fn crc8_update(mut crc: u8, byte: u8) -> u8 {
    crc ^= byte;
    let mut i = 0;
    while i < 8 {
        crc = if crc & 0x80 != 0 {
            (crc << 1) ^ 0xD5
        } else {
            crc << 1
        };
        i += 1;
    }
    crc
}
