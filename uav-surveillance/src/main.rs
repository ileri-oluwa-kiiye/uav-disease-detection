use std::{thread, time::Duration};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition,
};

mod camera;
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
    let _server = stream_server::start()?;
    log::info!("Open http://{ip}/ in your browser");

    // MQTT thread
    //thread::Builder::new()
    //    .stack_size(8 * 1024)
    //    .name("mqtt".into())
    //    .spawn(mqtt_loop)?;

    // Camera thread
    //thread::Builder::new()
    //    .stack_size(16 * 1024)
    //    .name("camera".into())
    //    .spawn(camera_loop)?;

    // Idle main thread
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
