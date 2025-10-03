use super::flow::sample_flow_at;
use super::terms::*;
use super::types::{SubInputState, SubState, SubStepDebug};
use super::util::{
    quat_rotate_vec3, quat_to_yaw, vadd, vscale, vsub, BODY_FWD, BODY_RIGHT, BODY_UP,
};
use crate::{LevelSpec, Quatf, SubPhysicsSpec, Vec3f};

/// Simple submarine dynamics step honoring thrust and rudder in a flow field.
/// See `step_submarine_dbg` for full details and telemetry.
pub fn step_submarine(
    level: &LevelSpec,
    spec: &SubPhysicsSpec,
    inputs: SubInputState,
    state: &mut SubState,
    dt: f32,
    time: f32,
) {
    step_submarine_dbg(level, spec, inputs, state, dt, time, None);
}

/// Variant of `step_submarine` that fills out an optional debug telemetry struct.
pub fn step_submarine_dbg(
    level: &LevelSpec,
    spec: &SubPhysicsSpec,
    inputs: SubInputState,
    state: &mut SubState,
    dt: f32,
    time: f32,
    mut dbg: Option<&mut SubStepDebug>,
) {
    if dt <= 0.0 {
        return;
    }

    let (flow, _variance) = sample_flow_at(level, state.position, time);
    // Integrate ballast pumps and compute effective mass + buoyancy.
    let pump_rate_per_s = 0.2_f32;
    if state.ballast_fill.len() >= 2 {
        state.ballast_fill[0] = (state.ballast_fill[0]
            + inputs.pump_fwd.clamp(-1.0, 1.0) * pump_rate_per_s * dt)
            .clamp(0.0, 1.0);
        state.ballast_fill[1] = (state.ballast_fill[1]
            + inputs.pump_aft.clamp(-1.0, 1.0) * pump_rate_per_s * dt)
            .clamp(0.0, 1.0);
    }
    let mut ballast_mass = 0.0_f32;
    let mut total_capacity = 0.0_f32;
    for (i, tank) in spec.ballast_tanks.iter().enumerate() {
        let cap = tank.capacity_kg.max(0.0);
        let fill = *state.ballast_fill.get(i).unwrap_or(&0.0);
        ballast_mass += cap * fill.max(0.0);
        total_capacity += cap;
    }
    let m_eff = (spec.m + ballast_mass).max(1e-3);
    let g = 9.81_f32;
    // Neutral buoyancy at 50% fill across all tanks
    let base_disp_mass = spec.m + 0.5 * total_capacity;
    let buoyancy = base_disp_mass * g; // N upward
    let weight = m_eff * g; // N downward (increases with ballast)
    let buoy_net = buoyancy - weight; // + up

    // Body axes in world (standard basis: +Z fwd, +X right, +Y up)
    let forward = quat_rotate_vec3(state.orientation, BODY_FWD);
    let right = quat_rotate_vec3(state.orientation, BODY_RIGHT);
    let up_b = quat_rotate_vec3(state.orientation, BODY_UP);

    // Thrust force along forward
    let thrust_force = spec.t_max * inputs.thrust.clamp(-1.0, 1.0);
    let a_thrust = vscale(forward, thrust_force / m_eff);

    // Yaw dynamics
    let rel = vsub(state.velocity, flow); // water-relative velocity (world)
    let u_rel = rel.x * forward.x + rel.y * forward.y + rel.z * forward.z; // surge
    let rho = 1025.0_f32; // seawater density kg/m^3
    let q = 0.5 * rho * (u_rel * u_rel);
    let sign_u = if u_rel >= 0.0 { 1.0 } else { -1.0 };
    let front_mount_gain = if u_rel < 0.0 { 2.0 } else { 1.0 };
    let yaw_in = inputs.yaw.clamp(-1.0, 1.0);
    let tau_control = torque_yaw_control(spec, yaw_in, sign_u, front_mount_gain, q);

    debug_assert!(
        q >= 0.0 && spec.n_delta_r >= 0.0 && spec.s_side >= 0.0 && spec.length >= 0.0,
        "Expected non-negative scaling terms for yaw control torque"
    );
    if u_rel.abs() > 1e-3 && inputs.yaw.abs() > 1e-6 {
        let expected_sign = if sign_u > 0.0 {
            -inputs.yaw.signum()
        } else {
            inputs.yaw.signum()
        };
        if tau_control.abs() > 1e-6 {
            debug_assert!((tau_control.signum() - expected_sign).abs() < 1.01,
                "Rudder control torque sign mismatch: tau_control={}, yaw_in={}, sign_u={}, inputs.yaw={}",
                tau_control, yaw_in, sign_u, inputs.yaw);
        }
    }

    // Derive body angular velocity from stored body angular momentum
    let inv_ixx = if spec.ixx > 0.0 { 1.0 / spec.ixx } else { 0.0 };
    let inv_iyy = if spec.iyy > 0.0 { 1.0 / spec.iyy } else { 0.0 };
    let inv_izz = if spec.izz > 0.0 { 1.0 / spec.izz } else { 0.0 };
    let mut omega_body = Vec3f::new(
        state.ang_mom.x * inv_ixx,
        state.ang_mom.y * inv_iyy,
        state.ang_mom.z * inv_izz,
    );
    let r = omega_body.y;
    let tau_damp_lin = torque_yaw_damping_linear(spec, r);
    let tau_damp_quad = torque_yaw_damping_quadratic(spec, r);
    let tau_damp_dyn = torque_yaw_damping_dynamic(spec, q, r);
    let mut tau_yaw = tau_control + tau_damp_lin + tau_damp_quad + tau_damp_dyn;

    // Sideslip coupling
    let w_cpl = rel.x * right.x + rel.y * right.y + rel.z * right.z;
    let tau_ws = torque_sideslip_ws(spec, rho, w_cpl);
    tau_yaw += tau_ws;

    // Weathervane torque
    let des_x = -rel.x;
    let des_z = -rel.z;
    let des_len = (des_x * des_x + des_z * des_z).sqrt().max(1e-6);
    let desx = des_x / des_len;
    let desz = des_z / des_len;
    let fwdx = forward.x;
    let fwdz = forward.z;
    let dot = (fwdx * desx + fwdz * desz).clamp(-1.0, 1.0);
    let cross_y = fwdx * desz - fwdz * desx;
    let mut yaw_err = cross_y.atan2(dot.abs());
    if yaw_err > std::f32::consts::PI {
        yaw_err -= std::f32::consts::TAU;
    }
    if yaw_err < -std::f32::consts::PI {
        yaw_err += std::f32::consts::TAU;
    }
    let yaw_err = yaw_err.clamp(-0.7, 0.7);
    let tau_beta = torque_weathervane_beta(spec, q, yaw_err);
    tau_yaw += tau_beta;
    // Gyroscopic coupling: Euler equation in body frame: Ldot = tau - omega × L
    let l = state.ang_mom;
    // We'll accumulate pitch torque later, so start with yaw only for now
    let mut tau_b = Vec3f::new(0.0, tau_yaw, 0.0);

    let (cg_body_current, _m) = compute_cg_body_current(spec, state);

    // Pitch and roll torque due to ballast distribution and COB offset
    let g = 9.81_f32;
    let mut tau_pitch = torque_from_ballast_gravity_about_axis(
        spec,
        state,
        cg_body_current,
        state.orientation,
        right,
        g,
    );
    let mut tau_roll = torque_from_ballast_gravity_about_axis(
        spec,
        state,
        cg_body_current,
        state.orientation,
        forward,
        g,
    );
    tau_pitch += torque_from_cob_buoyancy_about_axis(spec, state.orientation, right, buoyancy);
    tau_roll += torque_from_cob_buoyancy_about_axis(spec, state.orientation, forward, buoyancy);

    // Linear pitch damping uses current omega.x
    let q_pitch = omega_body.x;
    let tau_pitch_damp = torque_pitch_linear_damping(spec, q_pitch);
    let tau_pitch_total = tau_pitch + tau_pitch_damp;
    // Add pitch and roll torque components and integrate full L with gyroscopic coupling
    tau_b.x = tau_pitch_total;
    // Tiny linear roll damping (no clamp): τ_roll += -kp * ωz
    let tau_roll_damp = torque_roll_linear_damping(spec, omega_body.z);
    tau_b.z = tau_roll + tau_roll_damp;
    // Ldot = tau_b - omega × L
    let cross = Vec3f::new(
        omega_body.y * l.z - omega_body.z * l.y,
        omega_body.z * l.x - omega_body.x * l.z,
        omega_body.x * l.y - omega_body.y * l.x,
    );
    let ldot = Vec3f::new(tau_b.x - cross.x, tau_b.y - cross.y, tau_b.z - cross.z);
    state.ang_mom = Vec3f::new(l.x + ldot.x * dt, l.y + ldot.y * dt, l.z + ldot.z * dt);

    // Clamp pitch and yaw rates by limiting momentum
    let q_max = 0.5; // ~29 deg/s
    let r_max = 0.6; // ~34 deg/s
    let l_x_max = spec.ixx * q_max;
    let l_y_max = spec.iyy * r_max;
    if state.ang_mom.x > l_x_max {
        state.ang_mom.x = l_x_max;
    }
    if state.ang_mom.x < -l_x_max {
        state.ang_mom.x = -l_x_max;
    }
    if state.ang_mom.y > l_y_max {
        state.ang_mom.y = l_y_max;
    }
    if state.ang_mom.y < -l_y_max {
        state.ang_mom.y = -l_y_max;
    }

    // Update orientation using body-frame angular velocities (post-multiply deltas)
    omega_body = Vec3f::new(
        state.ang_mom.x * inv_ixx,
        state.ang_mom.y * inv_iyy,
        state.ang_mom.z * inv_izz,
    );
    // Debug yaw acceleration from Euler equation: omega_dot_y = Ldot_y / Iyy
    let yaw_acc = if spec.iyy > 0.0 {
        ldot.y * inv_iyy
    } else {
        0.0
    };
    let delta_yaw = Quatf::from_axis_angle(BODY_UP, omega_body.y * dt);
    // Pitch about body-right (+X)
    let delta_pitch = Quatf::from_axis_angle(BODY_RIGHT, omega_body.x * dt);
    // Roll about body-forward (+Z)
    let delta_roll = Quatf::from_axis_angle(BODY_FWD, omega_body.z * dt);
    state.orientation = (state.orientation * delta_pitch * delta_yaw * delta_roll).normalize();

    // Rudder sideforce tied to yaw rate: approximate centripetal acceleration ~ u * r
    // Positive yaw rate (left turn) should create acceleration to the left (−right axis)
    // Ideal centripetal acceleration to track yaw rate: a_c = v * r toward the center of curvature
    let a_rudder = vscale(right, -u_rel * r);

    // Hydrodynamic drag in body axes
    let u = rel.x * forward.x + rel.y * forward.y + rel.z * forward.z;
    let v_comp = rel.x * up_b.x + rel.y * up_b.y + rel.z * up_b.z;
    let w = rel.x * right.x + rel.y * right.y + rel.z * right.z;
    let fx = -(0.5 * rho * spec.cxd * spec.s_forward * u.abs() * u + spec.xu * u);
    let fy = -(0.5 * rho * spec.czd * spec.s_top * v_comp.abs() * v_comp + spec.zw * v_comp);
    let fz = -(0.5 * rho * spec.cyd * spec.s_side * w.abs() * w + spec.yv * w);
    let f_world = Vec3f::new(
        forward.x * fx + up_b.x * fy + right.x * fz,
        forward.y * fx + up_b.y * fy + right.y * fz,
        forward.z * fx + up_b.z * fy + right.z * fz,
    );
    let a_drag = vscale(f_world, 1.0 / m_eff);

    // Net buoyancy acceleration (world up)
    let a_buoy = Vec3f::new(0.0, buoy_net / m_eff, 0.0);

    // Sum accelerations
    let a = vadd(vadd(vadd(a_thrust, a_drag), a_rudder), a_buoy);

    // Integrate
    state.velocity = vadd(state.velocity, vscale(a, dt));
    state.position = vadd(state.position, vscale(state.velocity, dt));

    if let Some(d) = dbg.as_mut() {
        d.dt = dt;
        d.time = time;
        d.inputs = inputs;
        d.raw_inputs = None;
        d.forward = forward;
        d.right = right;
        d.flow = flow;
        d.rel = rel;
        d.u = u;
        d.v = v_comp;
        d.w = w;
        d.q_dyn = q;
        d.sign_u = sign_u;
        d.front_mount_gain = front_mount_gain;
        d.thrust_force = thrust_force;
        d.fx = fx;
        d.fy = fy;
        d.fz = fz;
        d.f_world = f_world;
        d.f_rudder_lat = -u_rel * r * m_eff;
        d.tau_control = tau_control;
        d.tau_damp_lin = tau_damp_lin;
        d.tau_damp_quad = tau_damp_quad;
        d.tau_damp_dyn = tau_damp_dyn;
        d.tau_ws = tau_ws;
        d.tau_beta = tau_beta;
        d.tau_total = tau_yaw;
        d.yaw_err = yaw_err;
        d.yaw_acc = yaw_acc;
        d.yaw_rate = omega_body.y;
        d.heading_yaw = quat_to_yaw(state.orientation);
        d.fill_fwd = state.ballast_fill.first().copied().unwrap_or(0.0);
        d.fill_aft = state.ballast_fill.get(1).copied().unwrap_or(0.0);
        d.mass_eff = m_eff;
        d.buoyancy_n = buoyancy;
        d.weight_n = weight;
        d.buoy_net_n = buoy_net;
        d.tau_pitch = tau_pitch;
        d.up_b = up_b;
    }
}

fn compute_cg_body_current(spec: &SubPhysicsSpec, state: &SubState) -> (Vec3f, f32) {
    let mut m_total = spec.m.max(0.0);
    let mut mr_sum = Vec3f::new(0.0, 0.0, 0.0) * m_total;

    for (i, tank) in spec.ballast_tanks.iter().enumerate() {
        let cap = tank.capacity_kg.max(0.0);
        if cap <= 0.0 {
            continue;
        }
        let fill = state
            .ballast_fill
            .get(i)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        let m = cap * fill;
        if m > 0.0 {
            m_total += m;
            mr_sum += tank.pos_body * m;
        }
    }

    if m_total > 0.0 {
        (mr_sum * (1.0 / m_total), m_total)
    } else {
        (Vec3f::new(0.0, 0.0, 0.0), 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BallastTankSpec;

    fn spec_with_two_tanks() -> SubPhysicsSpec {
        let mut s = crate::subspecs::small_skiff_spec();
        // Override to two symmetric tanks on +X and -X with round numbers
        s.m = 100.0;
        s.ballast_tanks = vec![
            BallastTankSpec {
                pos_body: Vec3f::new(1.0, 0.0, 0.0),
                capacity_kg: 20.0,
            },
            BallastTankSpec {
                pos_body: Vec3f::new(-1.0, 0.0, 0.0),
                capacity_kg: 20.0,
            },
        ];
        s
    }

    fn base_state() -> SubState {
        SubState {
            position: Vec3f::new(0.0, 0.0, 0.0),
            velocity: Vec3f::new(0.0, 0.0, 0.0),
            orientation: Quatf::from_rotation_y(0.0),
            ang_mom: Vec3f::new(0.0, 0.0, 0.0),
            ballast_fill: vec![0.0, 0.0],
        }
    }

    #[test]
    fn cg_shifts_toward_filled_tank() {
        let spec = spec_with_two_tanks();
        let mut state = base_state();
        // Fill only the +X tank
        state.ballast_fill = vec![1.0, 0.0];
        let (cg, m_total) = compute_cg_body_current(&spec, &state);
        // total mass = hull + 20 kg
        assert!((m_total - (spec.m + 20.0)).abs() < 1e-6);
        // cg.x = (20*1) / (hull + 20)
        let expected_x = 20.0 / (spec.m + 20.0);
        assert!(
            (cg.x - expected_x).abs() < 1e-6,
            "cg.x={}, expected={}",
            cg.x,
            expected_x
        );
        assert!(cg.y.abs() < 1e-6 && cg.z.abs() < 1e-6);
    }

    #[test]
    fn cg_balanced_when_both_equal() {
        let spec = spec_with_two_tanks();
        let mut state = base_state();
        state.ballast_fill = vec![1.0, 1.0];
        let (cg, m_total) = compute_cg_body_current(&spec, &state);
        assert!((m_total - (spec.m + 40.0)).abs() < 1e-6);
        assert!(cg.length() < 1e-6, "cg should be at origin when symmetric");
    }
}
