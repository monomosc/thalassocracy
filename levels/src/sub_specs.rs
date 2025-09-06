use serde::{Deserialize, Serialize};
use crate::math::Vec3f;

/// Precomputed physics parameters for a specific submarine hull class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubPhysicsSpec {
    pub m: f32,
    pub ixx: f32,
    pub iyy: f32,
    pub izz: f32,
    pub cxd: f32,
    pub cyd: f32,
    pub czd: f32,
    pub xu: f32,
    pub yv: f32,
    pub zw: f32,
    pub kr: f32,
    pub kr2: f32,
    pub kq: f32,
    pub nr_v: f32,
    pub volume_m3: f32,
    pub t_max: f32,
    pub tau_thr: f32,
    pub n_delta_r: f32,
    pub n_beta: f32,
    pub m_delta_b: f32,
    pub delta_r_max: f32,
    pub delta_b_max: f32,
    pub length: f32,
    pub diameter: f32,
    pub s_forward: f32,
    pub s_side: f32,
    pub s_top: f32,
    pub ballast_tanks: Vec<BallastTankSpec>,
    pub n_ws: f32,
    pub y_delta_r: f32,
    /// Center of buoyancy offset from center of mass in body space (meters).
    /// Positive Y means COB above COM, creating a restoring torque toward level.
    pub cb_offset_body: Vec3f,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BallastTankSpec {
    pub pos_body: Vec3f,
    pub capacity_kg: f32,
}

pub mod subspecs {
    use super::*;

    // Sensible defaults for a small 1‑person submersible (prototype scale, SI units)
    pub fn small_skiff_spec() -> SubPhysicsSpec {
        // Geometry estimates
        let length = 3.0; // meters
        let diameter = 1.0; // meters
        let radius = diameter * 0.5;
        let s_forward = std::f32::consts::PI * radius * radius; // frontal area
        let s_side = length * diameter; // side area (approx)
        let s_top = length * diameter; // top/bottom area (approx)

        // Mass & inertia (approx cylinder)
        let m = 1200.0; // kg
        let ixx = 0.5 * m * radius * radius; // roll
        let iyy = (1.0 / 12.0) * m * (3.0 * radius * radius + length * length); // pitch
        let izz = iyy; // yaw ~ pitch

        SubPhysicsSpec {
            m,
            ixx,
            iyy,
            izz,
            // Quadratic drag coefficients (dimensionless, tuned)
            cxd: 0.35,
            cyd: 3.0,
            czd: 1.2,
            // Small linear damping (N·s/m) to help at very low speeds
            xu: 30.0,
            yv: 60.0,
            zw: 40.0,
            // Angular damping
            kr: 400.0,
            kr2: 120.0,
            kq: 200.0,
            nr_v: 0.02,
            volume_m3: std::f32::consts::PI * radius * radius * length,
            // Controls
            t_max: 1200.0, // N
            tau_thr: 2.5, // s
            // Rudder effectiveness
            n_delta_r: 0.006,
            // Weathervane effectiveness
            n_beta: 0.10,
            m_delta_b: 1200.0,
            delta_r_max: 1.0,
            delta_b_max: 1.0,
            // Geometry
            length,
            diameter,
            s_forward,
            s_side,
            s_top,
            ballast_tanks: vec![
                BallastTankSpec { pos_body: Vec3f::new( 0.9, 0.0, 0.0), capacity_kg: 80.0 }, // forward
                BallastTankSpec { pos_body: Vec3f::new(-0.9, 0.0, 0.0), capacity_kg: 80.0 }, // aft
            ],
            n_ws: 0.6,
            y_delta_r: 0.04,
            cb_offset_body: Vec3f::new(0.0, 0.12, 0.0),
        }
    }
}
