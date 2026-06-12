//! One-pole IIR low-pass filter for rate-loop gyro conditioning.
//!
//! Discrete form: y[n] = y[n-1] + alpha * (x[n] - y[n-1])
//! where alpha = dt / (rc + dt), rc = 1 / (2*pi*fc).
//!
//! This sits between the IMU and the rate PID. It removes vibration-band noise
//! that would otherwise be amplified by the derivative term and injected into
//! motor duty. Keep the cutoff well above the airframe's control bandwidth
//! (~10-30 Hz of real dynamics) but below the dominant vibration frequencies.

use core::f32::consts::PI;

/// Single-axis one-pole low-pass.
#[derive(Clone, Copy)]
pub struct LowPass {
    alpha: f32,
    state: f32,
    initialized: bool,
}

impl LowPass {
    /// Create a filter for cutoff `fc_hz` sampled at `sample_hz`.
    pub fn new(fc_hz: f32, sample_hz: f32) -> Self {
        Self {
            alpha: Self::alpha(fc_hz, sample_hz),
            state: 0.0,
            initialized: false,
        }
    }

    fn alpha(fc_hz: f32, sample_hz: f32) -> f32 {
        let dt = 1.0 / sample_hz;
        let rc = 1.0 / (2.0 * PI * fc_hz);
        dt / (rc + dt)
    }

    /// Retune the cutoff at runtime (e.g. live tuning over telemetry).
    pub fn set_cutoff(&mut self, fc_hz: f32, sample_hz: f32) {
        self.alpha = Self::alpha(fc_hz, sample_hz);
    }

    /// Push a sample, get the filtered value. First sample seeds the state so
    /// the output doesn't ramp from zero.
    pub fn update(&mut self, x: f32) -> f32 {
        if !self.initialized {
            self.state = x;
            self.initialized = true;
        } else {
            self.state += self.alpha * (x - self.state);
        }
        self.state
    }

    /// Reset to uninitialized — next sample reseeds.
    pub fn reset(&mut self) {
        self.state = 0.0;
        self.initialized = false;
    }
}

/// Three-axis bundle for roll/pitch/yaw gyro.
#[derive(Clone, Copy)]
pub struct GyroLowPass {
    axes: [LowPass; 3],
}

impl GyroLowPass {
    pub fn new(fc_hz: f32, sample_hz: f32) -> Self {
        Self {
            axes: [LowPass::new(fc_hz, sample_hz); 3],
        }
    }

    pub fn update(&mut self, g: [f32; 3]) -> [f32; 3] {
        [
            self.axes[0].update(g[0]),
            self.axes[1].update(g[1]),
            self.axes[2].update(g[2]),
        ]
    }

    pub fn set_cutoff(&mut self, fc_hz: f32, sample_hz: f32) {
        for ax in &mut self.axes {
            ax.set_cutoff(fc_hz, sample_hz);
        }
    }

    pub fn reset(&mut self) {
        for ax in &mut self.axes {
            ax.reset();
        }
    }
}
