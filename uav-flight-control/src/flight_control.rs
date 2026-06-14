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
//!     - Reads latest telemetry via the shared snapshot
//!     - Sets desired angles and throttle via shared commands
//!     - Handles arming

use stm32h7xx_hal::{pac::TIM2, timer::Timer};

use crate::{
    board::{self},
    drivers::{
        ahrs::{Attitude, MadgwickFilter},
        lowpass::GyroLowPass,
        motors::Motors,
        pid::{FlightPidOutput, FlightPids, PidConfig, PidGains},
    },
};

/// Rate-loop gyro low-pass cutoff. Below the dominant prop/frame vibration
/// band, above the airframe's real control dynamics. Lower if duties still
/// twitch at hover; raise if the craft feels sluggish or sloppy.
const GYRO_LPF_HZ: f32 = 80.0;
const RATE_LOOP_HZ: f32 = 1000.0;

/// Telemetry snapshot
#[derive(Clone, Copy, Default)]
pub struct Telemetry {
    pub attitude: Attitude,
    pub gyro: [f32; 3],
    pub accl: [f32; 3],
    pub pid_output: FlightPidOutput,
    pub motor_duties: [u16; 4],
    pub throttle: f32,
    pub tick: u32,
    pub armed: bool,
}

/// Commands from main loop to ISR
#[derive(Clone, Copy, Default)]
pub struct Commands {
    pub desired_angles: [f32; 3],
    pub base_throttle: f32,
    pub arm_request: bool,
    /// Monotonic microsecond timestamp of the last valid RC frame. The flight
    /// loop uses this for the link-loss failsafe.
    pub last_rc_us: u64,
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
    pub gyro_lpf: GyroLowPass,
    pub level_trim: [f32; 2],
    pub rate_pids: FlightPids,
    pub angle_pids: FlightPids,
    pub motors: Motors,

    // State
    pub desired_rates: [f32; 3],
    pub base_throttle: f32,
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
            gyro_lpf: GyroLowPass::new(GYRO_LPF_HZ, RATE_LOOP_HZ),
            level_trim: [0.0; 2],
            rate_pids: FlightPids::new(default_rate_config(), default_rate_yaw_config()),
            angle_pids: FlightPids::new(default_angle_config(), default_angle_yaw_config()),
            motors: Motors::new(max_duty),
            desired_rates: [0.0; 3],
            base_throttle: 0.0,
            tick_count: 0,
        }
    }

    /// Run the AHRS to convergence on a level, stationary craft, then capture
    /// the residual roll/pitch as a level-trim offset. Call once after the gyro
    /// is calibrated and before flight. ~1 ms per iteration at 240 MHz.
    pub fn calibrate_level(&mut self, settle_iters: u32, trim_samples: u32) {
        for _ in 0..settle_iters {
            if let Ok(r) = self.imu.read_scaled() {
                self.ahrs.update(
                    r.gyro_x, r.gyro_y, r.gyro_z, r.accel_x, r.accel_y, r.accel_z,
                );
            }
            cortex_m::asm::delay(240_000);
        }

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

/// PID configuration defaults — tune these with motors running.
/// Note: rate-loop kd starts at 0.0 for roll/pitch. A nonzero derivative on a
/// vibration-contaminated gyro signal injects motor noise directly. Reintroduce
/// kd only after the craft hovers cleanly and the gyro is well filtered.
pub fn default_rate_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 0.08,
            ki: 0.05,
            kd: 0.001,
        },
        integral_limit: 0.3,
        output_limit: 1.0,
    }
}

pub fn default_rate_yaw_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 0.12,
            ki: 0.03,
            kd: 0.0,
        },
        integral_limit: 0.3,
        output_limit: 1.0,
    }
}

pub fn default_angle_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 3.0,
            ki: 0.0,
            kd: 0.0,
        },
        integral_limit: 0.0,
        output_limit: 200.0,
    }
}

pub fn default_angle_yaw_config() -> PidConfig {
    PidConfig {
        gains: PidGains {
            kp: 2.0,
            ki: 0.0,
            kd: 0.0,
        },
        integral_limit: 0.0,
        output_limit: 200.0,
    }
}
