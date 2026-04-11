use std::{thread, time::Duration};

use super::{client::MqttV3Client, MqttError};

const BROKER_ADDR: &str = "mqtt.thingsboard.cloud:1883";
const CLIENT_ID: &str = "6aaj58yw4sn5cc0pvkwp";
const USERNAME: Option<&str> = Some("jw1uuebkpfpldi5zupeo");
const PASSWORD: Option<&str> = Some("pn7bxmkmrk9e4nhytp5z");
const KEEP_ALIVE: u16 = 60;

const TELEMETRY_TOPIC: &str = "v1/devices/me/telemetry";
const RPC_REQUEST_TOPIC: &str = "v1/devices/me/rpc/request/+";
const STATUS_TELEMETRY: &[u8] = b"{\"status\": \"online\"}";

pub fn mqtt_loop() {
    loop {
        log::info!("Connecting to MQTT broker...");

        let result = mqtt_session();

        log::error!("MQTT session ended: {result:?}");
        log::info!("Reconnecting in 5s...");

        thread::sleep(Duration::from_secs(5));
    }
}

fn mqtt_session() -> Result<(), MqttError> {
    loop {
        let mut mqtt = MqttV3Client::connect_tcp(BROKER_ADDR, Duration::from_secs(10))?;
        mqtt.connect(CLIENT_ID, USERNAME, PASSWORD, KEEP_ALIVE)?;
        log::info!("MQTT connected");

        // publish status telemetry
        mqtt.publish(TELEMETRY_TOPIC, STATUS_TELEMETRY, 0, false)?;
        log::info!("Telemetry published");

        // Subscribe to RPC
        mqtt.subscribe(RPC_REQUEST_TOPIC, 1)?;
        log::info!("Subscribed to RPC");

        // Mesage loop
        let mut topic_buf = [0u8; 256];
        let mut payload_buf = [0u8; 512];

        loop {
            match mqtt.read_message(&mut topic_buf, &mut payload_buf)? {
                Some((topic, payload)) => {
                    let msg = core::str::from_utf8(payload).unwrap_or("<binary>");
                    log::info!("[{}]: {}", topic, msg);
                }
                None => {}
            }
        }
    }
}
