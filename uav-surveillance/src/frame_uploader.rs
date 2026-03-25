use std::{io::Write, net::TcpStream, thread, time::Duration};

use crate::camera;

const SERVER_ADDR: &str = "";

pub struct FrameUploader {
    stream: TcpStream,
}

impl FrameUploader {
    pub fn connect(addr: &str) -> Result<Self, std::io::Error> {
        Ok(Self {
            stream: TcpStream::connect(addr)?,
        })
    }

    pub fn upload_frame(&mut self, data: &[u8]) -> Result<(), std::io::Error> {
        self.stream.write_all(&((data.len() as u32).to_be_bytes()))?;
        self.stream.write_all(data)?;
        Ok(())
    }
}

pub fn camera_loop() {
    loop {
        let mut uploader = match FrameUploader::connect(SERVER_ADDR) {
            Ok(u) => u,
            Err(e) => {
                log::error!("Connect failed: {e:?}");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        log::info!("Connected to frame server");

        loop {
            match camera::capture() {
                Ok(frame) => {
                    if let Err(e) = uploader.upload_frame(frame.data()) {
                        log::error!("Upload failed: {e:?}");
                        break; // reconnect
                    }
                }
                Err(e) => log::error!("Capture failed: {e:?}"),
            }
            thread::sleep(Duration::from_millis(100)); // ~10 FPS
        }

        log::warn!("Connection lost, reconnecting in 5s...");
        thread::sleep(Duration::from_secs(5));
    }
}
