use std::{thread, time::Duration};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition,
};

mod camera;
mod comms;
mod frame_uploader;
mod mqtt;
mod stream_server;
mod util;
mod wifi;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    log::info!("Initializing camera...");
    camera::init()?;
    log::info!("Camera ready");

    log::info!("Connecting to WiFi...");
    let (_wifi, ip) = wifi::connect(peripherals.modem, sysloop, nvs)?;
    let _ = stream_server::start()?;
    log::info!("Open http://{ip}/ in your browser");

    let comms = comms::start(
        peripherals.uart1,
        peripherals.pins.gpio1, // ESP TX -> STM RX (PA10)
        peripherals.pins.gpio2, // ESP RX -> STM TX (PA9)
        115_200,
    )?;

    // MQTT thread
    //thread::Builder::new()
    //    .stack_size(8 * 1024)
    //    .name("mqtt".into())
    //    .spawn(mqtt_loop)?;

    // Camera thread
    // thread::Builder::new()
    //    .stack_size(16 * 1024)
    //    .name("camera".into())
    //    .spawn(camera_loop)?;

    // Idle main thread
    loop {
        if let Some(t) = comms.telemetry() {
            let stale = comms.since_last_rx_ms() > 500;
            log::info!(
                "att r={:.1} p={:.1} y={:.1} armed={} motors={:?}{}",
                t.roll,
                t.pitch,
                t.yaw,
                t.armed,
                t.motor_duties,
                if stale { " (LINK STALE)" } else { "" },
            );
        }
        thread::sleep(Duration::from_secs(1));
    }
}
