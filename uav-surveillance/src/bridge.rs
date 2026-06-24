use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use drone_protocol::Telemetry;

use crate::comms::Comms;
use crate::mqtt::client::MqttV3Client;

const BROKER_ADDR: &str = std::env!("BROKER_ADDR");
const TOPIC_CONTROL: &str = "drone/control";
const TOPIC_TELEMETRY: &str = "drone/telemetry";
const RC_HZ: u64 = 50;

#[derive(Clone, Copy, Default)]
struct Control {
    armed: bool,
    throttle: f32,
    roll: f32,
    pitch: f32,
}

pub fn start(comms: Comms) {
    let last = Arc::new(Mutex::new(Control::default()));

    let recv_last = last.clone();
    thread::Builder::new()
        .name("mqtt-control".into())
        .stack_size(8192)
        .spawn(move || control_loop(recv_last))
        .unwrap();

    // Keepalive resender: pushes the latest control to the STM at RC_HZ so the
    // flight-controller link-loss watchdog stays fed even if MQTT goes quiet
    // (e.g. the dashboard tab is backgrounded and its timer is throttled).
    let ka_comms = comms.clone();
    thread::Builder::new()
        .name("rc-keepalive".into())
        .stack_size(4096)
        .spawn(move || loop {
            let c = *last.lock().unwrap();
            ka_comms.send_arm(c.armed);
            ka_comms.send_rc(c.throttle, c.roll, c.pitch);
            thread::sleep(Duration::from_millis(1000 / RC_HZ));
        })
        .unwrap();

    thread::Builder::new()
        .name("mqtt-telemetry".into())
        .stack_size(8192)
        .spawn(move || telemetry_loop(comms))
        .unwrap();
}

// MQTT (drone/control) -> STM. Also re-sends the latest control at RC_HZ so the
// STM's link-loss watchdog stays fed even when the dashboard is idle.
fn control_loop(last: Arc<Mutex<Control>>) {
    loop {
        let mut client = match MqttV3Client::connect_tcp(BROKER_ADDR, Duration::from_secs(10)) {
            Ok(c) => c,
            Err(e) => {
                log::error!("control connect failed: {e:?}");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if client.connect("uav-esp32-ctl", None, None, 60).is_err()
            || client.subscribe(TOPIC_CONTROL, 0).is_err()
        {
            thread::sleep(Duration::from_secs(5));
            continue;
        }

        log::info!("control link up");

        let mut topic_buf = [0u8; 64];
        let mut payload_buf = [0u8; 512];

        loop {
            match client.read_message(&mut topic_buf, &mut payload_buf) {
                Ok(Some((topic, payload))) if topic == TOPIC_CONTROL => {
                    if let Some(c) = parse_control(payload) {
                        *last.lock().unwrap() = c;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("control link lost: {e:?}");
                    break;
                }
            }
        }
    }
}

// STM telemetry -> MQTT (drone/telemetry), ~10 Hz.
fn telemetry_loop(comms: Comms) {
    loop {
        let mut client = match MqttV3Client::connect_tcp(BROKER_ADDR, Duration::from_secs(10)) {
            Ok(c) => {
                log::info!("telemetry connected");
                c
            }
            Err(e) => {
                log::error!("telemetry connect failed: {e:?}");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        if client.connect("uav-esp32-tel", None, None, 60).is_err() {
            thread::sleep(Duration::from_secs(5));
            continue;
        }

        log::info!("telemetry link up");

        loop {
            if let Some(t) = comms.telemetry() {
                let mut buf = [0u8; 256];
                let json = format_telemetry(&mut buf, &t);
                if client.publish(TOPIC_TELEMETRY, json, 0, false).is_err() {
                    log::warn!("telemetry publish failed");
                    break;
                }
            }

            thread::sleep(Duration::from_millis(100));
        }
    }
}

// Minimal field extraction from the dashboard's ControlState JSON.
fn parse_control(payload: &[u8]) -> Option<Control> {
    let s = core::str::from_utf8(payload).ok()?;
    Some(Control {
        armed: json_bool(s, "armed").unwrap_or(false),
        throttle: json_num(s, "throttle").unwrap_or(0.0),
        roll: json_num(s, "roll").unwrap_or(0.0),
        pitch: json_num(s, "pitch").unwrap_or(0.0),
    })
}

fn json_bool(s: &str, key: &str) -> Option<bool> {
    let rest = after_key(s, key)?;
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_num(s: &str, key: &str) -> Option<f32> {
    let rest = after_key(s, key)?;
    let end = rest
        .find(|c: char| {
            !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')
        })
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn after_key<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let mut pat = String::with_capacity(key.len() + 2);
    pat.push('"');
    pat.push_str(key);
    pat.push('"');
    let i = s.find(&pat)? + pat.len();
    let colon = s[i..].find(':')? + i + 1;
    Some(s[colon..].trim_start())
}

fn format_telemetry<'a>(buf: &'a mut [u8], t: &Telemetry) -> &'a [u8] {
    use std::io::Write;
    let mut w = std::io::Cursor::new(buf);
    let _ = write!(
        w,
        "{{\"attitude\":{{\"roll\":{:.2},\"pitch\":{:.2},\"yaw\":{:.2}}},\"motors\":[{},{},{},{}],\"armed\":{},\"tick\":{}}}",
        t.roll, t.pitch, t.yaw,
        t.motor_duties[0], t.motor_duties[1], t.motor_duties[2], t.motor_duties[3],
        t.armed,
        t.tick
    );
    let n = w.position() as usize;
    &w.into_inner()[..n]
}
