use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, SubInputs, SubState, Vec3f, step_submarine, Quatf};

fn level_with_uniform_flow(mut base: LevelSpec, flow: Vec3f) -> LevelSpec {
    base.tunnel.flow = FlowFieldSpec::Uniform { flow, variance: 0.0 };
    base
}

#[test]
fn right_rudder_decreases_yaw_when_moving_forward() {
    // Zero ambient flow; move forward under thrust
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 0.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f { x: level.tunnel.pos.x, y: level.tunnel.pos.y, z: level.tunnel.pos.z },
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_yaw(0.0),
        ang_vel: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.0; spec.ballast_tanks.len()],
    };

    let dt = 1.0 / 60.0; // fine step; not critical
    let mut t = 0.0f32;

    // Warm up to get forward relative flow
    let warm = SubInputs { thrust: 0.5, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, warm, &mut state, dt, t); t += dt; }
    let yaw0 = state.orientation.to_yaw();

    // Apply right rudder (positive input) while moving forward
    let steer = SubInputs { thrust: 0.5, yaw: 0.3, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, steer, &mut state, dt, t); t += dt; }
    let yaw1 = state.orientation.to_yaw();

    assert!(yaw1 < yaw0 - 0.005, "right rudder should decrease yaw under forward motion (yaw0={}, yaw1={})", yaw0, yaw1);
}

#[test]
fn right_rudder_decreases_yaw_when_moving_backward() {
    // Zero ambient flow; move backward under thrust
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 0.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f { x: level.tunnel.pos.x, y: level.tunnel.pos.y, z: level.tunnel.pos.z },
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_yaw(0.0),
        ang_vel: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.0; spec.ballast_tanks.len()],
    };

    let dt = 1.0 / 60.0; let mut t = 0.0f32;

    // Warm up backward (negative thrust) to get reversed relative flow
    let warm = SubInputs { thrust: -0.6, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, warm, &mut state, dt, t); t += dt; }
    let yaw0 = state.orientation.to_yaw();

    // Apply right rudder (positive input) while moving backward
    let steer = SubInputs { thrust: -0.6, yaw: 0.3, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, steer, &mut state, dt, t); t += dt; }
    let yaw1 = state.orientation.to_yaw();

    assert!(yaw1 < yaw0 - 0.005, "right rudder should decrease yaw under reverse motion (yaw0={}, yaw1={})", yaw0, yaw1);
}
