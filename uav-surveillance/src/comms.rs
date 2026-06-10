//! UART link to the STM32 flight controller.
//!
//! A dedicated thread owns the RX half and parses inbound Telemetry/Heartbeat
//! frames into shared state. The TX half lives behind a mutex in `CommsHandle`
//! so any thread can send RcCommand/ArmCommand. Wire format comes from the
//! shared `drone-protocol` crate, so this stays in lockstep with the STM side.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use drone_protocol::{Message, Parser, RcCommand, Telemetry, MAX_FRAME_SIZE};
use esp_idf_svc::hal::gpio::{Gpio1, Gpio2};
use esp_idf_svc::hal::{
    delay::BLOCK,
    gpio::AnyIOPin,
    uart::{config::Config, Uart, UartDriver, UartRxDriver, UartTxDriver},
    units::Hertz,
};

/// Cloneable handle for talking to the flight controller. Cheap to clone; share
/// it across threads (e.g. hand a clone to the MQTT control handler).
#[derive(Clone)]
pub struct Comms {
    tx: Arc<Mutex<UartTxDriver<'static>>>,
    telemetry: Arc<Mutex<Option<Telemetry>>>,
    last_rx_ms: Arc<AtomicU32>,
}

impl Comms {
    /// Encode and transmit one frame. The mutex keeps frames atomic relative to
    /// other senders, so the STM never sees two frames interleaved.
    pub fn send(&self, msg: Message) {
        let mut buf = [0u8; MAX_FRAME_SIZE];
        if let Some(n) = msg.encode(&mut buf) {
            if let Ok(mut tx) = self.tx.lock() {
                let _ = tx.write(&buf[..n]);
            }
        }
    }

    pub fn send_rc(&self, throttle: f32, roll: f32, pitch: f32) {
        self.send(Message::RcCommand(RcCommand {
            throttle,
            roll,
            pitch,
        }));
    }

    pub fn send_arm(&self, armed: bool) {
        self.send(Message::ArmCommand(armed));
    }

    /// Most recent telemetry frame, if any has arrived.
    pub fn telemetry(&self) -> Option<Telemetry> {
        *self.telemetry.lock().unwrap()
    }

    /// Milliseconds since the last valid inbound frame. Use this for a
    /// link-alive check on the ESP side.
    pub fn since_last_rx_ms(&self) -> u32 {
        now_ms().saturating_sub(self.last_rx_ms.load(Ordering::Relaxed))
    }
}

/// Start the UART link. `baud` must match the STM's USART config (115200).
pub fn start<UART: Uart + 'static>(
    uart: UART,
    tx_pin: Gpio1<'static>,
    rx_pin: Gpio2<'static>,
    baud: u32,
) -> anyhow::Result<Comms> {
    let config = Config::new().baudrate(Hertz(baud));

    let driver = UartDriver::new(
        uart,
        tx_pin,
        rx_pin,
        Option::<AnyIOPin>::None, // CTS
        Option::<AnyIOPin>::None, // RTS
        &config,
    )?;

    // The link lives for the whole program; leaking gives us 'static TX/RX
    // halves to move across threads without lifetime gymnastics.
    let driver: &'static mut UartDriver<'static> = Box::leak(Box::new(driver));
    let (uart_tx, uart_rx) = driver.split();

    let telemetry = Arc::new(Mutex::new(None));
    let last_rx_ms = Arc::new(AtomicU32::new(0));

    {
        let telemetry = telemetry.clone();
        let last_rx_ms = last_rx_ms.clone();
        thread::Builder::new()
            .name("uart-rx".into())
            .stack_size(4096)
            .spawn(move || rx_loop(uart_rx, telemetry, last_rx_ms))?;
    }

    Ok(Comms {
        tx: Arc::new(Mutex::new(uart_tx)),
        telemetry,
        last_rx_ms,
    })
}

fn rx_loop(
    rx: UartRxDriver<'static>,
    telemetry: Arc<Mutex<Option<Telemetry>>>,
    last_rx_ms: Arc<AtomicU32>,
) {
    let mut parser = Parser::new();
    let mut byte = [0u8; 1];

    // One byte per read with BLOCK: uart_read_bytes returns as soon as a byte is
    // available (reading a full chunk under BLOCK would wait for the whole
    // buffer to fill). Per-byte syscall cost is irrelevant at 115200 / 10 Hz.
    loop {
        match rx.read(&mut byte, BLOCK) {
            Ok(1) => {
                if let Some(msg) = parser.feed(byte[0]) {
                    last_rx_ms.store(now_ms(), Ordering::Relaxed);
                    if let Message::Telemetry(tele) = msg {
                        println!("got telemetry");
                        *telemetry.lock().unwrap() = Some(tele);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("UART read error: {e:?}");
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn now_ms() -> u32 {
    (unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1000) as u32
}
