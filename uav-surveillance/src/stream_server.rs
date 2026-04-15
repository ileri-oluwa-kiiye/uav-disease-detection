use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::io::Write;

use crate::camera;

const STREAM_CONTENT_TYPE: &str = "multipart/x-mixed-replace; boundary=frame";
const HTML: &[u8] = include_bytes!("stream.html");

pub fn start() -> anyhow::Result<EspHttpServer<'static>> {
    let config = Configuration {
        stack_size: 16 * 1024,
        ..Default::default()
    };

    let mut server = EspHttpServer::new(&config)?;

    // Endpoint: http://{ip}/
    server.fn_handler("/", esp_idf_svc::http::Method::Get, |req| {
        let mut resp = req.into_ok_response()?;
        resp.write_all(HTML).map_err(|e| anyhow::anyhow!(e))
    })?;

    // Endpoint: http://{ip}/stream
    server.fn_handler("/stream", esp_idf_svc::http::Method::Get, |req| {
        let mut resp = req.into_response(
            200,
            Some("OK"),
            &[
                ("Content-Type", STREAM_CONTENT_TYPE),
                ("Cache-Control", "no-cache"),
                ("Connection", "keep-alive"),
            ],
        )?;

        loop {
            let Some(frame) = camera::capture() else {
                log::error!("Camera capture failed");
                continue;
            };

            let data = frame.data();

            let header = format!(
                "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                data.len()
            );

            let write_result = resp
                .write_all(header.as_bytes())
                .and_then(|_| resp.write_all(data))
                .and_then(|_| resp.write_all(b"\r\n"));

            if write_result.is_err() {
                // Client disconnected, exit cleanly
                break;
            }
        }

        anyhow::Ok(())
    })?;

    log::info!("HTTP stream server running");

    Ok(server)
}
