use embassy_net::tcp::TcpSocket;
use embedded_io_async::{Read, Write};

use crate::mqtt_utils::*;

#[derive(Debug)]
pub enum MqttError {
    Network,
    Protocol,
    ConnectRejected(u8),
}

pub struct MiniMqtt<'a> {
    pub socket: TcpSocket<'a>,
    pub buf: [u8; 1024],
}

impl<'a> MiniMqtt<'a> {
    pub fn new(socket: TcpSocket<'a>) -> Self {
        Self {
            socket,
            buf: [0u8; 1024],
        }
    }

    pub async fn connect(
        &mut self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        keep_alive: u16,
    ) -> Result<(), MqttError> {
        let packet = build_connect(&mut self.buf, client_id, username, password, keep_alive);
        self.socket
            .write_all(packet)
            .await
            .map_err(|_| MqttError::Network)?;

        // Read CONNACK (4 bytes: fixed header + remaining len + flags + return code)
        let mut connack = [0u8; 4];
        self.socket
            .read_exact(&mut connack)
            .await
            .map_err(|_| MqttError::Network)?;

        if connack[0] != 0x20 {
            return Err(MqttError::Protocol);
        }
        if connack[3] != 0x00 {
            return Err(MqttError::ConnectRejected(connack[3]));
        }

        Ok(())
    }

    pub async fn publish(&mut self, topic: &str, payload: &[u8]) -> Result<(), MqttError> {
        let packet = build_publish(&mut self.buf, topic, payload);
        self.socket
            .write_all(packet)
            .await
            .map_err(|_| MqttError::Network)?;
        Ok(())
    }

    pub async fn subscribe(&mut self, topic: &str, qos: u8) -> Result<(), MqttError> {
        let packet = build_subscribe(&mut self.buf, 1, topic, qos);
        self.socket
            .write_all(packet)
            .await
            .map_err(|_| MqttError::Network)?;

        // Read SUBACK — at least 5 bytes
        let mut suback = [0u8; 5];
        self.socket
            .read_exact(&mut suback)
            .await
            .map_err(|_| MqttError::Network)?;

        if suback[0] != 0x90 {
            return Err(MqttError::Protocol);
        }

        Ok(())
    }

    pub async fn ping(&mut self) -> Result<(), MqttError> {
        self.socket
            .write_all(&build_pingreq())
            .await
            .map_err(|_| MqttError::Network)?;
        Ok(())
    }

    /// Read one incoming message. Returns (topic, payload) for PUBLISH packets.
    /// Blocks until data arrives.
    pub async fn read_message<'b>(
        &mut self,
        topic_buf: &'b mut [u8],
        payload_buf: &'b mut [u8],
    ) -> Result<Option<(&'b str, &'b [u8])>, MqttError> {
        // Read fixed header byte
        let mut header = [0u8; 1];
        self.socket
            .read_exact(&mut header)
            .await
            .map_err(|_| MqttError::Network)?;

        let packet_type = header[0] >> 4;

        // Decode remaining length
        let mut remaining: usize = 0;
        let mut shift = 0;
        loop {
            let mut byte = [0u8; 1];
            self.socket
                .read_exact(&mut byte)
                .await
                .map_err(|_| MqttError::Network)?;
            remaining |= ((byte[0] & 0x7F) as usize) << shift;
            shift += 7;
            if byte[0] & 0x80 == 0 {
                break;
            }
        }

        if packet_type == 3 {
            // PUBLISH
            // Read topic length
            let mut len_bytes = [0u8; 2];
            self.socket
                .read_exact(&mut len_bytes)
                .await
                .map_err(|_| MqttError::Network)?;
            let topic_len = ((len_bytes[0] as usize) << 8) | len_bytes[1] as usize;

            self.socket
                .read_exact(&mut topic_buf[..topic_len])
                .await
                .map_err(|_| MqttError::Network)?;

            let payload_len = remaining - 2 - topic_len;
            self.socket
                .read_exact(&mut payload_buf[..payload_len])
                .await
                .map_err(|_| MqttError::Network)?;

            let topic =
                core::str::from_utf8(&topic_buf[..topic_len]).map_err(|_| MqttError::Protocol)?;

            Ok(Some((topic, &payload_buf[..payload_len])))
        } else if packet_type == 13 {
            // PINGRESP — just ignore
            Ok(None)
        } else {
            // Drain unknown packet
            for _ in 0..remaining {
                let mut discard = [0u8; 1];
                self.socket
                    .read_exact(&mut discard)
                    .await
                    .map_err(|_| MqttError::Network)?;
            }
            Ok(None)
        }
    }
}
