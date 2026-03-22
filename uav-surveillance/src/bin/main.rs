#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_net::{DhcpConfig, StackResources};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::peripherals::Peripherals;
use esp_hal::rng::Rng;
use esp_hal::timer::timg::TimerGroup;
use esp_radio::wifi::{ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent, WifiStaState};
use log::*;
use static_cell::StaticCell;
use uav_surveillance::mqtt;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const WIFI_SSID: &'static str = "Byt3Mage 5G";
const WIFI_PSWD: &'static str = "0zym@ndia$";
static RADIO_INIT: StaticCell<esp_radio::Controller> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

fn init_hardware() -> Peripherals {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Initialize heap memory (72KB)
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);

    peripherals
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();

    let peripherals = init_hardware();

    // init async executor
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0);

    let radio_init = RADIO_INIT.init(esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller"));

    let (wifi_controller, interfaces) = esp_radio::wifi::new(radio_init, peripherals.WIFI, Default::default())
        .expect("Failed to initialize Wi-Fi controller");

    let device = interfaces.sta;

    let rng = Rng::new();
    let net_seed = rng.random() as u64 | ((rng.random() as u64) << 32);

    let dhcp_config = DhcpConfig::default();
    let config = embassy_net::Config::dhcpv4(dhcp_config);
    let stack_resources = RESOURCES.init(StackResources::<3>::new());

    // Init network stack
    let (stack, runner) = embassy_net::new(device, config, stack_resources, net_seed);

    spawner.spawn(net_task(runner)).unwrap();
    spawner.spawn(wifi_connect(wifi_controller)).unwrap();

    loop {
        if stack.is_link_up() && stack.config_v4().is_some() {
            let ip = stack.config_v4().unwrap().address;
            info!("Got IP: {ip}");
            break;
        }

        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Starting MQTT task");
    spawner.spawn(mqtt::tasks::mqtt_task(stack)).unwrap();
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

#[embassy_executor::task]
async fn wifi_connect(mut controller: WifiController<'static>) {
    loop {
        if WifiStaState::Connected == esp_radio::wifi::sta_state() {
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            log::warn!("WiFi disconnected, reconnecting...");
            Timer::after(Duration::from_secs(1)).await;
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(WIFI_SSID.into())
                    .with_password(WIFI_PSWD.into()),
            );
            controller.set_config(&config).unwrap();
            controller.start_async().await.unwrap();
        }

        match controller.connect_async().await {
            Ok(()) => log::info!("WiFi connected"),
            Err(e) => {
                log::error!("WiFi connect failed: {:?}", e);
                Timer::after(Duration::from_secs(3)).await;
            }
        }
    }
}
