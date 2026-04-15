//! Motor PWM output driver for quadcopter ESCs
//!
//! Uses TIM3 with 4 channels for standard PWM ESCs (1000-2000µs pulse at 50Hz)
//!
//! Pin mapping (AF2):
//! - Motor 1 (front-left):  TIM3_CH1 = PA6
//! - Motor 2 (front-right): TIM3_CH2 = PA7
//! - Motor 3 (rear-left):   TIM3_CH3 = PB0
//! - Motor 4 (rear-right):  TIM3_CH4 = PB1

use simple_ternary::tnr;

use crate::math;

/// ESC pulse width limits in microseconds
const ESC_MIN_US: f32 = 1000.0; // idle / off
const ESC_MAX_US: f32 = 2000.0; // full throttle

/// PWM period in microseconds (50Hz = 20000µs)
const PWM_PERIOD_US: f32 = 20000.0;

/// Motor index constants
pub const MOTOR_FL: usize = 0;
pub const MOTOR_FR: usize = 1;
pub const MOTOR_RL: usize = 2;
pub const MOTOR_RR: usize = 3;

/// Motor output state
pub struct Motors {
    /// Current throttle values [0.0 = idle, 1.0 = full] for each motor
    throttle: [f32; 4],
    /// Whether motors are armed (will actually output above idle)
    armed: bool,
    /// Maximum duty count corresponding to the timer's auto-reload value
    max_duty: u16,
}

impl Motors {
    /// Create new motor output handler.
    /// `max_duty` is the timer's auto-reload value (determines PWM resolution)
    pub fn new(max_duty: u16) -> Self {
        Self {
            throttle: [0.0; 4],
            armed: false,
            max_duty,
        }
    }

    /// Arm the motors — allows throttle commands above idle
    pub fn arm(&mut self) {
        self.armed = true;
    }

    /// Disarm the motors — all outputs go to idle
    pub fn disarm(&mut self) {
        self.armed = false;
        self.throttle = [0.0; 4];
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Set throttle for a single motor [0.0 - 1.0]
    #[inline(always)]
    pub fn set_throttle<const MOTOR: usize>(&mut self, value: f32) {
        self.throttle[MOTOR] = math::clamp(value, 0.0, 1.0);
    }

    /// Set throttle for all 4 motors at once
    #[inline(always)]
    pub fn set_all(&mut self, val: [f32; 4]) {
        self.set_throttle::<MOTOR_FL>(val[MOTOR_FL]);
        self.set_throttle::<MOTOR_FR>(val[MOTOR_FR]);
        self.set_throttle::<MOTOR_RL>(val[MOTOR_RL]);
        self.set_throttle::<MOTOR_RR>(val[MOTOR_RR]);
    }

    /// Apply mixer output from PID controller.
    ///
    /// - `base_throttle`: overall throttle (0.0 - 1.0) (from RC or altitude hold)
    /// - `roll`: roll correction from PID (positive = increase right motors)
    /// - `pitch`: pitch correction from PID (positive = increase rear motors)
    /// - `yaw`: yaw correction from PID (positive = increase CW motors)
    ///
    /// Quadcopter "X" configuration:
    /// ```text
    ///     Front
    ///  FL(1)  FR(2)
    ///     \  /
    ///      \/
    ///      /\
    ///     /  \
    ///  RL(3)  RR(4)
    ///      Rear
    ///
    ///  FL & RR spin clockwise (CW)
    ///  FR & RL spin counter-clockwise (CCW)
    /// ```
    pub fn mix(&mut self, base_throttle: f32, roll: f32, pitch: f32, yaw: f32) {
        if !self.armed {
            self.throttle = [0.0; 4];
            return;
        }

        let t = math::clamp(base_throttle, 0.0, 1.0);

        // X-configuration mixing
        // FL (CW):  +roll, +pitch, +yaw
        // FR (CCW): -roll, +pitch, -yaw
        // RL (CCW): +roll, -pitch, -yaw
        // RR (CW):  -roll, -pitch, +yaw
        self.set_throttle::<MOTOR_FL>(t + roll + pitch + yaw);
        self.set_throttle::<MOTOR_FR>(t - roll + pitch - yaw);
        self.set_throttle::<MOTOR_RL>(t + roll - pitch - yaw);
        self.set_throttle::<MOTOR_RR>(t - roll - pitch + yaw);
    }

    /// Get the duty cycle count for a motor (to write to CCR register)
    /// Maps throttle [0.0 - 1.0] to ESC pulse width [1000 - 2000µs]
    /// within a 20000µs (50Hz) period
    pub fn duty<const MOTOR: usize>(&self) -> u16 {
        // Map [0.0, 1.0] to [1000µs, 2000µs]
        let throttle = tnr! {self.armed => self.throttle[MOTOR] : 0.0};
        let pulse_us = ESC_MIN_US + throttle * (ESC_MAX_US - ESC_MIN_US);

        // Convert µs to duty count
        // duty = (pulse_us / period_us) * max_duty
        let duty = (pulse_us / PWM_PERIOD_US) * (self.max_duty as f32);
        duty as u16
    }

    /// Get duty values for all 4 motors
    pub fn duties(&self) -> [u16; 4] {
        [
            self.duty::<MOTOR_FL>(),
            self.duty::<MOTOR_FR>(),
            self.duty::<MOTOR_RL>(),
            self.duty::<MOTOR_RR>(),
        ]
    }

    /// Get throttle value for a specific motor
    pub fn throttle<const MOTOR: usize>(&self) -> f32 {
        self.throttle[MOTOR]
    }

    /// Get current throttle values
    pub fn throttles(&self) -> [f32; 4] {
        self.throttle
    }
}
