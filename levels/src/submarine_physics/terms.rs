use crate::{Quatf, SubPhysicsSpec, SubState, Vec3f};
use super::util::quat_rotate_vec3;

// ----- Yaw torques -----
pub(super) fn torque_yaw_control(
    spec: &SubPhysicsSpec,
    yaw_in: f32,
    sign_u: f32,
    front_mount_gain: f32,
    q_dyn: f32,
) -> f32 {
    // Positive yaw input = right rudder; produce negative yaw torque under forward flow.
    -yaw_in * sign_u * front_mount_gain * spec.n_delta_r * q_dyn * spec.s_side * spec.length
}

pub(super) fn torque_yaw_damping_linear(spec: &SubPhysicsSpec, r_yaw: f32) -> f32 {
    -spec.kr * r_yaw
}

pub(super) fn torque_yaw_damping_quadratic(spec: &SubPhysicsSpec, r_yaw: f32) -> f32 {
    -spec.kr2 * r_yaw.abs() * r_yaw
}

pub(super) fn torque_yaw_damping_dynamic(spec: &SubPhysicsSpec, q_dyn: f32, r_yaw: f32) -> f32 {
    -spec.nr_v * q_dyn * r_yaw
}

pub(super) fn torque_sideslip_ws(spec: &SubPhysicsSpec, rho: f32, w_cpl: f32) -> f32 {
    // Apply corrective yaw torque to reduce lateral slip (weathervane toward airflow).
    // Use quadratic scaling in lateral slip (w * |w|) to soften small slips and avoid overpowering control.
    let q_lat = 0.5 * rho * spec.s_side * spec.length;
    spec.n_ws * (w_cpl * w_cpl.abs()) * q_lat
}

pub(super) fn torque_weathervane_beta(spec: &SubPhysicsSpec, q_dyn: f32, yaw_err: f32) -> f32 {
    spec.n_beta * q_dyn * spec.s_side * spec.length * yaw_err
}

// ----- Pitch / Roll torques from ballast and COB -----

pub(super) fn torque_from_ballast_gravity_about_axis(
    spec: &SubPhysicsSpec,
    state: &SubState,
    cg_body_current: Vec3f,
    orientation: Quatf,
    axis_world: Vec3f,
    g: f32,
) -> f32 {
    let mut tau = 0.0_f32;
    for (i, tank) in spec.ballast_tanks.iter().enumerate() {
        let cap = tank.capacity_kg.max(0.0);
        let fill = state.ballast_fill.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let m = cap * fill;
        if m <= 0.0 { continue; }

        let r_body = tank.pos_body - cg_body_current;
        let r_world = quat_rotate_vec3(orientation, r_body);
        let f_world = Vec3f::new(0.0, -m * g, 0.0);
        let moment = Vec3f::new(
            r_world.y * f_world.z - r_world.z * f_world.y,
            r_world.z * f_world.x - r_world.x * f_world.z,
            r_world.x * f_world.y - r_world.y * f_world.x,
        );
        tau += moment.x * axis_world.x + moment.y * axis_world.y + moment.z * axis_world.z;
    }
    tau
}

pub(super) fn torque_from_cob_buoyancy_about_axis(
    spec: &SubPhysicsSpec,
    orientation: Quatf,
    axis_world: Vec3f,
    buoyancy: f32,
) -> f32 {
    let r_cb_world = quat_rotate_vec3(orientation, spec.cb_offset_body);
    let buoy_force = Vec3f::new(0.0, buoyancy, 0.0);
    let moment_cb = Vec3f::new(
        r_cb_world.y * buoy_force.z - r_cb_world.z * buoy_force.y,
        r_cb_world.z * buoy_force.x - r_cb_world.x * buoy_force.z,
        r_cb_world.x * buoy_force.y - r_cb_world.y * buoy_force.x,
    );
    moment_cb.x * axis_world.x + moment_cb.y * axis_world.y + moment_cb.z * axis_world.z
}

// ----- Linear damping on pitch/roll -----

pub(super) fn torque_pitch_linear_damping(spec: &SubPhysicsSpec, omega_x: f32) -> f32 {
    -spec.kq * omega_x
}

pub(super) fn torque_roll_linear_damping(spec: &SubPhysicsSpec, omega_z: f32) -> f32 {
    -spec.kp * omega_z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vec3f, Quatf};
    use crate::subspecs::small_skiff_spec;
    use crate::SubState;

    fn make_state_with_fill(fill: &[f32]) -> SubState {
        SubState {
            position: Vec3f::new(0.0, 0.0, 0.0),
            velocity: Vec3f::new(0.0, 0.0, 0.0),
            orientation: Quatf::from_rotation_y(0.0),
            ang_mom: Vec3f::new(0.0, 0.0, 0.0),
            ballast_fill: fill.to_vec(),
        }
    }

    #[test]
    fn yaw_control_torque_scales_and_signs() {
        let mut spec = small_skiff_spec();
        // Make the geometric terms simple
        spec.s_side = 2.0;
        spec.length = 3.0;
        spec.n_delta_r = 0.5;

        let yaw_in = 0.4; // right rudder
        let q_dyn = 10.0;

        // Forward motion (sign_u=+1), no special front mount gain
        let tau_fwd = torque_yaw_control(&spec, yaw_in, 1.0, 1.0, q_dyn);
        let expected_fwd = -yaw_in * 1.0 * 1.0 * spec.n_delta_r * q_dyn * spec.s_side * spec.length;
        assert!((tau_fwd - expected_fwd).abs() < 1e-6);

        // Reverse motion (sign_u=-1), double mount gain
        let tau_rev = torque_yaw_control(&spec, yaw_in, -1.0, 2.0, q_dyn);
        let expected_rev = -yaw_in * -1.0 * 2.0 * spec.n_delta_r * q_dyn * spec.s_side * spec.length;
        assert!((tau_rev - expected_rev).abs() < 1e-6);
        // Opposite sign from forward case, larger magnitude
        assert!(tau_fwd.signum() != tau_rev.signum());
        assert!(tau_rev.abs() > tau_fwd.abs());
    }

    #[test]
    fn yaw_damping_terms() {
        let mut spec = small_skiff_spec();
        spec.kr = 3.0;
        spec.kr2 = 7.0;
        spec.nr_v = 0.2;
        let r = -0.6;
        let q_dyn = 12.0;

        let lin = torque_yaw_damping_linear(&spec, r);
        let quad = torque_yaw_damping_quadratic(&spec, r);
        let dynm = torque_yaw_damping_dynamic(&spec, q_dyn, r);

        assert!((lin - (-spec.kr * r)).abs() < 1e-6);
        assert!((quad - (-spec.kr2 * r.abs() * r)).abs() < 1e-6);
        assert!((dynm - (-spec.nr_v * q_dyn * r)).abs() < 1e-6);
    }

    #[test]
    fn sideslip_quadratic_ws_term() {
        let mut spec = small_skiff_spec();
        spec.s_side = 2.0;
        spec.length = 3.5;
        spec.n_ws = 0.08;
        let rho = 1025.0;
        let w = -0.7; // sway right negative in our convention here -> squared*sign gives negative

        let tau = torque_sideslip_ws(&spec, rho, w);
        let q_lat = 0.5 * rho * spec.s_side * spec.length;
        let expected = spec.n_ws * (w * w.abs()) * q_lat;
        assert!((tau - expected).abs() < 1e-6);

        // Symmetry: flipping sign should flip torque sign
        let tau_flip = torque_sideslip_ws(&spec, rho, -w);
        assert!((tau + tau_flip).abs() < 1e-6);
    }

    #[test]
    fn weathervane_beta_term() {
        let mut spec = small_skiff_spec();
        spec.s_side = 1.0;
        spec.length = 2.0;
        spec.n_beta = 0.03;
        let q_dyn = 15.0;
        let yaw_err = -0.2;

        let tau = torque_weathervane_beta(&spec, q_dyn, yaw_err);
        let expected = spec.n_beta * q_dyn * spec.s_side * spec.length * yaw_err;
        assert!((tau - expected).abs() < 1e-6);
    }

    #[test]
    fn ballast_gravity_torque_about_axis() {
        let spec = {
            let mut s = small_skiff_spec();
            // single tank located at +Z (nose)
            s.ballast_tanks = vec![crate::BallastTankSpec { pos_body: Vec3f::new(0.0, 0.0, 1.0), capacity_kg: 10.0 }];
            s
        };
        // Full fill produces m = 10 kg
        let state = make_state_with_fill(&[1.0]);
        let cg = Vec3f::new(0.0, 0.0, 0.0);
        let axis_right = Vec3f::new(1.0, 0.0, 0.0);
        let g = 9.81;

        // Expect tau_x = r_z * m * g with r=(0,0,1)
        let tau = torque_from_ballast_gravity_about_axis(&spec, &state, cg, state.orientation, axis_right, g);
        let expected = 1.0 * 10.0 * g;
        assert!((tau - expected).abs() < 1e-4, "tau={}, expected={}", tau, expected);
    }

    #[test]
    fn cob_buoyancy_torque_about_axis() {
        let mut spec = small_skiff_spec();
        // Put center of buoyancy forward of COM by 0.5 m
        spec.cb_offset_body = Vec3f::new(0.0, 0.0, 0.5);
        let buoy_n = 200.0; // upward N
        let axis_right = Vec3f::new(1.0, 0.0, 0.0);
        let tau = torque_from_cob_buoyancy_about_axis(&spec, Quatf::from_rotation_y(0.0), axis_right, buoy_n);
        // r × F with r=(0,0,0.5), F=(0,200,0) gives (-100, 0, 0); dot with +X = -100
        assert!((tau + 100.0).abs() < 1e-4, "tau={}, expected=-100", tau);
    }

    #[test]
    fn pitch_roll_linear_damping() {
        let mut spec = small_skiff_spec();
        spec.kq = 12.0;
        spec.kp = 9.0;
        assert!((torque_pitch_linear_damping(&spec, 0.3) + 3.6).abs() < 1e-6);
        assert!((torque_roll_linear_damping(&spec, -0.5) - 4.5).abs() < 1e-6);
    }
}
