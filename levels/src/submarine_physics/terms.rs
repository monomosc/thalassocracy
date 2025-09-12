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
