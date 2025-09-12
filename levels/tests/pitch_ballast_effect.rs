use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, SubInputs, SubState, Vec3f, step_submarine, Quatf};

fn level_with_uniform_flow(mut base: LevelSpec, flow: Vec3f) -> LevelSpec {
    base.tunnel.flow = FlowFieldSpec::Uniform { flow, variance: 0.0 };
    base
}

fn forward_vector(q: Quatf) -> Vec3f { q * Vec3f::new(1.0, 0.0, 0.0) }

fn pitch_angle_from_forward(fwd: Vec3f) -> f32 {
    // Signed pitch: + up, - down
    let horiz = (fwd.x * fwd.x + fwd.z * fwd.z).sqrt().max(1e-6);
    fwd.y.atan2(horiz)
}

#[test]
fn forward_heavy_ballast_pitches_nose_down() {
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 0.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z),
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_rotation_y(0.0),
        ang_mom: Vec3f::new(0.0, 0.0, 0.0),
        // Heavier forward (1.0) vs aft (0.0) should create negative pitch torque (nose down)
        ballast_fill: vec![1.0, 0.0],
    };

    let dt = 1.0 / 60.0; let mut t = 0.0f32;
    let inputs = SubInputs { thrust: 0.0, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, inputs, &mut state, dt, t); t += dt; }

    let fwd = forward_vector(state.orientation);
    let pitch = pitch_angle_from_forward(fwd);
    assert!(pitch < -0.05, "expected nose-down pitch (forward heavy); got {pitch} rad (fwd={:?})", fwd);
}

#[test]
fn aft_heavy_ballast_pitches_nose_up() {
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 0.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z),
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_rotation_y(0.0),
        ang_mom: Vec3f::new(0.0, 0.0, 0.0),
        // Heavier aft (1.0) vs forward (0.0) should create positive pitch torque (nose up)
        ballast_fill: vec![0.0, 1.0],
    };

    let dt = 1.0 / 60.0; let mut t = 0.0f32;
    let inputs = SubInputs { thrust: 0.0, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..600 { step_submarine(&level, &spec, inputs, &mut state, dt, t); t += dt; }

    let fwd = forward_vector(state.orientation);
    let pitch = pitch_angle_from_forward(fwd);
    assert!(pitch > 0.05, "expected nose-up pitch (aft heavy); got {pitch} rad (fwd={:?})", fwd);
}
