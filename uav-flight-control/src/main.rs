#![no_std]
#![no_main]

mod board;
mod drivers;
mod flight_control;
mod math;

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = stm32h7xx_hal::stm32, peripherals = true, dispatchers = [SPI1])]
mod app {
    use drone_protocol::{Message, Parser, MAX_FRAME_SIZE};
    use rtic_monotonics::stm32::prelude::*;
    use stm32h7xx_hal::{
        nb,
        prelude::*,
        rcc::rec::{Spi123ClkSel, UsbClkSel},
        serial, spi,
        timer::Event,
    };

    use crate::{
        board::{self},
        drivers::{
            icm42688p::Icm42688p,
            motors::{MOTOR_FL, MOTOR_FR, MOTOR_RL, MOTOR_RR},
        },
        flight_control::{Commands, FlightControl, Telemetry},
    };

    stm32_tim5_monotonic!(Mono, 1_000_000); // 1 MHz -> microsecond ticks

    const RC_TIMEOUT_US: u64 = 500_000;

    #[shared]
    struct Shared {
        commands: Commands,
        telemetry: Telemetry,
        uart_tx: board::Serial1Tx,
    }

    #[local]
    struct Local {
        // Flight loop (TIM2 ISR owns these exclusively)
        fc: FlightControl,
        // Idle loop
        led: board::Led,
        // UART serial loop
        uart_rx: board::Serial1Rx,
        // comms protocol parser
        parser: Parser,
    }

    #[init]
    fn init(ctx: init::Context) -> (Shared, Local) {
        let dp = ctx.device;

        // Power
        let pwr = dp.PWR.constrain();
        let pwrcfg = pwr.freeze();

        // Clocks
        let rcc = dp.RCC.constrain();
        let mut ccdr = rcc
            .use_hse(25.MHz())
            .sys_ck(240.MHz())
            .freeze(pwrcfg, &dp.SYSCFG);

        // Peripheral clocks
        ccdr.clocks.hsi48_ck().expect("HSI48 must run");
        ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::Hsi48);
        ccdr.peripheral.kernel_spi123_clk_mux(Spi123ClkSel::Per);

        // LED
        let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
        let led = gpioe.pe3.into_push_pull_output();

        // SPI2 for IMU
        let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
        let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
        let cs = gpiob.pb12.into_push_pull_output();
        let spi2 = dp.SPI2.spi(
            (
                gpiob.pb13.into_alternate::<5>(),
                gpioc.pc2.into_alternate::<5>(),
                gpioc.pc3.into_alternate::<5>(),
            ),
            spi::Config::new(spi::MODE_3).communication_mode(spi::CommunicationMode::FullDuplex),
            1.MHz(),
            ccdr.peripheral.SPI2,
            &ccdr.clocks,
        );

        let tim5_clock_hz = ccdr.clocks.timx_ker_ck().raw();
        defmt::info!("TIM5 ker_ck = {} Hz", tim5_clock_hz);
        Mono::start(240_000_000);

        let mut imu = Icm42688p::new(spi2, cs);
        imu.init().expect("IMU init failed");

        let mut attempts = 0;

        while attempts < 5 {
            // Gyro bias — craft MUST be stationary and level at power-up.
            match imu.calibrate_gyro(2000) {
                Ok(bias) => {
                    defmt::info!("gyro bias (dps): {}, tries {}x", bias, attempts + 1);
                    break;
                }
                Err(crate::drivers::icm42688p::Error::MotionDetected(spread)) => {
                    attempts += 1;
                    defmt::warn!("gyro cal spread {} dps too high", spread)
                }
                Err(_) => {
                    defmt::warn!("gyro cal: sensor error");
                    break;
                }
            }
        }

        // PWM for motors (TIM3, 50Hz)
        let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
        let (mut ch1, mut ch2, mut ch3, mut ch4) = dp.TIM3.pwm(
            (
                gpioa.pa6.into_alternate::<2>(),
                gpioa.pa7.into_alternate::<2>(),
                gpiob.pb0.into_alternate::<2>(),
                gpiob.pb1.into_alternate::<2>(),
            ),
            50.Hz(),
            ccdr.peripheral.TIM3,
            &ccdr.clocks,
        );

        ch1.enable();
        ch2.enable();
        ch3.enable();
        ch4.enable();
        let max_duty = ch1.get_max_duty() as u16;

        defmt::info!("max duty: {}", max_duty);

        // ESC idle = 1000µs of the 20ms (50Hz) frame — the same value Motors emits
        // when disarmed. Write it the instant the outputs go live so the ESCs see a
        // valid minimum-throttle signal instead of the 0µs reset value.
        let idle_duty = max_duty / 20;
        ch1.set_duty(idle_duty);
        ch2.set_duty(idle_duty);
        ch3.set_duty(idle_duty);
        ch4.set_duty(idle_duty);

        // Hold steady idle so the ESCs arm/calibrate before the flight loop and UART
        // RX come online. Interrupts are masked until init returns, so nothing else
        // touches the outputs during this window.
        cortex_m::asm::delay(480_000_000); // ~2s at 240MHz

        // TIM2 at 1kHz
        let mut timer = dp.TIM2.timer(1.kHz(), ccdr.peripheral.TIM2, &ccdr.clocks);
        timer.listen(Event::TimeOut);

        // USART1 on PA9/PA10 for ESP32 comms
        let tx_pin = gpioa.pa9.into_alternate::<7>();
        let rx_pin = gpioa.pa10.into_alternate::<7>();

        let config = serial::config::Config::default()
            .baudrate(115200.bps())
            .parity_none();

        let mut uart = dp
            .USART1
            .serial(
                (tx_pin, rx_pin),
                config,
                ccdr.peripheral.USART1,
                &ccdr.clocks,
            )
            .unwrap();
        uart.listen(serial::Event::Rxne);

        let (uart_tx, uart_rx) = uart.split();

        let mut fc = FlightControl::init(imu, timer, ch1, ch2, ch3, ch4, max_duty);
        // Settle the AHRS and capture level trim (craft still level & stationary).
        fc.calibrate_level(1500, 500);
        defmt::info!("level trim (deg): {}", fc.level_trim);

        (
            Shared {
                telemetry: Telemetry::default(),
                commands: Commands::default(),
                uart_tx,
            },
            Local {
                fc,
                led,
                uart_rx,
                parser: Parser::new(),
            },
        )
    }

    /// Flight control loop, runs at exactly 1kHz in TIM2 ISR. Highest priority.
    #[task(binds = TIM2, priority = 2, local = [fc], shared = [telemetry, commands])]
    fn flight_loop(mut ctx: flight_loop::Context) {
        let fc = ctx.local.fc;

        // 1. Clear interrupt
        fc.timer.clear_irq();

        // fc.tick_count = fc.tick_count.wrapping_add(1);
        // if fc.tick_count % 4 == 0 {
        //     if let Ok(raw) = fc.imu.read_scaled() {
        //         defmt::info!("gyro: [{}, {}, {}]", raw.gyro_x, raw.gyro_y, raw.gyro_z);
        //     }
        // }

        // 2. Read commands (short lock)
        let cmds = ctx.shared.commands.lock(|c| *c);

        // 3. Link-loss failsafe: if RC frames have gone stale, treat as disarm
        // and hold level setpoints. last_rc_us == 0 means "no frame ever yet",
        // which also fails safe (stays disarmed until the first real command).
        let now_us = Mono::now().ticks();
        let link_alive =
            cmds.last_rc_us != 0 && now_us.saturating_sub(cmds.last_rc_us) < RC_TIMEOUT_US;

        let (arm_request, desired_angles, base_throttle) = if link_alive {
            (cmds.arm_request, cmds.desired_angles, cmds.base_throttle)
        } else {
            (false, [0.0; 3], 0.0)
        };

        // 4. Arm / disarm
        if arm_request {
            fc.motors.arm();
        } else {
            fc.motors.disarm();
            fc.rate_pids.reset();
            fc.angle_pids.reset();
            fc.gyro_lpf.reset();
        }

        // 5. Read IMU
        let read = match fc.imu.read_scaled() {
            Ok(r) => r,
            Err(_) => return,
        };

        // 6. AHRS update
        fc.ahrs.update(
            read.gyro_x,
            read.gyro_y,
            read.gyro_z,
            read.accel_x,
            read.accel_y,
            read.accel_z,
        );

        fc.tick_count = fc.tick_count.wrapping_add(1);
        fc.base_throttle = base_throttle;

        // 7. Software low-pass on gyro, feeding ONLY the rate loop. This is the
        // signal the derivative term sees, so it must be clean.
        let gyro_filt = fc.gyro_lpf.update([read.gyro_x, read.gyro_y, read.gyro_z]);

        // 8. Outer loop at 250Hz: angle -> desired rate
        if fc.tick_count % 4 == 0 {
            let att = fc.attitude_trimmed();
            let out = fc
                .angle_pids
                .update(desired_angles, [att.roll, att.pitch, att.yaw], 0.004);
            fc.desired_rates = [out.roll.output, out.pitch.output, out.yaw.output];
        }

        // 9. Inner loop at 1kHz: desired rate -> correction
        let pid_out = fc.rate_pids.update(fc.desired_rates, gyro_filt, 0.001);

        // 10. Mix motors
        fc.motors.mix(
            fc.base_throttle,
            pid_out.roll.output,
            pid_out.pitch.output,
            pid_out.yaw.output,
        );

        // 11. Write PWM
        let duties = fc.motors.duties();
        fc.ch1.set_duty(duties[MOTOR_FL]);
        fc.ch2.set_duty(duties[MOTOR_FR]);
        fc.ch3.set_duty(duties[MOTOR_RL]);
        fc.ch4.set_duty(duties[MOTOR_RR]);

        // 12. Update telemetry (short lock)
        ctx.shared.telemetry.lock(|t| {
            t.attitude = fc.attitude_trimmed();
            t.gyro = gyro_filt;
            t.accl = [read.accel_x, read.accel_y, read.accel_z];
            t.pid_output = pid_out;
            t.motor_duties = duties;
            t.throttle = fc.base_throttle;
            t.tick = fc.tick_count;
            t.armed = fc.motors.is_armed();
        });
    }

    /// UART RX interrupt. Parse incoming bytes from ESP32
    #[task(binds = USART1, priority = 1, local = [uart_rx, parser], shared = [commands])]
    fn uart_rx(mut ctx: uart_rx::Context) {
        let Ok(byte) = ctx.local.uart_rx.read() else {
            return;
        };

        let Some(msg) = ctx.local.parser.feed(byte) else {
            return;
        };

        ctx.shared.commands.lock(|cmds| {
            cmds.last_rc_us = Mono::now().ticks();
            match msg {
                Message::RcCommand(drone_protocol::RcCommand {
                    throttle,
                    roll,
                    pitch,
                }) => {
                    cmds.base_throttle = throttle;
                    cmds.desired_angles[0] = roll;
                    cmds.desired_angles[1] = pitch;
                }
                Message::ArmCommand(armed) => cmds.arm_request = armed,
                _ => {}
            }
        });
    }

    /// Idle loop, runs when no ISR is active
    /// Handles telemetry output over serial
    #[idle(local = [led], shared = [commands, telemetry, uart_tx])]
    fn idle(mut ctx: idle::Context) -> ! {
        let mut counter: u32 = 0;
        let mut tx_buf = [0u8; MAX_FRAME_SIZE];

        loop {
            counter = counter.wrapping_add(1);

            if counter % 500_000 == 0 {
                ctx.local.led.toggle();

                let t = ctx.shared.telemetry.lock(|t| *t);

                defmt::info!(
                    "armed:{} att:{} pid_r:{} pid_p: {} thr:{} motors:{}",
                    t.armed,
                    [t.attitude.roll, t.attitude.pitch, t.attitude.yaw],
                    t.pid_output.roll.output,
                    t.pid_output.pitch.output,
                    t.throttle,
                    t.motor_duties,
                );

                let len = Message::Telemetry(drone_protocol::Telemetry {
                    roll: t.attitude.roll,
                    pitch: t.attitude.pitch,
                    yaw: t.attitude.yaw,
                    throttle: t.throttle,
                    motor_duties: t.motor_duties,
                    armed: t.armed,
                    tick: t.tick,
                })
                .encode(&mut tx_buf)
                .unwrap();

                ctx.shared.uart_tx.lock(|tx| {
                    for &byte in &tx_buf[..len] {
                        let _ = nb::block!(tx.write(byte));
                    }
                });
            }
        }
    }
}
