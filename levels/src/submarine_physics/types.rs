use crate::{Quatf, Vec3f};

#[derive(Debug, Clone, Copy, Default)]
pub struct SubStepDebug {
    pub dt: f32,
    pub time: f32,
    pub inputs: SubInputs,
    // Orientation basis (world XZ plane)
    pub forward: Vec3f,
    pub right: Vec3f,
    pub up_b: Vec3f,
    // Flow and relative velocity
    pub flow: Vec3f,
    pub rel: Vec3f,
    pub u: f32, // surge (forward component of rel)
    pub v: f32, // heave (vertical rel approx)
    pub w: f32, // sway (right component of rel)
    pub q_dyn: f32,
    pub sign_u: f32,
    pub front_mount_gain: f32,
    // Forces (body components) and world recompose
    pub thrust_force: f32,
    pub fx: f32,
    pub fy: f32,
    pub fz: f32,
    pub f_world: Vec3f,
    pub f_rudder_lat: f32,
    // Yaw torques (breakdown)
    pub tau_control: f32,
    pub tau_damp_lin: f32,
    pub tau_damp_quad: f32,
    pub tau_damp_dyn: f32,
    pub tau_ws: f32,
    pub tau_beta: f32,
    pub tau_total: f32,
    pub yaw_err: f32,
    pub yaw_acc: f32,
    pub yaw_rate: f32,
    pub heading_yaw: f32,
    // Ballast & buoyancy diagnostics
    pub fill_fwd: f32,
    pub fill_aft: f32,
    pub mass_eff: f32,
    pub buoyancy_n: f32,
    pub weight_n: f32,
    pub buoy_net_n: f32,
    // Pitch diagnostics
    pub tau_pitch: f32,
}

#[derive(Debug, Clone)]
pub struct SubState {
    pub position: Vec3f,
    pub velocity: Vec3f,
    /// Orientation as quaternion (body→world).
    /// Frame conventions (see design/COORDINATES_AND_CONVENTIONS.md):
    /// - Body axes: +Z forward, +Y up, +X right (starboard).
    /// - World axes: +Z forward, +Y up, +X right.
    /// - Positive yaw rate (ω_y) turns the nose to the left (CCW when looking down +Y).
    pub orientation: Quatf,
    /// Angular momentum in body frame (kg·m²·rad/s). Use spec inertia to derive ω.
    /// Convention: body axes are +Z forward, +Y up, +X right.
    pub ang_mom: Vec3f,
    /// Ballast tank fill state in [0,1] for each tank in spec.ballast_tanks (future use)
    pub ballast_fill: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubInputs {
    pub thrust: f32, // -1..1 (forward/back)
    /// Rudder input in [-1, 1].
    /// Convention: +1 = right rudder (nose yaws right when moving forward),
    /// -1 = left rudder. The physics maps this to yaw torque so that forward
    /// motion with positive input decreases heading_yaw (right turn).
    pub yaw: f32, // -1..1 (right rudder positive)
    /// Forward ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_fwd: f32,
    /// Aft ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_aft: f32,
}
