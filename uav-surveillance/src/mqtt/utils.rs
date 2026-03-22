use embedded_io_async::Read;

use super::{MqttError, PROTOCOL_LEVEL_3_1_1, PROTOCOL_NAME, VARIABLE_HEADER_LEN, flags, packet_type};

// Encoding helpers

fn encode_remaining_length(buf: &mut [u8], len: usize) -> usize {
    let mut x = len;
    let mut i = 0;

    loop {
        let mut byte = (x % 128) as u8;
        x /= 128;
        if x > 0 {
            byte |= 0x80;
        }
        buf[i] = byte;
        i += 1;
        if x == 0 {
            break;
        }
    }

    i
}

fn write_utf8_string(buf: &mut [u8], offset: usize, s: &str) -> usize {
    let len = s.len();
    buf[offset] = (len >> 8) as u8;
    buf[offset + 1] = (len & 0xFF) as u8;
    buf[offset + 2..offset + 2 + len].copy_from_slice(s.as_bytes());
    2 + len
}

#[inline(always)]
fn write_u8(buf: &mut [u8], offset: usize, value: u8) -> usize {
    buf[offset] = value;
    1
}

#[inline(always)]
fn write_u16(buf: &mut [u8], offset: usize, value: u16) -> usize {
    buf[offset] = (value >> 8) as u8;
    buf[offset + 1] = (value & 0xFF) as u8;
    2
}

#[inline(always)]
pub fn read_u16(buf: &[u8]) -> usize {
    ((buf[0] as usize) << 8) | buf[1] as usize
}

pub async fn read_remaining_length<T: Read>(reader: &mut T) -> Result<usize, MqttError> {
    let mut remaining: usize = 0;
    let mut shift = 0;
    let mut byte = [0u8; 1];

    loop {
        reader.read_exact(&mut byte).await.map_err(|_| MqttError::Network)?;
        remaining |= ((byte[0] & 0x7F) as usize) << shift;
        shift += 7;

        if byte[0] & 0x80 == 0 {
            break;
        }
    }

    Ok(remaining)
}

// Packet builders

pub fn build_connect<'a>(
    buf: &'a mut [u8],
    client_id: &str,
    username: Option<&str>,
    password: Option<&str>,
    keep_alive_secs: u16,
) -> &'a [u8] {
    // Variable header: protocol name + level + flags + keepalive
    // Payload: client_id, username, password

    let mut payload_len = 2 + client_id.len();
    let mut connect_flags: u8 = flags::CONNECT_CLEAN_SESSION;

    if let Some(u) = username {
        connect_flags |= flags::CONNECT_USERNAME;
        payload_len += 2 + u.len();
    }

    if let Some(p) = password {
        connect_flags |= flags::CONNECT_PASSWORD;
        payload_len += 2 + p.len();
    }

    let remaining = VARIABLE_HEADER_LEN + payload_len;
    let mut pos = 0;

    // Fixed header
    pos += write_u8(buf, pos, packet_type::CONNECT);
    pos += encode_remaining_length(&mut buf[pos..], remaining);

    // Encode protocol name
    const NAME_LEN: usize = PROTOCOL_NAME.len();
    buf[pos..pos + NAME_LEN].copy_from_slice(PROTOCOL_NAME);
    pos += NAME_LEN;

    // Encode protocol level, connect flags, keep alive
    pos += write_u8(buf, pos, PROTOCOL_LEVEL_3_1_1);
    pos += write_u8(buf, pos, connect_flags);
    pos += write_u16(buf, pos, keep_alive_secs);

    // Payload
    pos += write_utf8_string(buf, pos, client_id);

    if let Some(u) = username {
        pos += write_utf8_string(buf, pos, u);
    }

    if let Some(p) = password {
        pos += write_utf8_string(buf, pos, p);
    }

    &buf[..pos]
}

pub fn build_publish<'a>(buf: &'a mut [u8], topic: &str, payload: &[u8], qos: u8, retain: bool) -> &'a [u8] {
    let mut flags = 0;

    match qos {
        1 => flags |= flags::PUBLISH_QOS1,
        2 => flags |= flags::PUBLISH_QOS2,
        _ => {}
    }

    if retain {
        flags |= flags::PUBLISH_RETAIN;
    }

    let remaining = 2 + topic.len() + payload.len();
    let mut pos = 0;

    pos += write_u8(buf, pos, packet_type::PUBLISH | flags);
    pos += encode_remaining_length(&mut buf[pos..], remaining);
    pos += write_utf8_string(buf, pos, topic);
    buf[pos..pos + payload.len()].copy_from_slice(payload);
    pos += payload.len();

    &buf[..pos]
}

pub fn build_subscribe<'a>(buf: &'a mut [u8], packet_id: u16, topic: &str, qos: u8) -> &'a [u8] {
    let remaining = 2 + 2 + topic.len() + 1; // packet_id + topic string + qos byte
    let mut pos = 0;

    pos += write_u8(buf, pos, packet_type::SUBSCRIBE | flags::SUBSCRIBE_FLAGS); // Fixed header
    pos += encode_remaining_length(&mut buf[pos..], remaining);
    pos += write_u16(buf, pos, packet_id);
    pos += write_utf8_string(buf, pos, topic);
    pos += write_u8(buf, pos, qos);

    &buf[..pos]
}

pub fn build_pingreq() -> [u8; 2] {
    [packet_type::PINGREQ, 0x00]
}

pub fn build_disconnect() -> [u8; 2] {
    [packet_type::DISCONNECT, 0x00]
}
