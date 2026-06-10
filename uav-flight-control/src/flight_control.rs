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
    position::{PositionConfig, PositionController, PositionEstimate},
};

#[derive(Clone, Copy, PartialEq, Default)]
pub enum FlightMode {
    #[default]
    Manual,
    Position,
}

/// Telemetry snapshot
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
    pub last_rc_us: u64,
    pub manual_override: bool,
    pub move_request: Option<[f32; 3]>,
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
    pub level_trim: [f32; 2],
    pub rate_pids: FlightPids,
    pub angle_pids: FlightPids,
    pub motors: Motors,

    // State
    pub desired_rates: [f32; 3],
    pub tick_count: u32,

    // control
    pub mode: FlightMode,
    pub position_ctrl: PositionController,
    pub estimator: PositionEstimate,
    pub nav_target: [f32; 3],
    pub nav_yaw: f32,
    pub base_throttle: f32,
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
            level_trim: [0.0; 2],
            rate_pids: FlightPids::new(default_rate_config(), default_rate_yaw_config()),
            angle_pids: FlightPids::new(default_angle_config(), default_angle_yaw_config()),
            motors: Motors::new(max_duty),
            desired_rates: [0.0; 3],
            tick_count: 0,
            mode: FlightMode::Manual,
            position_ctrl: PositionController::new(default_position_config()),
            estimator: PositionEstimate::default(),
            nav_target: [0.0; 3],
            nav_yaw: 0.0,
            base_throttle: 0.0,
        }
    }

    /// Run the AHRS to convergence on a level, stationary craft, then capture
    /// the residual roll/pitch as a level-trim offset. Call once after the gyro
    /// is calibrated and before flight. ~1 ms per iteration at 240 MHz.
    pub fn calibrate_level(&mut self, settle_iters: u32, trim_samples: u32) {
        // 1. Let the Madgwick quaternion converge from identity to true gravity.
        for _ in 0..settle_iters {
            if let Ok(r) = self.imu.read_scaled() {
                self.ahrs.update(
                    r.gyro_x, r.gyro_y, r.gyro_z, r.accel_x, r.accel_y, r.accel_z,
                );
            }
            cortex_m::asm::delay(240_000);
        }

        // 2. Average the converged attitude as the level reference.
        let mut sum_r = 0.0f32;
        let mut sum_p = 0.0f32;
        for _ in 0..trim_samples {
            if let Ok(r) = self.imu.read_scaled() {
                self.ahrs.update(
                    r.gyro_x, r.gyro_y, r.gyro_z, r.accel_x, r.accel_y, r.accel_z,
                );
            }
            let a = self.ahrs.attitude();
            sum_r += a.roll;
            sum_p += a.pitch;
            cortex_m::asm::delay(240_000);
        }

        let n = trim_samples as f32;
        self.level_trim = [sum_r / n, sum_p / n];
    }

    /// Attitude with level trim removed — use this as the control measurement.
    pub fn attitude_trimmed(&self) -> Attitude {
        let a = self.ahrs.attitude();
        Attitude {
            roll: a.roll - self.level_trim[0],
            pitch: a.pitch - self.level_trim[1],
            yaw: a.yaw,
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
        output_limit: 0.3,
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

pub fn default_position_config() -> PositionConfig {
    PositionConfig {
        kp_xy: 1.0,
        kd_xy: 2.0,
        kp_z: 0.10,
        kd_z: 0.05,
        max_tilt_deg: 15.0,
        hover_throttle: 0.5,
        arrive_radius_m: 0.5,
        arrive_speed_mps: 0.3,
    }
}
