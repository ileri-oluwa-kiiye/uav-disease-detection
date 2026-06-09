use std::{thread, time::Duration};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop, hal::peripherals::Peripherals, nvs::EspDefaultNvsPartition,
};

mod bridge;
mod camera;
mod comms;
pub mod mqtt;
mod stream_server;
mod wifi;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    //log::info!("Initializing camera...");
    //camera::init()?;
    //log::info!("Camera ready");

    std::thread::sleep(Duration::from_secs(1));

    log::info!("Connecting to WiFi...");
    let (_wifi, ip) = wifi::connect(peripherals.modem, sysloop, nvs)?;
    //let _server = stream_server::start()?;
    //log::info!("Open http://{ip}/ in your browser");

    let comms = comms::start(
        peripherals.uart1,
        peripherals.pins.gpio1, // ESP TX -> STM RX (PA10)
        peripherals.pins.gpio2, // ESP RX -> STM TX (PA9)
        115_200,
    )?;

    bridge::start(comms);

    // Idle main thread
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
