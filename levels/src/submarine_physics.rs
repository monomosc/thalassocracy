use crate::{FlowFieldSpec, LevelSpec, Vec3f, Quatf, SubPhysicsSpec};

#[derive(Debug, Clone, Copy, Default)]
pub struct SubStepDebug {
    pub dt: f32,
    pub time: f32,
    pub inputs: SubInputs,
    // Orientation basis (world XZ plane)
    pub forward: Vec3f,
    pub right: Vec3f,
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
}

#[derive(Debug, Clone)]
pub struct SubState {
    pub position: Vec3f,
    pub velocity: Vec3f,
    /// Orientation as quaternion (body→world). Positive yaw turns +X toward −Z.
    pub orientation: Quatf,
    /// Angular velocity in body frame (rad/s). Currently only `.y` is driven (yaw).
    pub ang_vel: Vec3f,
    /// Ballast tank fill state in [0,1] for each tank in spec.ballast_tanks (future use)
    pub ballast_fill: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubInputs {
    pub thrust: f32,  // -1..1 (forward/back)
    /// Rudder input in [-1, 1].
    /// Convention: +1 = right rudder (nose yaws right when moving forward),
    /// -1 = left rudder. The physics maps this to yaw torque so that forward
    /// motion with positive input decreases heading_yaw (right turn).
    pub yaw: f32,     // -1..1 (right rudder positive)
    /// Forward ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_fwd: f32,
    /// Aft ballast pump speed in [-1,1]. +1 pumps water in (fill), -1 pumps out.
    pub pump_aft: f32,
}

#[inline]
fn vadd(a: Vec3f, b: Vec3f) -> Vec3f { Vec3f { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z } }
#[inline]
fn vsub(a: Vec3f, b: Vec3f) -> Vec3f { Vec3f { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z } }
#[inline]
fn vscale(a: Vec3f, s: f32) -> Vec3f { Vec3f { x: a.x * s, y: a.y * s, z: a.z * s } }

/// Sample the flow field and variance at a world position.
/// Currently only the tunnel contributes; extend later for multiple fields.
pub fn sample_flow_at(level: &LevelSpec, pos: Vec3f, time: f32) -> (Vec3f, f32) {
    let mut flow = Vec3f::new(0.0, 0.0, 0.0);
    let mut variance = 0.0f32;
    let mut count = 0.0f32;

    // Tunnel AABB check
    let half = Vec3f::new(level.tunnel.size.x * 0.5, level.tunnel.size.y * 0.5, level.tunnel.size.z * 0.5);
    let min = vsub(level.tunnel.pos, half);
    let max = vadd(level.tunnel.pos, half);
    if pos.x >= min.x && pos.x <= max.x && pos.y >= min.y && pos.y <= max.y && pos.z >= min.z && pos.z <= max.z {
        match level.tunnel.flow {
            FlowFieldSpec::Uniform { flow: f, variance: var } => {
                flow = vadd(flow, f);
                variance += var;
                count += 1.0;
            }
        }
    }

    // Torus tunnel interior check (if present). Uses a simple geometric test:
    // let n be the normalized axis, d = pos - center. Decompose d into normal
    // component (h along n) and in-plane component (p = d - h*n). The distance
    // to the torus tube centerline is sqrt((|p| - R)^2 + h^2). Inside if <= r.
    if let Some(t) = &level.torus_tunnel {
        // Normalize axis safely
        let axis_len2 = t.axis.x * t.axis.x + t.axis.y * t.axis.y + t.axis.z * t.axis.z;
        if axis_len2 > 1e-8 {
            let axis_len = axis_len2.sqrt();
            let n = Vec3f { x: t.axis.x / axis_len, y: t.axis.y / axis_len, z: t.axis.z / axis_len };
            let d = vsub(pos, t.center);
            let h = d.x * n.x + d.y * n.y + d.z * n.z; // signed height from ring plane
            let p = Vec3f { x: d.x - n.x * h, y: d.y - n.y * h, z: d.z - n.z * h };
            let p_len = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt();
            let tube = ((p_len - t.major_radius).abs().powi(2) + h * h).sqrt();
            if tube <= t.minor_radius {
                match t.flow {
                    FlowFieldSpec::Uniform { flow: f, variance: var } => {
                        flow = vadd(flow, f);
                        variance += var;
                        count += 1.0;
                    }
                }
            }
        }
    }

    if count > 0.0 {
        flow = vscale(flow, 1.0 / count);
        variance /= count;
    }

    // Return mean flow only; variance can be used to modulate control/torque noise if desired.
    let _ = time; // keep signature for deterministic extensions
    (flow, variance)
}

/// Simple submarine dynamics step honoring thrust and rudder in a flow field.
/// - Heading is around world +Y; positive yaw turns left (towards −Z).
/// - Inputs: `thrust ∈ [-1,1]` (forward/back), `yaw ∈ [-1,1]` (right rudder positive).
/// - Rudder yaw torque scales with dynamic pressure along body-forward and flips sign
///   when relative flow is reversed (intuitive "steering while backing up").
pub fn step_submarine(level: &LevelSpec, spec: &SubPhysicsSpec, inputs: SubInputs, state: &mut SubState, dt: f32, time: f32) {
    step_submarine_dbg(level, spec, inputs, state, dt, time, None);
}

/// Variant of `step_submarine` that fills out an optional debug telemetry struct.
pub fn step_submarine_dbg(
    level: &LevelSpec,
    spec: &SubPhysicsSpec,
    inputs: SubInputs,
    state: &mut SubState,
    dt: f32,
    time: f32,
    mut dbg: Option<&mut SubStepDebug>,
) {
    if dt <= 0.0 { return; }

    let (flow, _variance) = sample_flow_at(level, state.position, time);
    // Integrate ballast pumps and compute effective mass + buoyancy.
    // Pump model: +1 fills, -1 empties; 0.2 fill/sec rate.
    let pump_rate_per_s = 0.2_f32;
    if state.ballast_fill.len() >= 2 {
        state.ballast_fill[0] = (state.ballast_fill[0]
            + inputs.pump_fwd.clamp(-1.0, 1.0) * pump_rate_per_s * dt).clamp(0.0, 1.0);
        state.ballast_fill[1] = (state.ballast_fill[1]
            + inputs.pump_aft.clamp(-1.0, 1.0) * pump_rate_per_s * dt).clamp(0.0, 1.0);
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
    let _rho_w = 1000.0_f32; // water density kg/m^3 (unused in this neutral baseline)
    let g = 9.81_f32;
    // Neutral buoyancy at 50% fill across all tanks
    let base_disp_mass = spec.m + 0.5 * total_capacity;
    let buoyancy = base_disp_mass * g;         // N upward
    let weight = m_eff * g;                    // N downward (increases with ballast)
    let buoy_net = buoyancy - weight;          // + up

    // Forward unit vector from orientation (body +X)
    let forward = state.orientation.rotate_vec3(Vec3f::new(1.0, 0.0, 0.0));
    // Speed and body axes (derive directly from orientation to avoid degeneracy when forward ~ world up)
    let speed2 = state.velocity.x * state.velocity.x + state.velocity.y * state.velocity.y + state.velocity.z * state.velocity.z;
    // World-space body axes
    let right = state.orientation.rotate_vec3(Vec3f::new(0.0, 0.0, 1.0));
    let up_b = state.orientation.rotate_vec3(Vec3f::new(0.0, 1.0, 0.0));

    // Thrust force along forward
    let thrust_force = spec.t_max * inputs.thrust.clamp(-1.0, 1.0);
    let a_thrust = vscale(forward, thrust_force / m_eff);

    // Rudder input: positive = right rudder (nose right under forward motion)
    let _speed = speed2.sqrt();
    let yaw_in = (-inputs.yaw).clamp(-1.0, 1.0);
    let mut a_rudder = Vec3f::new(0.0, 0.0, 0.0);

    // Yaw dynamics: torque from rudder scales with dynamic pressure along forward; add angular damping.
    let rel = vsub(state.velocity, flow); // water-relative velocity (world)
    let u_rel = rel.x * forward.x + rel.y * forward.y + rel.z * forward.z; // surge
    let rho = 1025.0_f32; // seawater density kg/m^3
    let q = 0.5 * rho * (u_rel * u_rel);
    let sign_u = if u_rel >= 0.0 { 1.0 } else { -1.0 };
    // Increase authority when flow is reversed (rudder effectively front-mounted)
    let front_mount_gain = if u_rel < 0.0 { 2.0 } else { 1.0 };
    // Control torque from rudder alone (before damping/flow terms)
    let tau_control = yaw_in * sign_u * front_mount_gain * spec.n_delta_r * q * spec.s_side * spec.length;
    // Runtime guardrail: with forward relative flow (sign_u=+1) and positive input,
    // control torque should reduce yaw (right turn → negative torque). With reversed
    // flow (sign_u=-1), positive input should produce positive torque (nose still goes
    // to the right when backing, given aft-mounted rudder moves the tail left).
    debug_assert!(
        q >= 0.0 && spec.n_delta_r >= 0.0 && spec.s_side >= 0.0 && spec.length >= 0.0,
        "Expected non-negative scaling terms for yaw control torque"
    );
    if u_rel.abs() > 1e-3 && inputs.yaw.abs() > 1e-6 {
        let expected_sign = if sign_u > 0.0 { -inputs.yaw.signum() } else { inputs.yaw.signum() };
        // Allow zero if q ~ 0
        if tau_control.abs() > 1e-6 { debug_assert!((tau_control.signum() - expected_sign).abs() < 1.01, "Rudder control torque sign mismatch: tau_control={}, yaw_in={}, sign_u={}, inputs.yaw={}", tau_control, yaw_in, sign_u, inputs.yaw); }
    }

    let r = state.ang_vel.y;
    let tau_damp_lin = - spec.kr * r;
    let tau_damp_quad = - spec.kr2 * r.abs() * r;
    let tau_damp_dyn = - spec.nr_v * q * r;
    let mut tau_yaw = tau_control + tau_damp_lin + tau_damp_quad + tau_damp_dyn;
    // Sideslip (sway) coupling: positive w (body-right) should turn the nose into the slip (to the right).
    // Convention: positive yaw increases heading toward +Z (left), so apply negative yaw torque when w > 0
    // (nose turns right to reduce rightward sideslip).
    let w_cpl = rel.x * right.x + rel.y * right.y + rel.z * right.z;
    let tau_ws = -spec.n_ws * w_cpl * (0.5 * rho * spec.s_side * spec.length);
    tau_yaw += tau_ws;

    // Weathervane torque: rotate heading towards incoming water direction (reduce sideslip)
    // Compute signed small-angle error between forward (+X in body/world yaw plane)
    // and the incoming water direction (−rel) in the XZ plane.
    let des_x = -rel.x; let des_z = -rel.z;
    let des_len = (des_x * des_x + des_z * des_z).sqrt().max(1e-6);
    let desx = des_x / des_len; let desz = des_z / des_len;
    // Forward in XZ from current heading
    let fwdx = forward.x; let fwdz = forward.z;
    let dot = (fwdx * desx + fwdz * desz).clamp(-1.0, 1.0);
    let cross_y = fwdx * desz - fwdz * desx; // + when desired is to the left of forward
    // Symmetrize along surge: treat forward/back along the same line (no urge to flip 180°).
    // Using atan2(cross, |dot|) yields zero error when desired is exactly ahead or behind.
    let mut yaw_err = cross_y.atan2(dot.abs());
    // Wrap to [-pi, pi]
    if yaw_err > std::f32::consts::PI { yaw_err -= std::f32::consts::TAU; }
    if yaw_err < -std::f32::consts::PI { yaw_err += std::f32::consts::TAU; }
    // Clamp error to avoid excessive torque at large misalignment
    let yaw_err = yaw_err.clamp(-0.7, 0.7);
    // Positive yaw_err should increase yaw (left), aligning heading to incoming water
    let tau_beta = spec.n_beta * q * spec.s_side * spec.length * yaw_err;
    tau_yaw += tau_beta;
    let yaw_acc = if spec.izz > 0.0 { tau_yaw / spec.izz } else { 0.0 };
    // Integrate yaw angular velocity
    state.ang_vel.y += yaw_acc * dt;
    let r_max = 0.6; // ~34 deg/s
    if state.ang_vel.y > r_max { state.ang_vel.y = r_max; } else if state.ang_vel.y < -r_max { state.ang_vel.y = -r_max; }
    // ------------------------------------------------------------
    // Pitch dynamics from ballast distribution (world torque projected onto body-right)
    // Torque about body right axis (world `right` vector) due to vertical forces
    // from ballast mass deviations relative to 50% fill baseline.
    // Compute torque in world coordinates: tau = sum_i (r_i_world × F_i_world) · right_world
    let mut tau_pitch = 0.0_f32; // about +right axis; negative => nose down
    for (i, tank) in spec.ballast_tanks.iter().enumerate() {
        let cap = tank.capacity_kg.max(0.0);
        let fill = state.ballast_fill.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let delta_m = (fill - 0.5) * cap; // positive = extra weight vs neutral
        if delta_m.abs() <= 0.0 { continue; }
        // Position of ballast mass in body space rotated to world space
        let r_world = state.orientation.rotate_vec3(Vec3f::new(tank.pos_body.x, tank.pos_body.y, tank.pos_body.z));
        // Downward force from excess mass (world)
        let f_world = Vec3f::new(0.0, -delta_m * g, 0.0);
        // Cross product r × F (world)
        let tau_world = Vec3f {
            x: r_world.y * f_world.z - r_world.z * f_world.y,
            y: r_world.z * f_world.x - r_world.x * f_world.z,
            z: r_world.x * f_world.y - r_world.y * f_world.x,
        };
        // Component about body-right axis (world)
        let tau_about_right = tau_world.x * right.x + tau_world.y * right.y + tau_world.z * right.z;
        tau_pitch += tau_about_right;
    }
    // Restoring torque from center of buoyancy offset (projects onto pitch axis)
    if !(spec.cb_offset_body.x == 0.0 && spec.cb_offset_body.y == 0.0 && spec.cb_offset_body.z == 0.0) {
        let r_cb_world = state.orientation.rotate_vec3(spec.cb_offset_body);
        let f_buoy_world = Vec3f { x: 0.0, y: buoyancy, z: 0.0 };
        let tau_cb_world = Vec3f {
            x: r_cb_world.y * f_buoy_world.z - r_cb_world.z * f_buoy_world.y,
            y: r_cb_world.z * f_buoy_world.x - r_cb_world.x * f_buoy_world.z,
            z: r_cb_world.x * f_buoy_world.y - r_cb_world.y * f_buoy_world.x,
        };
        let tau_cb_pitch = tau_cb_world.x * right.x + tau_cb_world.y * right.y + tau_cb_world.z * right.z;
        tau_pitch += tau_cb_pitch;
    }

    // Linear pitch damping
    let q_pitch = state.ang_vel.z; // use .z component for pitch rate about +right
    let tau_pitch_damp = -spec.kq * q_pitch;
    let tau_pitch_total = tau_pitch + tau_pitch_damp;
    let pitch_acc = if spec.iyy > 0.0 { tau_pitch_total / spec.iyy } else { 0.0 };
    state.ang_vel.z += pitch_acc * dt;
    // Limit pitch rate similarly
    let q_max = 0.5; // ~29 deg/s
    if state.ang_vel.z > q_max { state.ang_vel.z = q_max; } else if state.ang_vel.z < -q_max { state.ang_vel.z = -q_max; }
    // Update orientation from yaw and pitch angular velocities
    let delta_yaw = crate::Quatf::from_axis_angle(Vec3f::new(0.0, 1.0, 0.0), state.ang_vel.y * dt);
    let delta_pitch = crate::Quatf::from_axis_angle(right, state.ang_vel.z * dt);
    state.orientation = state.orientation.mul_q(delta_pitch).mul_q(delta_yaw).normalize();

    // Add rudder sideforce as primary source of centripetal force to bend the path
    // Positive input with forward flow yields rightward force; reverse flow flips sign.
    let f_lat = inputs.yaw.clamp(-1.0, 1.0) * sign_u * front_mount_gain * spec.y_delta_r * q * spec.s_side;
    let a_lat = vscale(right, f_lat / m_eff);
    a_rudder = vadd(a_rudder, a_lat);

    // Hydrodynamic drag based on water-relative velocity components in body axes
    // Decompose into forward (x), up (y), right (z) using rotated basis
    let u = rel.x * forward.x + rel.y * forward.y + rel.z * forward.z; // surge (body forward)
    let v_comp = rel.x * up_b.x + rel.y * up_b.y + rel.z * up_b.z; // heave (body up)
    let w = rel.x * right.x + rel.y * right.y + rel.z * right.z; // sway (body right)
    // Quadratic + linear damping per axis: F = -sign * 0.5*rho*Cd*A*comp^2 - K*comp
    let fx = -(0.5 * rho * spec.cxd * spec.s_forward * u.abs() * u + spec.xu * u);
    let fy = -(0.5 * rho * spec.czd * spec.s_top * v_comp.abs() * v_comp + spec.zw * v_comp);
    let fz = -(0.5 * rho * spec.cyd * spec.s_side * w.abs() * w + spec.yv * w);
    // Recompose to world using body axes
    let f_world = Vec3f {
        x: forward.x * fx + up_b.x * fy + right.x * fz,
        y: forward.y * fx + up_b.y * fy + right.y * fz,
        z: forward.z * fx + up_b.z * fy + right.z * fz,
    };
    let a_drag = vscale(f_world, 1.0 / m_eff);

    // Net buoyancy acceleration (world up)
    let a_buoy = Vec3f { x: 0.0, y: buoy_net / m_eff, z: 0.0 };

    // Sum accelerations
    let a = vadd(vadd(vadd(a_thrust, a_drag), a_rudder), a_buoy);

    // Integrate
    state.velocity = vadd(state.velocity, vscale(a, dt));
    state.position = vadd(state.position, vscale(state.velocity, dt));

    // Fill debug telemetry if requested
    if let Some(d) = dbg.as_mut() {
        d.dt = dt;
        d.time = time;
        d.inputs = inputs;
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
        d.fx = fx; d.fy = fy; d.fz = fz;
        d.f_world = f_world;
        d.f_rudder_lat = f_lat;
        d.tau_control = tau_control;
        d.tau_damp_lin = tau_damp_lin;
        d.tau_damp_quad = tau_damp_quad;
        d.tau_damp_dyn = tau_damp_dyn;
        d.tau_ws = tau_ws;
        d.tau_beta = tau_beta;
        d.tau_total = tau_yaw;
        d.yaw_err = yaw_err;
        d.yaw_acc = yaw_acc;
        d.yaw_rate = state.ang_vel.y;
        d.heading_yaw = state.orientation.to_yaw();
        // Ballast & buoyancy
        d.fill_fwd = state.ballast_fill.first().copied().unwrap_or(0.0);
        d.fill_aft = state.ballast_fill.get(1).copied().unwrap_or(0.0);
        d.mass_eff = m_eff;
        d.buoyancy_n = buoyancy;
        d.weight_n = weight;
        d.buoy_net_n = buoy_net;
    }
}
