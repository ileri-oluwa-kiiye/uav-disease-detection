//! Flight control loop — runs in TIM2 ISR at 1kHz
//!
//! Architecture:
//!   TIM2 ISR (1kHz, every 1ms):
//!     1. Clear timer interrupt
//!     2. Read IMU via SPI
//!     3. Run AHRS filter (Madgwick)
//!     4. Every 4th tick: run outer loop (angle PID, 250Hz)
//!     5. Run inner loop (rate PID, 1kHz)
//!     6. Mix motor outputs
//!     7. Write PWM duty cycles
//!
//!   Main loop:
//!     - Reads latest telemetry via snapshot()
//!     - Sets desired angles and throttle via set_setpoints()
//!     - Handles arming via arm() / disarm()

use stm32h7xx_hal::{pac::TIM2, timer::Timer};

use crate::{
    board::{self},
    drivers::{
        ahrs::{Attitude, MadgwickFilter},
        motors::Motors,
        pid::{FlightPidOutput, FlightPids, PidConfig, PidGains},
    },
};

/// Telemetry snapshot. Read by main loop for USB output
#[derive(Clone, Copy, Default)]
pub struct Telemetry {
    pub attitude: Attitude,
    pub gyro: [f32; 3],
    pub accl: [f32; 3],
    pub pid_output: FlightPidOutput,
    pub motor_duties: [u16; 4],
    pub tick: u32,
    pub armed: bool,
}

/// Commands from main loop to ISR
#[derive(Clone, Copy, Default)]
pub struct Commands {
    pub desired_angles: [f32; 3],
    pub base_throttle: f32,
    pub arm_request: bool,
}

/// Everything the ISR needs
pub struct FlightControl {
    // Hardware
    pub imu: board::Imu,
    pub ch1: board::PwmCh1,
    pub ch2: board::PwmCh2,
    pub ch3: board::PwmCh3,
    pub ch4: board::PwmCh4,
    pub timer: Timer<TIM2>,

    // Filters & controllers
    pub ahrs: MadgwickFilter,
    pub rate_pids: FlightPids,
    pub angle_pids: FlightPids,
    pub motors: Motors,

    // State
    pub desired_rates: [f32; 3],
    pub tick_count: u32,
}

impl FlightControl {
    /// Initialize the flight control loop.
    /// Takes ownership of all flight-critical hardware.
    /// After this call, the TIM2 ISR owns the IMU, motors, and runs the full loop.
    pub fn init(
        imu: board::Imu,
        timer: Timer<TIM2>,
        ch1: board::PwmCh1,
        ch2: board::PwmCh2,
        ch3: board::PwmCh3,
        ch4: board::PwmCh4,
        max_duty: u16,
    ) -> Self {
        Self {
            imu,
            timer,
            ch1,
            ch2,
            ch3,
            ch4,
            ahrs: MadgwickFilter::new(1000.0, 0.033),
            rate_pids: FlightPids::new(default_rate_config(), default_rate_yaw_config()),
            angle_pids: FlightPids::new(default_angle_config(), default_angle_yaw_config()),
            motors: Motors::new(max_duty),
            desired_rates: [0.0; 3],
            tick_count: 0,
        }
    }
}

/// PID configuration defaults — tune these with motors running
pub fn default_rate_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 0.5,
            ki: 0.1,
            kd: 0.01,
        },
        integral_limit: 50.0,
        output_limit: 0.3, // max contribution to throttle [0-1]
    }
}

pub fn default_rate_yaw_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 0.3,
            ki: 0.05,
            kd: 0.0,
        },
        integral_limit: 50.0,
        output_limit: 0.2,
    }
}

pub fn default_angle_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 4.0,
            ki: 0.0,
            kd: 0.0,
        },
        integral_limit: 20.0,
        output_limit: 300.0,
    }
}

pub fn default_angle_yaw_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 2.0,
            ki: 0.0,
            kd: 0.0,
        },
        integral_limit: 20.0,
        output_limit: 200.0,
    }
}
