use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::io::Write;
use std::time::Duration;

use crate::camera;

const BOUNDARY: &str = "frame";
const STREAM_CONTENT_TYPE: &str = "multipart/x-mixed-replace; boundary=frame";

pub fn start_stream_server() -> anyhow::Result<EspHttpServer<'static>> {
    let config = Configuration {
        stack_size: 16384,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&config)?;

    // Endpoint: http://esp32-ip/
    server.fn_handler("/", esp_idf_svc::http::Method::Get, |req| {
        let html = b"<html><body><h1>Drone Camera</h1><img src=\"/stream\" /></body></html>";
        let mut resp = req.into_ok_response()?;
        resp.write_all(html)?;
        Ok::<(), anyhow::Error>(())
    })?;

    // Endpoint: http://esp32-ip/stream
    server.fn_handler("/stream", esp_idf_svc::http::Method::Get, |req| {
        let mut resp = req.into_response(
            200,
            None,
            &[
                ("Content-Type", STREAM_CONTENT_TYPE),
                ("Cache-Control", "no-cache"),
                ("Connection", "keep-alive"),
            ],
        )?;

        loop {
            match camera::capture() {
                Ok(frame) => {
                    let header = format!(
                        "--{}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        BOUNDARY,
                        frame.len()
                    );

                    if resp.write_all(header.as_bytes()).is_err() {
                        break; // client disconnected
                    }
                    if resp.write_all(frame.data()).is_err() {
                        break;
                    }
                    if resp.write_all(b"\r\n").is_err() {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Capture failed: {:?}", e);
                }
            }

            std::thread::sleep(Duration::from_millis(100)); // ~10 FPS
        }

        Ok::<(), anyhow::Error>(())
    })?;

    // Endpoint: http://esp32-ip/capture (single frame)
    server.fn_handler("/capture", esp_idf_svc::http::Method::Get, |req| {
        match camera::capture() {
            Ok(frame) => {
                let mut resp = req.into_response(
                    200,
                    None,
                    &[
                        ("Content-Type", "image/jpeg"),
                        ("Content-Length", &frame.len().to_string()),
                    ],
                )?;
                resp.write_all(frame.data())?;
            }
            Err(_) => {
                let mut resp = req.into_response(500, None, &[])?;
                resp.write_all(b"Capture failed")?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })?;

    log::info!("Stream server started on port 80");
    Ok(server)
}
