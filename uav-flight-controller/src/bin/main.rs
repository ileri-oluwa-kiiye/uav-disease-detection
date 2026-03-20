#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use blocking_network_stack::{Socket, Stack};
use embedded_io::{Read, Write};
use esp_backtrace as _;
use esp_hal::peripherals::Peripherals;
use esp_hal::rng::Rng;
use esp_hal::time::{Duration, Instant};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{clock::CpuClock, main};
use esp_radio::wifi::{ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice};
use log::{info, warn};
use smoltcp::iface::{SocketSet, SocketStorage};
use smoltcp::wire::{DhcpOption, IpAddress};
use uav_flight_controller::delay;

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const WIFI_SSID: &'static str = "Byt3Mage 5G";
const WIFI_PSWD: &'static str = "0zym@ndia$";

fn init_hardware() -> Peripherals {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Initialize heap memory (72KB)
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 72 * 1024);

    peripherals
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    let peripherals = init_hardware();
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let rng = Rng::new();

    esp_rtos::start(timg0.timer0);

    let radio = esp_radio::init().expect("Failed to initialize Wi-Fi/BLE controller");
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(&radio, peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    let mut device = interfaces.sta;

    let mut socket_set_entries: [SocketStorage; 3] = Default::default();
    let mut socket_set = SocketSet::new(&mut socket_set_entries[..]);

    let mut dhcp_socket = smoltcp::socket::dhcpv4::Socket::new();

    dhcp_socket.set_outgoing_options(&[DhcpOption {
        kind: 12,
        data: b"uav-flight-controller",
    }]);

    socket_set.add(dhcp_socket);

    let now = || Instant::now().duration_since_epoch().as_millis();
    let mut stack = Stack::new(
        create_interface(&mut device),
        device,
        socket_set,
        now,
        rng.random(),
    );

    configure_wifi(&mut wifi_controller);
    scan_wifi(&mut wifi_controller);
    connect_wifi(&mut wifi_controller);
    obtain_ip(&mut stack);

    let mut rx_buffer = [0u8; 1536];
    let mut tx_buffer = [0u8; 1536];

    let socket = stack.get_socket(&mut rx_buffer, &mut tx_buffer);

    http_loop(socket);
}

pub fn create_interface(device: &mut esp_radio::wifi::WifiDevice) -> smoltcp::iface::Interface {
    // users could create multiple instances but since they only have one WifiDevice
    // they probably can't do anything bad with that
    smoltcp::iface::Interface::new(
        smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ethernet(
            smoltcp::wire::EthernetAddress::from_bytes(&device.mac_address()),
        )),
        device,
        timestamp(),
    )
}

// some smoltcp boilerplate
fn timestamp() -> smoltcp::time::Instant {
    smoltcp::time::Instant::from_micros(
        esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_micros() as i64,
    )
}

fn configure_wifi(controller: &mut WifiController<'_>) {
    controller
        .set_power_saving(esp_radio::wifi::PowerSaveMode::None)
        .unwrap();

    let client_config = ModeConfig::Client(
        ClientConfig::default()
            .with_ssid(WIFI_SSID.into())
            .with_password(WIFI_PSWD.into()),
    );

    let res = controller.set_config(&client_config);
    info!("wifi_set_configuration returned {:?}", res);

    controller.start().unwrap();
    info!("is wifi started: {:?}", controller.is_started());
}

fn scan_wifi(controller: &mut WifiController<'_>) {
    info!("Start Wifi Scan");
    let scan_config = ScanConfig::default().with_max(10);
    let results = controller.scan_with_config(scan_config).unwrap();
    results.iter().for_each(|ap| info!("{:?}", ap));
}

fn connect_wifi(controller: &mut WifiController<'_>) {
    info!("{:?}", controller.capabilities());
    info!("wifi_connect {:?}", controller.connect());
    info!("Wait for connection...");

    loop {
        match controller.is_connected() {
            Ok(true) => break,
            Ok(false) => {}
            Err(e) => warn!("{:?}", e),
        }
    }

    info!("Connected: {:?}", controller.is_connected())
}

fn obtain_ip(stack: &mut Stack<'_, esp_radio::wifi::WifiDevice<'_>>) {
    info!("Wait for IP address");

    loop {
        stack.work();
        if stack.is_iface_up() {
            info!("IP acquired: {:?}", stack.get_ip_info());
            break;
        }
    }
}

fn http_loop(mut socket: Socket<'_, '_, WifiDevice>) -> ! {
    info!("Starting HTTP client loop");

    loop {
        info!("Making HTTP request");
        socket.work();

        let remote_addr = IpAddress::v4(172, 217, 18, 115);
        socket.open(remote_addr, 80).unwrap();
        socket
            .write(b"GET / HTTP/1.0\r\nHost: www.mobile-j.de\r\n\r\n")
            .unwrap();
        socket.flush().unwrap();

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut buffer = [0u8; 512];

        while let Ok(len) = socket.read(&mut buffer) {
            let Ok(text) = core::str::from_utf8(&buffer[..len]) else {
                panic!("Invalid UTF-8 sequence encountered");
            };

            info!("Received: {}", text);

            if Instant::now() > deadline {
                info!("Timeout reached, exiting");
                break;
            }
        }

        socket.disconnect();
        let deadline = Instant::now() + Duration::from_secs(5);

        while Instant::now() < deadline {
            socket.work();
        }

        delay::ms(1000);
    }
}
