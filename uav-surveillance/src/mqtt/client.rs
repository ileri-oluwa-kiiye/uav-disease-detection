use embassy_net::tcp::TcpSocket;
use embedded_io_async::{Read, Write};

use crate::mqtt::{flags::CONNACK_ACCEPTED, packet_type};

use super::{MqttError, utils::*};

pub struct MqttV3Client<'a> {
    socket: TcpSocket<'a>,
    buf: [u8; 1024],
    next_packet_id: u16,
}

impl<'a> MqttV3Client<'a> {
    pub fn new(socket: TcpSocket<'a>) -> Self {
        Self {
            socket,
            buf: [0u8; 1024],
            next_packet_id: 1,
        }
    }

    fn alloc_packet_id(&mut self) -> u16 {
        let id = self.next_packet_id;
        self.next_packet_id = self.next_packet_id.wrapping_add(1);
        if self.next_packet_id == 0 {
            self.next_packet_id = 1; // packet ID 0 is invalid per spec
        }
        id
    }

    pub async fn connect(
        &mut self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        keep_alive: u16,
    ) -> Result<(), MqttError> {
        let packet = build_connect(&mut self.buf, client_id, username, password, keep_alive);
        self.socket.write_all(packet).await?;

        let mut connack = [0u8; 4];
        self.socket.read_exact(&mut connack).await?;

        if connack[0] & 0xF0 != packet_type::CONNACK {
            return Err(MqttError::Protocol);
        }

        if connack[3] != CONNACK_ACCEPTED {
            return Err(MqttError::ConnectRejected(connack[3]));
        }

        Ok(())
    }

    pub async fn publish(&mut self, topic: &str, payload: &[u8], qos: u8, retain: bool) -> Result<(), MqttError> {
        let packet = build_publish(&mut self.buf, topic, payload, qos, retain);
        self.socket.write_all(packet).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self, topic: &str, qos: u8) -> Result<(), MqttError> {
        let id = self.alloc_packet_id();
        let packet = build_subscribe(&mut self.buf, id, topic, qos);
        self.socket.write_all(packet).await?;

        let mut suback = [0u8; 5];
        self.socket.read_exact(&mut suback).await?;

        if suback[0] & 0xF0 != packet_type::SUBACK {
            return Err(MqttError::Protocol);
        }

        Ok(())
    }

    pub async fn ping(&mut self) -> Result<(), MqttError> {
        self.socket.write_all(&build_pingreq()).await?;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        self.socket.write_all(&build_disconnect()).await?;
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
        self.socket.read_exact(&mut header).await?;

        let packet_type = header[0] & 0xF0;
        let remaining: usize = read_remaining_length(&mut self.socket).await?;

        match packet_type {
            packet_type::PUBLISH => {
                let mut len_bytes = [0u8; 2];
                self.socket.read_exact(&mut len_bytes).await?;
                let topic_len = read_u16(&len_bytes);

                self.socket.read_exact(&mut topic_buf[..topic_len]).await?;

                let payload_len = remaining - 2 - topic_len;
                self.socket.read_exact(&mut payload_buf[..payload_len]).await?;

                let topic = core::str::from_utf8(&topic_buf[..topic_len]).map_err(|_| MqttError::Protocol)?;

                Ok(Some((topic, &payload_buf[..payload_len])))
            }
            packet_type::PINGRESP => Ok(None),
            _ => {
                // Drain unknown packet
                for _ in 0..remaining {
                    let mut discard = [0u8; 1];
                    self.socket.read_exact(&mut discard).await?;
                }
                Ok(None)
            }
        }
    }
}
