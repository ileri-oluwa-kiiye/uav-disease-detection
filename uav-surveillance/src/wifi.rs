use std::{net::Ipv4Addr, time::Duration};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    nvs::EspDefaultNvsPartition,
    sys::EspError,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

type WiFi = BlockingWifi<EspWifi<'static>>;

const WIFI_SSID: &str = std::env!("WIFI_SSID");
const WIFI_PASS: &str = std::env!("WIFI_PASS");

pub fn connect(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> Result<(WiFi, Ipv4Addr), EspError> {
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into().unwrap(),
        password: WIFI_PASS.try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;

    log::info!("Connecting to WiFi...");

    while let Err(err) = wifi.connect() {
        log::warn!("Connect failed: {err:?}, retrying in 1s...");
        std::thread::sleep(Duration::from_secs(1));
    }

    wifi.wait_netif_up()?;
    let ip = wifi.wifi().sta_netif().get_ip_info()?.ip;

    log::info!("WiFi Connected! IP: {ip}");

    Ok((wifi, ip))
}
