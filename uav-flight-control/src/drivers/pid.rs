//! PID Controller — single-axis, generic, reusable
//!
//! Designed for flight control with:
//! - Configurable P, I, D gains
//! - Integral windup protection (clamping)
//! - Derivative on measurement (not error) to avoid derivative kick
//! - Output clamping
//! - Integral term reset

use crate::math;

/// PID tuning parameters
#[derive(Debug, Clone, Copy)]
pub struct PidGains {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
}

/// PID configuration
#[derive(Debug, Clone, Copy)]
pub struct PidConfig {
    pub gains: PidGains,
    /// Maximum absolute value the integral term can accumulate
    pub integral_limit: f32,
    /// Maximum absolute value of the output
    pub output_limit: f32,
}

/// Single-axis PID controller
pub struct Pid {
    config: PidConfig,
    /// Accumulated integral term
    integral: f32,
    /// Previous measurement (for derivative-on-measurement)
    prev_measurement: f32,
    /// Whether we've received at least one measurement
    initialized: bool,
}

/// PID computation output with individual term visibility
#[derive(Debug, Clone, Copy, Default)]
pub struct PidOutput {
    /// Total output (P + I + D), clamped to output_limit
    pub output: f32,
    /// Proportional contribution
    pub p: f32,
    /// Integral contribution
    pub i: f32,
    /// Derivative contribution
    pub d: f32,
}

impl Pid {
    /// Create a new PID controller
    pub fn new(config: PidConfig) -> Self {
        Self {
            config,
            integral: 0.0,
            prev_measurement: 0.0,
            initialized: false,
        }
    }

    /// Update the PID controller.
    ///
    /// - `setpoint`: desired value (e.g. desired rate in dps, or desired angle in degrees)
    /// - `measurement`: current measured value
    /// - `dt`: time step in seconds (e.g. 0.001 for 1kHz)
    ///
    /// Returns the control output.
    pub fn update(&mut self, setpoint: f32, measurement: f32, dt: f32) -> PidOutput {
        let error = setpoint - measurement;

        // Proportional
        let p = self.config.gains.kp * error;

        // Integral
        self.integral += error * dt;
        // Clamp integral to prevent windup
        self.integral = math::clamp(
            self.integral,
            -self.config.integral_limit,
            self.config.integral_limit,
        );
        let i = self.config.gains.ki * self.integral;

        // Derivative (on measurement, not error)
        // Using derivative on measurement avoids the "derivative kick" that occurs
        // when the setpoint changes suddenly. The derivative of the error would spike,
        // but the derivative of the measurement changes smoothly.
        let d = if self.initialized {
            let d_measurement = (measurement - self.prev_measurement) / dt;
            -self.config.gains.kd * d_measurement // negative because we want to oppose change
        } else {
            self.initialized = true;
            0.0
        };
        self.prev_measurement = measurement;

        // Total output
        let output = math::clamp(
            p + i + d,
            -self.config.output_limit,
            self.config.output_limit,
        );

        PidOutput { output, p, i, d }
    }

    /// Reset the integral accumulator and derivative state
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_measurement = 0.0;
        self.initialized = false;
    }

    /// Update gains at runtime (for tuning)
    pub fn set_gains(&mut self, gains: PidGains) {
        self.config.gains = gains;
    }

    /// Update integral limit
    pub fn set_integral_limit(&mut self, limit: f32) {
        self.config.integral_limit = limit;
    }

    /// Update output limit
    pub fn set_output_limit(&mut self, limit: f32) {
        self.config.output_limit = limit;
    }

    /// Get current integral value (useful for debugging)
    pub fn integral(&self) -> f32 {
        self.integral
    }
}

/// Axis set of three PID controllers for roll, pitch, yaw
pub struct FlightPids {
    pub roll: Pid,
    pub pitch: Pid,
    pub yaw: Pid,
}

/// Three-axis PID output
#[derive(Debug, Clone, Copy, Default)]
pub struct FlightPidOutput {
    pub roll: PidOutput,
    pub pitch: PidOutput,
    pub yaw: PidOutput,
}

impl FlightPids {
    /// Create a new set of flight PIDs with the same config for roll and pitch,
    /// and a separate config for yaw (which typically has different gains).
    pub fn new(roll_pitch_config: PidConfig, yaw_config: PidConfig) -> Self {
        Self {
            roll: Pid::new(roll_pitch_config),
            pitch: Pid::new(roll_pitch_config),
            yaw: Pid::new(yaw_config),
        }
    }

    /// Update all three axes.
    ///
    /// - `setpoints`: desired (roll_rate, pitch_rate, yaw_rate) in dps
    /// - `measurements`: actual (roll_rate, pitch_rate, yaw_rate) in dps
    /// - `dt`: time step in seconds
    pub fn update(
        &mut self,
        setpoints: [f32; 3],
        measurements: [f32; 3],
        dt: f32,
    ) -> FlightPidOutput {
        FlightPidOutput {
            roll: self.roll.update(setpoints[0], measurements[0], dt),
            pitch: self.pitch.update(setpoints[1], measurements[1], dt),
            yaw: self.yaw.update(setpoints[2], measurements[2], dt),
        }
    }

    /// Reset all axes
    pub fn reset(&mut self) {
        self.roll.reset();
        self.pitch.reset();
        self.yaw.reset();
    }
}
