use std::{thread, time::Duration};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

use crate::{frame_uploader::camera_loop, mqtt::session::mqtt_loop};

mod camera;
mod frame_uploader;
mod mqtt;
mod stream_server;

const SSID: &str = "Byt3Mage";
const PASSWORD: &str = "12345678";

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        ..Default::default()
    }))?;

    wifi.start()?;
    log::info!("WiFi started");

    wifi.connect()?;
    log::info!("WiFi connected");

    wifi.wait_netif_up()?;
    let ip = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("Got IP: {:?}", ip.ip);

    camera::init_camera()?;
    log::info!("Camera ready");

    let _server = stream_server::start_stream_server()?;

    // MQTT thread
    thread::Builder::new()
        .stack_size(8192)
        .name("mqtt".into())
        .spawn(mqtt_loop)?;

    // Camera thread
    thread::Builder::new()
        .stack_size(16384)
        .name("camera".into())
        .spawn(camera_loop)?;

    // Idle main thread
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
