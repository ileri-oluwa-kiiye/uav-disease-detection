// --- Packet encoding helpers ---

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

// --- Packet builders ---

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
    let mut connect_flags: u8 = 0x02; // clean session

    if let Some(u) = username {
        connect_flags |= 0x80;
        payload_len += 2 + u.len();
    }
    if let Some(p) = password {
        connect_flags |= 0x40;
        payload_len += 2 + p.len();
    }

    // Variable header is always 10 bytes for MQTT 3.1.1
    let remaining = 10 + payload_len;

    let mut pos = 0;

    // Fixed header
    buf[pos] = 0x10; // CONNECT packet type
    pos += 1;
    pos += encode_remaining_length(&mut buf[pos..], remaining);

    // Variable header
    // Protocol name "MQTT"
    buf[pos..pos + 7].copy_from_slice(&[0x00, 0x04, b'M', b'Q', b'T', b'T', 0x04]);
    pos += 7;
    buf[pos] = connect_flags;
    pos += 1;
    buf[pos] = (keep_alive_secs >> 8) as u8;
    buf[pos + 1] = (keep_alive_secs & 0xFF) as u8;
    pos += 2;

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

pub fn build_publish<'a>(buf: &'a mut [u8], topic: &str, payload: &[u8]) -> &'a [u8] {
    // QoS 0, no packet ID needed
    let remaining = 2 + topic.len() + payload.len();

    let mut pos = 0;
    buf[pos] = 0x30; // PUBLISH, QoS 0, no retain
    pos += 1;
    pos += encode_remaining_length(&mut buf[pos..], remaining);
    pos += write_utf8_string(buf, pos, topic);
    buf[pos..pos + payload.len()].copy_from_slice(payload);
    pos += payload.len();

    &buf[..pos]
}

pub fn build_subscribe<'a>(buf: &'a mut [u8], packet_id: u16, topic: &str, qos: u8) -> &'a [u8] {
    let remaining = 2 + 2 + topic.len() + 1; // packet_id + topic string + qos byte

    let mut pos = 0;
    buf[pos] = 0x82; // SUBSCRIBE, QoS 1 (required by spec)
    pos += 1;
    pos += encode_remaining_length(&mut buf[pos..], remaining);

    // Packet ID
    buf[pos] = (packet_id >> 8) as u8;
    buf[pos + 1] = (packet_id & 0xFF) as u8;
    pos += 2;

    pos += write_utf8_string(buf, pos, topic);
    buf[pos] = qos;
    pos += 1;

    &buf[..pos]
}

pub fn build_pingreq() -> [u8; 2] {
    [0xC0, 0x00]
}
