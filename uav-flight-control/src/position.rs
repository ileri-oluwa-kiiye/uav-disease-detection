//! Position controller — the outer-outer loop. Turns a position target into a
//! desired tilt (fed to the angle PID) plus a base throttle. Runs at the 250Hz
//! outer-loop rate. Frame: local ENU, x=east, y=north, z=up, SI units.

use micromath::F32Ext;

use crate::math::clamp;

const G: f32 = 9.81;
const RAD_TO_DEG: f32 = 180.0 / core::f32::consts::PI;
const DEG_TO_RAD: f32 = core::f32::consts::PI / 180.0;

// Sign/axis conventions must match your AHRS yaw zero and motor mixing.
// VERIFY ON BENCH before flying: command a small +north delta and confirm the
// nose pitches the right way, etc. Flip these if a channel is inverted.
const PITCH_SIGN: f32 = -1.0; // forward accel -> nose-down pitch
const ROLL_SIGN: f32 = 1.0;

/// Position + velocity estimate (ENU). Placeholder: holds whatever was last
/// `set()`. Wire GPS/baro/flow into `set()` to close the loop.
#[derive(Clone, Copy, Default)]
pub struct PositionEstimate {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}

impl PositionEstimate {
    pub fn set(&mut self, pos: [f32; 3], vel: [f32; 3]) {
        self.pos = pos;
        self.vel = vel;
    }
}

#[derive(Clone, Copy)]
pub struct PositionConfig {
    pub kp_xy: f32,
    pub kd_xy: f32,
    pub kp_z: f32,
    pub kd_z: f32,
    pub max_tilt_deg: f32,
    pub hover_throttle: f32,
    pub arrive_radius_m: f32,
    pub arrive_speed_mps: f32,
}

#[derive(Clone, Copy, Default)]
pub struct PositionOutput {
    pub roll: f32,     // desired roll angle (deg)
    pub pitch: f32,    // desired pitch angle (deg)
    pub throttle: f32, // base throttle [0,1]
    pub arrived: bool,
}

pub struct PositionController {
    cfg: PositionConfig,
}

impl PositionController {
    pub fn new(cfg: PositionConfig) -> Self {
        Self { cfg }
    }

    /// `yaw_deg` is the current heading from the AHRS.
    pub fn update(&self, target: [f32; 3], est: &PositionEstimate, yaw_deg: f32) -> PositionOutput {
        let c = &self.cfg;

        let ex = target[0] - est.pos[0];
        let ey = target[1] - est.pos[1];
        let h_err = (ex * ex + ey * ey).sqrt();
        let h_speed = (est.vel[0] * est.vel[0] + est.vel[1] * est.vel[1]).sqrt();
        let arrived = h_err < c.arrive_radius_m && h_speed < c.arrive_speed_mps;

        // Altitude hold around hover throttle (PD).
        let ez = target[2] - est.pos[2];
        let throttle = clamp(
            c.hover_throttle + c.kp_z * ez - c.kd_z * est.vel[2],
            0.0,
            1.0,
        );

        if arrived {
            // Stop and settle: level attitude, hold altitude.
            return PositionOutput {
                roll: 0.0,
                pitch: 0.0,
                throttle,
                arrived: true,
            };
        }

        // PD on horizontal position -> desired world accel (ENU).
        let ax = c.kp_xy * ex - c.kd_xy * est.vel[0];
        let ay = c.kp_xy * ey - c.kd_xy * est.vel[1];

        // Small-angle: world accel -> world tilt (rad).
        let tilt_e = ax / G;
        let tilt_n = ay / G;

        // Rotate world tilt into body frame. yaw measured E->N, CCW positive;
        // body-forward = (cos y, sin y) in (E,N).
        let y = yaw_deg * DEG_TO_RAD;
        let (s, cy) = y.sin_cos();
        let fwd = cy * tilt_e + s * tilt_n; // along body forward
        let right = s * tilt_e - cy * tilt_n; // along body right

        let m = c.max_tilt_deg;
        PositionOutput {
            pitch: clamp(PITCH_SIGN * fwd * RAD_TO_DEG, -m, m),
            roll: clamp(ROLL_SIGN * right * RAD_TO_DEG, -m, m),
            throttle,
            arrived: false,
        }
    }
}
