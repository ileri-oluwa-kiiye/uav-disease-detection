use std::net::Ipv4Addr;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::modem::Modem,
    nvs::EspDefaultNvsPartition,
    wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi},
};

type WiFi = BlockingWifi<EspWifi<'static>>;

const WIFI_SSID: &str = std::env!("WIFI_SSID");
const WIFI_PASS: &str = std::env!("WIFI_PASS");

pub fn connect(
    modem: Modem<'static>,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
) -> anyhow::Result<(WiFi, Ipv4Addr)> {
    let mut wifi = BlockingWifi::wrap(EspWifi::new(modem, sysloop.clone(), Some(nvs))?, sysloop)?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: WIFI_SSID.try_into()?,
        password: WIFI_PASS.try_into()?,
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;

    loop {
        match wifi.connect() {
            Ok(()) => {
                log::info!("WiFi connected");
                break;
            }
            Err(e) => {
                log::warn!("Connect failed: {:?}, retrying in 2s...", e);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }

    wifi.wait_netif_up()?;

    let ip = wifi.wifi().sta_netif().get_ip_info()?.ip;
    log::info!("Connected! IP: {ip}");

    Ok((wifi, ip))
}
