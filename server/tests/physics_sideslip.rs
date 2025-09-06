use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, SubInputs, SubState, Vec3f, step_submarine, Quatf};

fn level_with_uniform_flow(mut base: LevelSpec, flow: Vec3f) -> LevelSpec {
    base.tunnel.flow = FlowFieldSpec::Uniform { flow, variance: 0.0 };
    base
}

fn run_sideslip_scenario(flow: Vec3f, ticks: usize, thrust: f32) {
    // Construct level and spec
    let level = level_with_uniform_flow(greybox_level(), flow);
    let spec = levels::subspecs::small_skiff_spec();

    // Start centered in tunnel, heading down +X
    let mut state = SubState {
        position: Vec3f { x: level.tunnel.pos.x, y: level.tunnel.pos.y, z: level.tunnel.pos.z },
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_yaw(0.0),
        ang_vel: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.0; spec.ballast_tanks.len()],
    };

    // Simulate
    let dt = 1.0 / 30.0; // 30 Hz
    let mut t = 0.0f32; 
    let inputs = SubInputs { thrust, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    let mut tick_counter = 0;
    for _ in 0..ticks {
        step_submarine(&level, &spec, inputs, &mut state, dt, t);
        t += dt;
        tick_counter += 1;
    }

    // Compute body-frame components of velocity
    let forward = state.orientation.rotate_vec3(Vec3f::new(1.0, 0.0, 0.0));
    let up = Vec3f { x: 0.0, y: 1.0, z: 0.0 };
    let right = Vec3f { x: forward.y * up.z - forward.z * up.y, y: forward.z * up.x - forward.x * up.z, z: forward.x * up.y - forward.y * up.x };

    let v = state.velocity;
    let u = v.x * forward.x + v.y * forward.y + v.z * forward.z; // surge
    let w = v.x * right.x + v.y * right.y + v.z * right.z; // sway

    // Thresholds
    let slip_ratio_thresh = 0.02;
    let abs_sway_thresh = 0.05;

    let slip_ratio = (w.abs()) / (u.abs().max(1e-3));
    assert!(slip_ratio < slip_ratio_thresh, "slip ratio too large: {} (u={}, w={}) after {} ticks", slip_ratio, u, w, tick_counter);
    assert!(w.abs() < abs_sway_thresh, "absolute sway too large: {} m/s after {} ticks)", w, tick_counter);
}

fn run_rudder_sign_scenario(thrust: f32, _rudder: f32, warm_ticks: usize, steer_ticks: usize) {
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 0.0));
    let spec = levels::subspecs::small_skiff_spec();
    let mut state = SubState {
        position: Vec3f { x: level.tunnel.pos.x, y: level.tunnel.pos.y, z: level.tunnel.pos.z },
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_yaw(0.0),
        ang_vel: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.0; spec.ballast_tanks.len()],
    };
    let dt = 1.0 / 30.0; let mut t = 0.0f32;
    let warm_inputs = SubInputs { thrust, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..warm_ticks { step_submarine(&level, &spec, warm_inputs, &mut state, dt, t); t += dt; }
    let yaw0 = state.orientation.to_yaw();
    // Steer to the right with positive slider
    let steer_inputs = SubInputs { thrust, yaw: 0.2, pump_fwd: 0.0, pump_aft: 0.0 };
    let mut w_sum = 0.0f32;
    for i in 0..steer_ticks {
        step_submarine(&level, &spec, steer_inputs, &mut state, dt, t); t += dt;
        if i + 100 >= steer_ticks {
            let forward = state.orientation.rotate_vec3(Vec3f::new(1.0, 0.0, 0.0)); let up = Vec3f { x: 0.0, y: 1.0, z: 0.0 };
            let right = Vec3f { x: forward.y * up.z - forward.z * up.y, y: forward.z * up.x - forward.x * up.z, z: forward.x * up.y - forward.y * up.x };
            let v = state.velocity; let w = v.x * right.x + v.y * right.y + v.z * right.z; w_sum += w;
        }
    }
    let yaw1 = state.orientation.to_yaw();
    assert!(yaw1 < yaw0 - 0.01, "rudder right did not decrease yaw: yaw0={}, yaw1={}", yaw0, yaw1);
    // Sideslip magnitude should not blow up during a gentle right turn
    assert!(w_sum.abs() < 2.0, "excessive average sway magnitude: w_sum={}", w_sum);
}

#[test]
fn rudder_sign_and_sway_consistency() { run_rudder_sign_scenario(0.4, 0.2, 1500, 1500); }

#[test]
fn forward_throttle_no_significant_sideslip() {
    run_sideslip_scenario(Vec3f::new(0.0, 0.0, 0.0), 5000, 0.1);
}

#[test]
fn forward_throttle_no_significant_sideslip_stronger() {
    run_sideslip_scenario(Vec3f::new(0.0, 0.0, 0.0), 5000, 0.5);
}

#[test]
fn forward_throttle_no_significant_sideslip_with_flow() {
    // With tail/current flow along +X, sideslip should remain negligible under forward thrust
    run_sideslip_scenario(Vec3f::new(1.5, 0.0, 0.0), 10_000, 0.8);
}
