use embassy_futures::select::{Either, select};
use embassy_net::{IpEndpoint, Ipv4Address, Stack, tcp::TcpSocket};
use embassy_time::{Duration, Timer};

use super::{MqttError, client::MqttV3Client};

const BROKER_ADDR: Ipv4Address = Ipv4Address::new(52, 73, 3, 142);
const BROKER_PORT: u16 = 1883;

const CLIENT_ID: &str = "6aaj58yw4sn5cc0pvkwp";
const USERNAME: Option<&str> = Some("jw1uuebkpfpldi5zupeo");
const PASSWORD: Option<&str> = Some("pn7bxmkmrk9e4nhytp5z");
const KEEP_ALIVE: u16 = 60;

const TELEMETRY_TOPIC: &str = "v1/devices/me/telemetry";
const RPC_REQUEST_TOPIC: &str = "v1/devices/me/rpc/request/+";
const RPC_RESPONSE_PREFIX: &[u8] = b"v1/devices/me/rpc/response/";

fn build_rpc_response_topic<'a>(buf: &'a mut [u8], request_id: &str) -> &'a str {
    let prefix_len = RPC_RESPONSE_PREFIX.len();
    buf[..prefix_len].copy_from_slice(RPC_RESPONSE_PREFIX);
    buf[prefix_len..prefix_len + request_id.len()].copy_from_slice(request_id.as_bytes());
    core::str::from_utf8(&buf[..prefix_len + request_id.len()]).unwrap()
}

#[embassy_executor::task]
pub async fn mqtt_task(stack: Stack<'static>) {
    // Wait for network
    loop {
        if stack.is_link_up() && stack.config_v4().is_some() {
            let ip = stack.config_v4().unwrap().address;
            log::info!("Network ready, IP: {ip}");
            break;
        }

        Timer::after_millis(500).await;
    }

    loop {
        if let Err(e) = mqtt_session(stack).await {
            log::error!("MQTT session ended: {e:?}");
            log::info!("Reconnecting in 5s...");
            Timer::after(Duration::from_secs(5)).await;
        }
    }
}

async fn mqtt_session(stack: Stack<'_>) -> Result<(), MqttError> {
    let mut rx_buf = [0u8; 4096];
    let mut tx_buf = [0u8; 4096];
    let mut socket = TcpSocket::new(stack, &mut rx_buf, &mut tx_buf);

    // Connect TCP
    socket.set_timeout(Some(Duration::from_secs(10)));
    socket.connect(IpEndpoint::new(BROKER_ADDR.into(), BROKER_PORT)).await?;
    log::info!("TCP connected");

    let mut mqtt = MqttV3Client::new(socket);
    mqtt.connect(CLIENT_ID, USERNAME, PASSWORD, KEEP_ALIVE).await?;
    log::info!("MQTT connected");

    // publish initial telemetry
    mqtt.publish(TELEMETRY_TOPIC, b"{\"status\": \"online\"}", 0, false)
        .await?;
    log::info!("Telemetry published");

    // Subscribe to RPC
    mqtt.subscribe(RPC_REQUEST_TOPIC, 1).await?;
    log::info!("Subscribed to RPC");

    // Mesage loop
    let mut topic_buf = [0u8; 256];
    let mut payload_buf = [0u8; 512];

    loop {
        match select(
            mqtt.read_message(&mut topic_buf, &mut payload_buf),
            Timer::after(Duration::from_secs(30)),
        )
        .await
        {
            Either::First(result) => {
                if let Some((topic, payload)) = result? {
                    let msg = core::str::from_utf8(payload).unwrap_or("<binary>");
                    log::info!("[{}]: {}", topic, msg);
                }
            }
            Either::Second(_) => {
                mqtt.ping().await?;
            }
        }
    }
}
