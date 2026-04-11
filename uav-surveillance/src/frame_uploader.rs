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
        let len = (data.len() as u32).to_be_bytes();
        self.stream.write_all(&len)?;
        self.stream.write_all(data)?;
        Ok(())
    }
}

fn try_connect(addr: &str) -> FrameUploader {
    loop {
        match FrameUploader::connect(addr) {
            Ok(u) => break u,
            Err(e) => {
                log::error!("Connect failed: {e:?}");
                thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

fn try_stream_camera(uploader: &mut FrameUploader) {
    loop {
        // Camera capture failure is not catastrophic.
        // Log an error and continue.
        let Some(frame) = camera::capture() else {
            log::error!("Camera capture failed");
            continue;
        };

        // Frame upload failure likely indicates a network issue.
        // Log an error and break the loop.
        // Caller can retry the connection.
        if let Err(e) = uploader.upload_frame(frame.data()) {
            log::error!("Upload failed: {e}");
            break;
        }

        // Stream at approximately 10 FPS
        thread::sleep(Duration::from_millis(100));
    }
}

pub fn camera_loop() {
    let mut uploader = try_connect(SERVER_ADDR);
    log::info!("Connected to frame server");

    loop {
        try_stream_camera(&mut uploader);

        // When the function above returns, the stream has ended.
        // This is likely due to a dropped connection.
        log::warn!("Connection lost, reconnecting in 5s...");
        thread::sleep(Duration::from_secs(5));
    }
}
