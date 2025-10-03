use levels::{
    builtins::greybox_level, step_submarine, FlowFieldSpec, LevelSpec, Quatf, SubInputState,
    SubState, Vec3f,
};

fn calm_level(mut base: LevelSpec) -> LevelSpec {
    base.tunnel.flow = FlowFieldSpec::Uniform {
        flow: Vec3f::new(0.0, 0.0, 0.0),
        variance: 0.0,
    };
    base
}

#[test]
fn forward_full_aft_empty_pitches_nose_down() {
    let level = calm_level(greybox_level());
    let spec = levels::subspecs::small_skiff_spec();

    // Start centered, neutral orientation
    let mut state = SubState {
        position: Vec3f::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z),
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_rotation_y(0.0),
        ang_mom: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.5; spec.ballast_tanks.len()],
    };

    // Set forward tank to full (index 0), aft to empty (index 1)
    if spec.ballast_tanks.len() >= 2 {
        state.ballast_fill[0] = 1.0; // forward full
        state.ballast_fill[1] = 0.0; // aft empty
    }

    // No thrust or rudder; no pumps during the test
    let inputs = SubInputState {
        thrust: 0.0,
        yaw: 0.0,
        pump_fwd: 0.0,
        pump_aft: 0.0,
    };
    let dt = 1.0 / 60.0;
    let mut t = 0.0f32;

    // Run for a short while to integrate pitch
    for _ in 0..900 {
        // 15 seconds
        step_submarine(&level, &spec, inputs, &mut state, dt, t);
        t += dt;
    }

    // Forward vector should have a negative Y component (nose pitched downward)
    let fwd = state.orientation * Vec3f::new(1.0, 0.0, 0.0);
    assert!(
        fwd.y < -0.02,
        "expected nose-down pitch; forward.y = {}",
        fwd.y
    );
}
