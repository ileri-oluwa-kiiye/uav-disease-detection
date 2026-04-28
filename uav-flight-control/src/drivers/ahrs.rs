//! Madgwick AHRS filter. Lightweight 6-axis IMU sensor fusion
//!
//! Fuses accelerometer and gyroscope data into a stable orientation estimate.
//! Based on: "An efficient orientation filter for inertial and inertial/magnetic
//! sensor arrays", Sebastian O.H. Madgwick, 2010

use core::f32::consts::PI;

use crate::math;

const DEG_TO_RAD: f32 = PI / 180.0;

/// Orientation as a unit quaternion [w, x, y, z]
#[derive(Debug, Clone, Copy)]
pub struct Quaternion {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quaternion {
    /// Normalize to unit length
    fn normalize(self) -> Self {
        let norm = math::fast_inv_sqrt(
            self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z,
        );
        Self {
            w: self.w * norm,
            x: self.x * norm,
            y: self.y * norm,
            z: self.z * norm,
        }
    }

    /// Convert quaternion to Euler angles (roll, pitch, yaw) in degrees
    pub fn to_euler(&self) -> Attitude {
        let (q0, q1, q2, q3) = (self.w, self.x, self.y, self.z);

        // Roll (rotation around X axis)
        let sinr_cosp = 2.0 * (q0 * q1 + q2 * q3);
        let cosr_cosp = 1.0 - 2.0 * (q1 * q1 + q2 * q2);
        let roll = math::atan2f(sinr_cosp, cosr_cosp);

        // Pitch (rotation around Y axis)
        let sinp = 2.0 * (q0 * q2 - q3 * q1);
        let pitch = if sinp.abs() >= 1.0 {
            core::f32::consts::FRAC_PI_2.copysign(sinp) // clamp to ±90°
        } else {
            math::asinf(sinp)
        };

        // Yaw (rotation around Z axis)
        let siny_cosp = 2.0 * (q0 * q3 + q1 * q2);
        let cosy_cosp = 1.0 - 2.0 * (q2 * q2 + q3 * q3);
        let yaw = math::atan2f(siny_cosp, cosy_cosp);

        Attitude {
            roll: roll * (180.0 / PI),
            pitch: pitch * (180.0 / PI),
            yaw: yaw * (180.0 / PI),
        }
    }
}

/// Euler angle attitude in degrees
#[derive(Debug, Clone, Copy, Default)]
pub struct Attitude {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

/// Madgwick filter for 6-axis IMU (accel + gyro)
pub struct MadgwickFilter {
    /// Current orientation estimate
    pub q: Quaternion,
    /// Filter gain. Controls how much the accelerometer corrects gyro drift.
    /// - Higher = more accel trust, noisier
    /// - Lower = more gyro trust, more drift.
    /// - Typical range: 0.01 (slow correction) to 0.5 (aggressive correction).
    beta: f32,
    /// Sample period in seconds (1/sample_rate)
    sample_period: f32,
}

impl MadgwickFilter {
    /// Create a new filter.
    ///
    /// - `sample_rate_hz`: how often `update()` is called (e.g. 1000.0 for 1kHz)
    /// - `beta`: filter gain (start with 0.033)
    pub fn new(sample_rate_hz: f32, beta: f32) -> Self {
        Self {
            q: Quaternion {
                w: 1.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            beta,
            sample_period: 1.0 / sample_rate_hz,
        }
    }

    /// Update the filter with new IMU data.
    ///
    /// - `gx, gy, gz`: gyroscope in **degrees/second** (converted internally to rad/s)
    /// - `ax, ay, az`: accelerometer in **g** (normalized internally)
    pub fn update(&mut self, gx: f32, gy: f32, gz: f32, ax: f32, ay: f32, az: f32) {
        let q = &self.q;
        let (q0, q1, q2, q3) = (q.w, q.x, q.y, q.z);

        // Convert gyro to rad/s
        let gx = gx * DEG_TO_RAD;
        let gy = gy * DEG_TO_RAD;
        let gz = gz * DEG_TO_RAD;

        // Rate of change of quaternion from gyroscope
        let q_dot_w = 0.5 * (-q1 * gx - q2 * gy - q3 * gz);
        let q_dot_x = 0.5 * (q0 * gx + q2 * gz - q3 * gy);
        let q_dot_y = 0.5 * (q0 * gy - q1 * gz + q3 * gx);
        let q_dot_z = 0.5 * (q0 * gz + q1 * gy - q2 * gx);

        // Compute feedback only if accelerometer measurement valid (avoid NaN)
        let (mut s0, mut s1, mut s2, mut s3) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);

        let a_norm_sq = ax * ax + ay * ay + az * az;
        if a_norm_sq > 0.0 {
            // Normalize accelerometer
            let recip_norm = math::fast_inv_sqrt(a_norm_sq);
            let ax = ax * recip_norm;
            let ay = ay * recip_norm;
            let az = az * recip_norm;

            // Auxiliary variables to avoid repeated arithmetic
            let _2q0 = 2.0 * q0;
            let _2q1 = 2.0 * q1;
            let _2q2 = 2.0 * q2;
            let _2q3 = 2.0 * q3;
            let _4q0 = 4.0 * q0;
            let _4q1 = 4.0 * q1;
            let _4q2 = 4.0 * q2;
            let _8q1 = 8.0 * q1;
            let _8q2 = 8.0 * q2;
            let q0q0 = q0 * q0;
            let q1q1 = q1 * q1;
            let q2q2 = q2 * q2;
            let q3q3 = q3 * q3;

            // Gradient descent corrective step
            // Objective function: rotate gravity [0,0,1] by quaternion, subtract measured accel
            s0 = _4q0 * q2q2 + _2q2 * ax + _4q0 * q1q1 - _2q1 * ay;
            s1 = _4q1 * q3q3 - _2q3 * ax + 4.0 * q0q0 * q1 - _2q0 * ay - _4q1
                + _8q1 * q1q1
                + _8q1 * q2q2
                + _4q1 * az;
            s2 = 4.0 * q0q0 * q2 + _2q0 * ax + _4q2 * q3q3 - _2q3 * ay - _4q2
                + _8q2 * q1q1
                + _8q2 * q2q2
                + _4q2 * az;
            s3 = 4.0 * q1q1 * q3 - _2q1 * ax + 4.0 * q2q2 * q3 - _2q2 * ay;

            // Normalize step
            let recip_norm = math::fast_inv_sqrt(s0 * s0 + s1 * s1 + s2 * s2 + s3 * s3);
            s0 *= recip_norm;
            s1 *= recip_norm;
            s2 *= recip_norm;
            s3 *= recip_norm;
        }

        // Apply feedback step
        let dt = self.sample_period;
        self.q = Quaternion {
            w: q0 + (q_dot_w - self.beta * s0) * dt,
            x: q1 + (q_dot_x - self.beta * s1) * dt,
            y: q2 + (q_dot_y - self.beta * s2) * dt,
            z: q3 + (q_dot_z - self.beta * s3) * dt,
        }
        .normalize();
    }

    /// Get current attitude as Euler angles in degrees
    pub fn attitude(&self) -> Attitude {
        self.q.to_euler()
    }

    /// Set the filter gain
    pub fn set_beta(&mut self, beta: f32) {
        self.beta = beta;
    }

    /// Set the sample rate
    pub fn set_sample_rate(&mut self, hz: f32) {
        self.sample_period = 1.0 / hz;
    }
}
