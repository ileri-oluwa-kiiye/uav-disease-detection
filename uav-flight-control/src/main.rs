#![no_std]
#![no_main]

mod board;
mod drivers;
mod flight_control;
mod math;
mod protocol;

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = stm32h7xx_hal::stm32, peripherals = true, dispatchers = [SPI1])]
mod app {
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
            usb_serial::{self, UsbDev, UsbSer},
        },
        flight_control::{Commands, FlightControl, Telemetry},
        protocol,
    };

    #[shared]
    struct Shared {
        usb_ser: UsbSer,
        commands: Commands,
        telemetry: Telemetry,
        uart_tx: board::Serial1Tx,
    }

    #[local]
    struct Local {
        // Flight loop (TIM2 ISR owns these exclusively)
        fc: FlightControl,
        // USB serial loop
        usb_dev: UsbDev,
        // Idle loop
        led: board::Led,
        // UART serial loop
        uart_rx: board::Serial1Rx,
        parser: protocol::Parser,
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

        // USB
        let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
        let (usb_dev, usb_ser) = usb_serial::init(
            dp.OTG2_HS_GLOBAL,
            dp.OTG2_HS_DEVICE,
            dp.OTG2_HS_PWRCLK,
            gpioa.pa11.into_alternate::<10>(),
            gpioa.pa12.into_alternate::<10>(),
            ccdr.peripheral.USB2OTG,
            &ccdr.clocks,
        );

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

        let mut imu = Icm42688p::new(spi2, cs);
        imu.init().expect("IMU init failed");

        // PWM for motors (TIM3, 50Hz)
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

        (
            Shared {
                usb_ser,
                telemetry: Telemetry::default(),
                commands: Commands::default(),
                uart_tx,
            },
            Local {
                fc: FlightControl::init(imu, timer, ch1, ch2, ch3, ch4, max_duty),
                usb_dev,
                led,
                uart_rx,
                parser: protocol::Parser::new(),
            },
        )
    }

    /// Flight control loop, runs at exactly 1kHz in TIM2 ISR
    /// Priority 2 (higher than USB, lower than nothing)
    #[task(binds = TIM2, priority = 2, local = [fc], shared = [telemetry, commands])]
    fn flight_loop(mut ctx: flight_loop::Context) {
        let fc = ctx.local.fc;

        // 1. Clear interrupt
        fc.timer.clear_irq();

        // 2. Read commands (short lock)
        let cmds = ctx.shared.commands.lock(|c| *c);

        // 3. Arm/disarm
        match cmds.arm_request {
            true => fc.motors.arm(),
            false => {
                fc.motors.disarm();
                fc.rate_pids.reset();
                fc.angle_pids.reset();
            }
        }

        // 4. Read IMU
        let reading = match fc.imu.read_scaled() {
            Ok(r) => r,
            Err(_) => return,
        };

        // 5. AHRS update
        fc.ahrs.update(
            reading.gyro_x,
            reading.gyro_y,
            reading.gyro_z,
            reading.accel_x,
            reading.accel_y,
            reading.accel_z,
        );

        fc.tick_count = fc.tick_count.wrapping_add(1);

        // 6. Outer loop at 250Hz
        if fc.tick_count % 4 == 0 {
            let att = fc.ahrs.attitude();

            let angle_out =
                fc.angle_pids
                    .update(cmds.desired_angles, [att.roll, att.pitch, att.yaw], 0.004);

            fc.desired_rates = [
                angle_out.roll.output,
                angle_out.pitch.output,
                angle_out.yaw.output,
            ];
        }

        // 7. Inner loop at 1kHz
        let pid_out = fc.rate_pids.update(
            fc.desired_rates,
            [reading.gyro_x, reading.gyro_y, reading.gyro_z],
            0.001,
        );

        // 8. Mix Motors
        fc.motors.mix(
            cmds.base_throttle,
            pid_out.roll.output,
            pid_out.pitch.output,
            pid_out.yaw.output,
        );

        // 9. Write PWM
        let duties = fc.motors.duties();
        fc.ch1.set_duty(duties[MOTOR_FL]);
        fc.ch2.set_duty(duties[MOTOR_FR]);
        fc.ch3.set_duty(duties[MOTOR_RL]);
        fc.ch4.set_duty(duties[MOTOR_RR]);

        // 10. Update telemetry (short lock)
        ctx.shared.telemetry.lock(|t| {
            t.attitude = fc.ahrs.attitude();
            t.gyro = [reading.gyro_x, reading.gyro_y, reading.gyro_z];
            t.motor_duties = duties;
            t.tick = fc.tick_count;
            t.armed = fc.motors.is_armed();
        });
    }

    /// UART RX interrupt. Parse incoming bytes from ESP32
    #[task(binds = USART1, priority = 1, local = [uart_rx, parser], shared = [commands])]
    fn uart_rx(mut ctx: uart_rx::Context) {
        if let Ok(byte) = ctx.local.uart_rx.read() {
            if let Some(msg) = ctx.local.parser.feed(byte) {
                match msg {
                    protocol::RxMessage::RcCommand {
                        throttle,
                        roll,
                        pitch,
                    } => {
                        ctx.shared.commands.lock(|cmds| {
                            cmds.base_throttle = throttle;
                            cmds.desired_angles[0] = roll;
                            cmds.desired_angles[1] = pitch;
                        });
                    }
                    protocol::RxMessage::ArmCommand { armed } => {
                        ctx.shared.commands.lock(|cmds| {
                            cmds.arm_request = armed;
                        });
                    }
                }
            }
        }
    }

    /// USB interrupt
    #[task(binds = OTG_FS, priority = 1, local = [usb_dev], shared = [usb_ser])]
    fn usb_irq(mut ctx: usb_irq::Context) {
        let dev = ctx.local.usb_dev;
        ctx.shared.usb_ser.lock(|ser| dev.poll(&mut [ser]));
    }

    /// Idle loop, runs when no ISR is active
    /// Handles telemetry output over serial
    #[idle(local = [led], shared = [telemetry, uart_tx])]
    fn idle(mut ctx: idle::Context) -> ! {
        let mut counter: u32 = 0;
        let mut tx_buf = [0u8; 40];

        loop {
            counter = counter.wrapping_add(1);

            // Send telemetry at ~10Hz
            if counter % 500_000 == 0 {
                ctx.local.led.toggle();

                let t = ctx.shared.telemetry.lock(|t| *t);

                defmt::info!("r:{}, p:{}", t.attitude.roll, t.attitude.pitch);

                // UART telemetry to ESP32
                let len = protocol::encode_telemetry(
                    &mut tx_buf,
                    t.attitude.roll,
                    t.attitude.pitch,
                    t.attitude.yaw,
                    0.0,
                    t.motor_duties,
                    t.armed,
                );

                ctx.shared.uart_tx.lock(|tx| {
                    for &byte in &tx_buf[..len] {
                        let _ = nb::block!(tx.write(byte));
                    }
                })
            }
        }
    }
}
