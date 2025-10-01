use bevy::animation::{animated_field, AnimationTarget, AnimationTargetId};
use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;

use levels::{builtins::greybox_level, SubInputs, SubState, SubStepDebug};
use levels::{step_submarine_dbg, SubPhysicsSpec};

use crate::sim_pause::SimPause;

#[derive(Component)]
pub struct Submarine;

#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

#[derive(Component, Default, Deref, DerefMut)]
pub struct AngularVelocity(pub Vec3);

#[derive(Component)]
pub struct Rudder;

#[allow(dead_code)]
#[derive(Component, Clone)]
pub struct SubPhysics(pub SubPhysicsSpec);

#[derive(Component, Debug, Clone)]
pub struct SubStateComp(pub SubState);

#[derive(Component, Debug, Clone)]
#[allow(dead_code)]
pub struct ServerCorrection {
    pub target_pos: Vec3,
    pub target_rot: Quat,
    pub target_vel: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

/// Marker for entities whose transform is driven by network snapshots.
#[derive(Component)]
pub struct NetControlled;

#[derive(Resource, Debug, Clone, Default)]
pub struct SubTelemetry(pub SubStepDebug);

#[derive(Resource, Debug, Clone, Copy)]
pub struct ClientPhysicsTiming {
    pub acc: f32,
    pub dt: f32,
}

impl Default for ClientPhysicsTiming {
    fn default() -> Self {
        // Match server default tick_hz (30 Hz) unless overridden later.
        Self {
            acc: 0.0,
            dt: 1.0 / 120.0,
        }
    }
}

// Quatf is the same type as Bevy's Quat (re-exported from bevy_math).

#[allow(clippy::type_complexity)]
pub fn simulate_submarine(
    time: Res<Time>,
    mut q_sub: Query<
        (
            &mut Transform,
            &mut Velocity,
            &mut SubStateComp,
            &SubPhysics,
            Option<&ServerCorrection>,
            &mut AngularVelocity,
            Option<&NetControlled>,
        ),
        With<Submarine>,
    >,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
    mut telemetry: ResMut<SubTelemetry>,
    paused: Res<SimPause>,
    mut timing: ResMut<ClientPhysicsTiming>,
) {
    let frame_dt = time.delta_secs();
    if frame_dt <= 0.0 {
        return;
    }
    if paused.0 {
        timing.acc = 0.0; // avoid catch-up on resume
        return;
    }
    timing.acc += frame_dt;
    let step_dt = timing.dt.max(1e-4);
    let mut steps: u32 = 0;
    while timing.acc >= step_dt {
        timing.acc -= step_dt;
        steps += 1;
    }
    if steps == 0 {
        return;
    }

    // Build a transient LevelSpec identical to what's spawned (use the builtin for now)
    let level = greybox_level();

    let inputs = if let Some(c) = controls {
        SubInputs {
            thrust: c.value,
            yaw: c.yaw,
            pump_fwd: c.pump_fwd,
            pump_aft: c.pump_aft,
        }
    } else {
        SubInputs::default()
    };

    for (mut transform, mut vel, mut state_comp, spec, _correction, mut ang_vel_comp, _net) in
        &mut q_sub
    {
        // Map visual mesh (+X forward) to physics body (+Z forward): yaw +90°
        let body_from_mesh = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mesh_from_body = body_from_mesh.conjugate();
        // Initialize persistent SubState once if needed
        if state_comp.0.ballast_fill.is_empty() {
            state_comp.0 = SubState {
                position: levels::Vec3f::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                velocity: levels::Vec3f::new(vel.x, vel.y, vel.z),
                orientation: transform.rotation * body_from_mesh,
                ang_mom: {
                    let ixx = spec.0.ixx.max(0.0);
                    let iyy = spec.0.iyy.max(0.0);
                    let izz = spec.0.izz.max(0.0);
                    levels::Vec3f::new(
                        ang_vel_comp.x * ixx,
                        ang_vel_comp.y * iyy,
                        ang_vel_comp.z * izz,
                    )
                },
                ballast_fill: vec![0.5; spec.0.ballast_tanks.len()],
            };
        }
        let mut state = state_comp.0.clone();
        // Fixed-step loop; advance time parameter for flow sampling consistently
        let t0 = time.elapsed_secs() - (timing.acc + steps as f32 * step_dt);
        for i in 0..steps {
            let mut dbg = SubStepDebug::default();
            let t_sub = t0 + (i + 1) as f32 * step_dt;
            step_submarine_dbg(
                &level,
                &spec.0,
                inputs,
                &mut state,
                step_dt,
                t_sub,
                Some(&mut dbg),
            );
            telemetry.0 = dbg; // store last step's diagnostics
        }
        // Persist state back to component
        state_comp.0 = state.clone();
        transform.translation = Vec3::new(state.position.x, state.position.y, state.position.z);
        // Convert physics (body) orientation back to visual (mesh) orientation
        transform.rotation = state.orientation * mesh_from_body;

        **vel = Vec3::new(state.velocity.x, state.velocity.y, state.velocity.z);
        // Update client-side rates from body angular momentum
        let wx = if spec.0.ixx > 0.0 {
            state.ang_mom.x / spec.0.ixx
        } else {
            0.0
        };
        let wy = if spec.0.iyy > 0.0 {
            state.ang_mom.y / spec.0.iyy
        } else {
            0.0
        };
        let wz = if spec.0.izz > 0.0 {
            state.ang_mom.z / spec.0.izz
        } else {
            0.0
        };
        **ang_vel_comp = Vec3::new(wx, wy, wz);
    }
}

pub fn apply_server_corrections(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Velocity, &mut ServerCorrection), With<Submarine>>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for (e, mut t, mut v, mut corr) in &mut q {
        // Critically-damped like smoothing via exponential approach, with
        // separate handling for rotation when player is actively steering.
        let yaw_input_mag = controls.as_ref().map(|c| c.yaw.abs()).unwrap_or(0.0);
        let steering = yaw_input_mag > 0.05;

        let stiff_pos = 10.0_f32; // position/velocity convergence
        let stiff_vel = 10.0_f32;
        let stiff_rot = if steering { 4.0 } else { 8.0 }; // reduce rotation stiffness while steering

        let alpha_pos = 1.0 - (-stiff_pos * dt).exp();
        let alpha_vel = 1.0 - (-stiff_vel * dt).exp();
        let alpha_rot = 1.0 - (-stiff_rot * dt).exp();

        t.translation = t.translation.lerp(corr.target_pos, alpha_pos);
        t.rotation = t.rotation.slerp(corr.target_rot, alpha_rot);
        **v = (**v).lerp(corr.target_vel, alpha_vel);

        corr.elapsed += dt;
        let pos_err = t.translation.distance(corr.target_pos);
        let ang_err = t.rotation.angle_between(corr.target_rot);
        let vel_err = (**v - corr.target_vel).length();

        // Remove correction only when sufficiently close; avoid forced final snap.
        if pos_err < 0.01 && ang_err < 0.01 && vel_err < 0.02 {
            // Close enough: stop correcting without forcing an exact snap.
            commands.entity(e).remove::<ServerCorrection>();
        }
    }
}

pub fn animate_rudder(
    _time: Res<Time>,
    q_sub: Query<(&Transform, &Velocity), With<Submarine>>,
    mut q_rudder: Query<&mut Transform, (With<Rudder>, Without<Submarine>)>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
) {
    let Ok((_sub_t, _sub_v)) = q_sub.single() else {
        return;
    };
    let Some(mut rudder_t) = q_rudder.iter_mut().next() else {
        return;
    };
    // Visual convention: positive input = right rudder. Mesh is built so that
    // positive rotation around +Y deflects the trailing edge starboard.
    let yaw = controls
        .as_ref()
        .map(|c| c.yaw)
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0);
    let max_angle = 0.6_f32; // radians (~34 degrees)
    let angle = yaw * max_angle;
    rudder_t.rotation = Quat::from_rotation_y(angle);
}

pub fn make_rudder_prism_mesh(length: f32, height: f32, thickness: f32) -> Mesh {
    // Create a triangular prism extruded along Z
    let zf = thickness * 0.5;
    let zb = -zf;
    // Triangle vertices in XY plane
    let a = Vec3::new(0.0, -height * 0.5, 0.0);
    let b = Vec3::new(0.0, height * 0.5, 0.0);
    let c = Vec3::new(-length, 0.0, 0.0);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut add_tri = |v0: Vec3, v1: Vec3, v2: Vec3, n: Vec3| {
        let base = positions.len() as u32;
        positions.push(v0.to_array());
        positions.push(v1.to_array());
        positions.push(v2.to_array());
        normals.push(n.to_array());
        normals.push(n.to_array());
        normals.push(n.to_array());
        uvs.push([0.0, 0.0]);
        uvs.push([1.0, 0.0]);
        uvs.push([0.5, 1.0]);
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    };

    // Front face (z = +zf), normal +Z
    add_tri(
        a + Vec3::Z * zf,
        b + Vec3::Z * zf,
        c + Vec3::Z * zf,
        Vec3::Z,
    );
    // Back face (z = -zf), normal -Z (note reversed winding)
    add_tri(
        b + Vec3::Z * zb,
        a + Vec3::Z * zb,
        c + Vec3::Z * zb,
        -Vec3::Z,
    );

    // Side faces for each edge (two triangles forming a quad)
    let edges = [
        (a, b), // edge ab
        (b, c), // edge bc
        (c, a), // edge ca
    ];
    for (p0, p1) in edges {
        let v0f = p0 + Vec3::Z * zf;
        let v1f = p1 + Vec3::Z * zf;
        let v0b = p0 + Vec3::Z * zb;
        let v1b = p1 + Vec3::Z * zb;
        let n = (v1f - v0f).cross(v0b - v0f).normalize_or_zero();
        add_tri(v0f, v1f, v1b, n);
        add_tri(v0f, v1b, v0b, n);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

const SWIVEL_PERIOD: f32 = 4.0f32;
const SWIVEL_DEG: f32 = 15.0f32;
const SAMPLES: u32 = 40;

pub fn make_swivel_clip(
    commands: &mut Commands,
    light_entity: Entity,
    name: &Name,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut clip = AnimationClip::default();

    let correction = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);

    let data = (0..=SAMPLES).map(|i| {
        let t = i as f32 / SAMPLES as f32 * SWIVEL_PERIOD;
        // sine goes -1..+1, scale to angle range
        let angle = (t / SWIVEL_PERIOD * std::f32::consts::TAU).sin() * SWIVEL_DEG.to_radians();
        let quat = correction * Quat::from_rotation_y(angle);
        (t, quat)
    });

    let target_id = AnimationTargetId::from_name(name);

    clip.add_curve_to_target(
        target_id,
        AnimatableCurve::new(
            animated_field!(Transform::rotation),
            UnevenSampleAutoCurve::new(data).expect("valid quat curve"),
        ),
    );

    // Turn the clip into a graph and set it to repeat when played
    let (graph, node_index) = AnimationGraph::from_clip(clips.add(clip));
    let graph_handle = graphs.add(graph);

    let mut player = AnimationPlayer::default();
    player.play(node_index).repeat();

    commands
        .entity(light_entity)
        .insert(AnimationGraphHandle(graph_handle))
        .insert(player)
        .insert(AnimationTarget {
            id: target_id,
            player: light_entity,
        });
}
