use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, SubInputs, SubState, Vec3f, step_submarine, Quatf};

#[inline]
fn yaw_of(q: Quatf) -> f32 {
    // Standard basis: +Z forward, +Y up; left-turn positive
    let fwd = q * Vec3f::new(0.0, 0.0, 1.0);
    fwd.x.atan2(fwd.z)
}

fn level_with_uniform_flow(mut base: LevelSpec, flow: Vec3f) -> LevelSpec {
    base.tunnel.flow = FlowFieldSpec::Uniform { flow, variance: 0.0 };
    base
}

#[test]
fn yaw_stability_tailflow_dt_1ms() {
    // Flow from behind along +Z; sub points +Z
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 2.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z),
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_rotation_y(0.0),
        ang_mom: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.5; spec.ballast_tanks.len()],
    };

    let dt = 0.001; // 1 ms
    let ticks = 1000;
    let mut t = 0.0;
    let inputs = SubInputs { thrust: 0.0, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..ticks {
        step_submarine(&level, &spec, inputs, &mut state, dt, t);
        t += dt;
    }
    let yaw = yaw_of(state.orientation);
    let eps = 0.02_f32; // ~1.1 degrees
    assert!(yaw.abs() <= eps, "yaw drifted under tailflow at 1ms dt: yaw={}", yaw);
}

#[test]
fn yaw_stability_tailflow_dt_10ms() {
    // Flow from behind along +Z; sub points +Z
    let level = level_with_uniform_flow(greybox_level(), Vec3f::new(0.0, 0.0, 2.0));
    let spec = levels::subspecs::small_skiff_spec();

    let mut state = SubState {
        position: Vec3f::new(level.tunnel.pos.x, level.tunnel.pos.y, level.tunnel.pos.z),
        velocity: Vec3f::new(0.0, 0.0, 0.0),
        orientation: Quatf::from_rotation_y(0.0),
        ang_mom: Vec3f::new(0.0, 0.0, 0.0),
        ballast_fill: vec![0.5; spec.ballast_tanks.len()],
    };

    let dt = 0.01; // 10 ms
    let ticks = 1000;
    let mut t = 0.0;
    let inputs = SubInputs { thrust: 0.0, yaw: 0.0, pump_fwd: 0.0, pump_aft: 0.0 };
    for _ in 0..ticks {
        step_submarine(&level, &spec, inputs, &mut state, dt, t);
        t += dt;
    }
    let yaw = yaw_of(state.orientation);
    let eps = 0.02_f32; // ~1.1 degrees
    assert!(yaw.abs() <= eps, "yaw drifted under tailflow at 10ms dt: yaw={}", yaw);
}
