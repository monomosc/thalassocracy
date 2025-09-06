use bevy::prelude::*;
use bevy::color::{LinearRgba, Srgba};
use bevy::math::primitives::{Cuboid, Sphere};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::Mesh3d;
use bevy::prelude::Gizmos;
use bevy::render::mesh::Indices;
use bevy::render::render_resource::{PrimitiveTopology};

use crate::debug_vis::DebugVis;
use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, Vec3f, SubState, SubInputs, Quatf};
use levels::subspecs::small_skiff_spec;
use levels::{SubPhysicsSpec, SubStepDebug, step_submarine_dbg};
use crate::sim_pause::SimPause;

#[derive(Component)]
pub struct StationRoom;

#[derive(Component)]
pub struct Tunnel;

#[derive(Component)]
pub struct Chamber;

#[derive(Component)]
pub struct DockPad;

/// Extensible flow field representation.
/// For M1 we keep a uniform field but design for future extension.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub enum FlowField {
    /// Uniform flow across space; `flow` is a 3D vector in world units/sec.
    /// `variance` encodes short-term stochastic deviation magnitude.
    Uniform { flow: Vec3, variance: f32 },
    // Future: Grid / Volumetric / Procedural variants can go here, e.g.:
    // Grid(GridFlow), Procedural(NoiseParams), etc.
}

impl FlowField {
    pub fn uniform(flow: Vec3, variance: f32) -> Self { Self::Uniform { flow, variance } }

    /// Sample the flow vector and variance at a world position and time.
    /// For Uniform, returns the same values regardless of `pos`/`time`.
    pub fn sample(&self, _pos: Vec3, _time: f32) -> (Vec3, f32) {
        match *self {
            FlowField::Uniform { flow, variance } => (flow, variance),
        }
    }
}

/// Cached bounds for the tunnel geometry for debug sampling and later gameplay.
#[derive(Component, Copy, Clone, Debug)]
pub struct TunnelBounds {
    pub size: Vec3, // X length, Y height, Z width (local space)
}

#[derive(Component)]
pub struct Submarine;

#[derive(Component, Default, Deref, DerefMut)]
pub struct Velocity(pub Vec3);

#[derive(Component, Default, Deref, DerefMut)]
pub struct AngularVelocity(pub Vec3);

#[derive(Component)]
pub struct FollowCam {
    pub distance: f32,
    pub height: f32,
    pub stiffness: f32, // larger = snappier follow
}

#[derive(Component)]
pub struct FollowCamState {
    pub last_dir: Vec3,
}

#[derive(Component)]
pub struct Rudder;

#[allow(dead_code)]
#[derive(Component, Clone)]
pub struct SubPhysics(pub SubPhysicsSpec);

#[derive(Component, Debug, Clone)]
pub struct ServerCorrection {
    pub target_pos: Vec3,
    pub target_rot: Quat,
    pub target_vel: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimSet;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FlowField>()
            .init_resource::<SubTelemetry>()
            .init_resource::<ClientPhysicsTiming>()
            .add_systems(Startup, (setup_scene, spawn_greybox))
            .add_systems(Update, (
                draw_flow_gizmos,
                simulate_submarine.in_set(SimSet),
                apply_server_corrections,
                update_follow_camera,
                animate_rudder,
            ));
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct SubTelemetry(pub SubStepDebug);

#[inline]
fn quatf_to_bevy(q: Quatf) -> Quat {
    // levels::Quatf stores (w, x, y, z); Bevy expects (x, y, z, w)
    Quat::from_xyzw(q.x, q.y, q.z, q.w)
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct ClientPhysicsTiming {
    pub acc: f32,
    pub dt: f32,
}

impl Default for ClientPhysicsTiming {
    fn default() -> Self {
        // Match server default tick_hz (30 Hz) unless overridden later.
        Self { acc: 0.0, dt: 1.0 / 30.0 }
    }
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    size: Vec3,
    pos: Vec3,
    color: Color,
) -> Entity {
    let mesh = meshes.add(Mesh::from(Cuboid::new(size.x, size.y, size.z)));
    let material = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..Default::default()
    });
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(pos),
            GlobalTransform::default(),
        ))
        .id()
}

fn setup_scene(mut commands: Commands) {
    // Gentle ambient light so unlit meshes are visible.
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        affects_lightmapped_meshes: true,
    });

    // Directional light for basic shading.
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: false,
            ..Default::default()
        },
        Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
        Name::new("Sun Light"),
    ));

    // Follow camera; exact position is updated to chase the submarine.
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.0, 10.0),
        GlobalTransform::default(),
        FollowCam { distance: 7.0, height: 2.2, stiffness: 6.0 },
        FollowCamState { last_dir: Vec3::NEG_X },
        Name::new("Follow Camera"),
    ));
}

fn spawn_greybox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Convert helpers
    fn v(v: Vec3f) -> Vec3 { Vec3::new(v.x, v.y, v.z) }

    // Reusable colors (StandardMaterial base_color: Color, emissive: LinearRgba)
    let wall_color: Color = Color::from(Srgba::new(0.35, 0.38, 0.42, 1.0));
    let floor_color: Color = Color::from(Srgba::new(0.20, 0.22, 0.25, 1.0));
    let tunnel_color: Color = Color::from(Srgba::new(0.25, 0.30, 0.35, 1.0));
    let chamber_color: Color = Color::from(Srgba::new(0.30, 0.32, 0.34, 1.0));
    let dock_emissive: LinearRgba = LinearRgba::from(Srgba::new(0.0, 0.8, 0.9, 1.0));

    // Load level spec from shared crate
    let level: LevelSpec = greybox_level();
    let room_w = level.room.size.x;
    let room_h = level.room.size.y;
    let room_d = level.room.size.z;
    let wall_thick = level.room.wall_thickness;

    // Floor
    let e_floor = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, wall_thick, room_d),
        Vec3::new(0.0, -wall_thick * 0.5, 0.0),
        floor_color,
    );
    commands.entity(e_floor).insert(Name::new("Station Floor"));
    // Walls
    // +X wall
    let wall_e = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(wall_thick, room_h, room_d),
        Vec3::new(room_w * 0.5, room_h * 0.5 - wall_thick, 0.0),
        wall_color,
    );
    commands.entity(wall_e).insert(StationRoom);
    // -X wall
    let e_wall_negx = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(wall_thick, room_h, room_d),
        Vec3::new(-room_w * 0.5, room_h * 0.5 - wall_thick, 0.0),
        wall_color,
    );
    commands.entity(e_wall_negx).insert(Name::new("Station Wall -X"));
    // +Z wall
    let e_wall_posz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, room_d * 0.5),
        wall_color,
    );
    commands.entity(e_wall_posz).insert(Name::new("Station Wall +Z"));
    // -Z wall
    let e_wall_negz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, -room_d * 0.5),
        wall_color,
    );
    commands.entity(e_wall_negz).insert(Name::new("Station Wall -Z"));

    // Docking pad in the station
    {
        let dsz = v(level.room.dock_size);
        let mesh = meshes.add(Mesh::from(Cuboid::new(dsz.x, dsz.y, dsz.z)));
        let material = materials.add(StandardMaterial { base_color: Color::BLACK, emissive: dock_emissive, ..Default::default() });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(v(level.room.dock_pos)),
            GlobalTransform::default(),
            DockPad,
            Name::new("Dock Pad"),
        ));
    }

    // Tunnel from +X wall outward (hollow shell: floor/ceiling/side walls)
    let tunnel_size = v(level.tunnel.size); // interior/open space
    let tunnel_pos = v(level.tunnel.pos);
    let tunnel_entity = {
        // Parent holds the field and bounds. Children are the shell meshes.
        let parent = commands
            .spawn((
                Transform::from_translation(tunnel_pos),
                GlobalTransform::default(),
                Tunnel,
                TunnelBounds { size: tunnel_size },
                // Flow field from spec
                match level.tunnel.flow {
                    FlowFieldSpec::Uniform { flow, variance } => FlowField::uniform(v(flow), variance),
                },
                Name::new("Tunnel"),
            ))
            .id();

        let mat_tunnel = materials.add(StandardMaterial { base_color: tunnel_color, ..Default::default() });
        let half = tunnel_size * 0.5;
        let t = level.tunnel.shell_thickness; // shell thickness

        // Helper to spawn a wall as a child
        let mut spawn_wall = |size: Vec3, local: Vec3, name: &str| {
            let wall_mesh = meshes.add(Mesh::from(Cuboid::new(size.x, size.y, size.z)));
            let child = commands
                .spawn((
                    Mesh3d(wall_mesh),
                    MeshMaterial3d(mat_tunnel.clone()),
                    Transform::from_translation(local),
                    GlobalTransform::default(),
                    Name::new(name.to_string()),
                ))
                .id();
            commands.entity(child).insert(ChildOf(parent));
        };

        // Floor and ceiling
        spawn_wall(
            Vec3::new(tunnel_size.x, t, tunnel_size.z),
            Vec3::new(0.0, -half.y + t * 0.5, 0.0),
            "Tunnel Floor",
        );
        spawn_wall(
            Vec3::new(tunnel_size.x, t, tunnel_size.z),
            Vec3::new(0.0, half.y - t * 0.5, 0.0),
            "Tunnel Ceiling",
        );
        // Side walls
        spawn_wall(
            Vec3::new(tunnel_size.x, tunnel_size.y, t),
            Vec3::new(0.0, 0.0, half.z - t * 0.5),
            "Tunnel Wall +Z",
        );
        spawn_wall(
            Vec3::new(tunnel_size.x, tunnel_size.y, t),
            Vec3::new(0.0, 0.0, -half.z + t * 0.5),
            "Tunnel Wall -Z",
        );

        parent
    };

    // Mining chamber at the end of the tunnel
    let chamber_size = v(level.chamber.size);
    let chamber_pos = v(level.chamber.pos);
    {
        let mesh = meshes.add(Mesh::from(Cuboid::new(
            chamber_size.x,
            chamber_size.y,
            chamber_size.z,
        )));
        let material = materials.add(StandardMaterial { base_color: chamber_color, ..Default::default() });
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(chamber_pos),
            GlobalTransform::default(),
            Chamber,
            Name::new("Chamber"),
        ));
    }

    // Spawn a submarine (parent) with child hull and child rudder
    {
        // Place near the -X end of the tunnel, centered in YZ
        let tb = TunnelBounds { size: tunnel_size };
        let start = tunnel_pos + Vec3::new(-tb.size.x * 0.5 + 6.0, 0.0, 0.0);

    let sub_root = commands
            .spawn((
                Transform::from_translation(start),
                GlobalTransform::default(),
                Submarine,
                Velocity::default(),
                AngularVelocity::default(),
                SubPhysics(small_skiff_spec()),
                Name::new("SubmarineRoot"),
            ))
            .id();

        // Hull (prolate spheroid) as child
        let sub_radius = 0.6;
        let sub_scale = Vec3::new(2.2, 0.8, 0.8); // prolate along +X
        let hull_mesh = meshes.add(Mesh::from(Sphere::new(sub_radius)));
        let hull_material = materials.add(StandardMaterial {
            base_color: Color::from(Srgba::new(0.75, 0.8, 0.85, 1.0)),
            perceptual_roughness: 0.4,
            metallic: 0.1,
            ..Default::default()
        });
        let hull = commands
            .spawn((
                Mesh3d(hull_mesh),
                MeshMaterial3d(hull_material),
                Transform::from_scale(sub_scale),
                GlobalTransform::default(),
                Name::new("SubmarineHull"),
            ))
            .id();
        commands.entity(hull).insert(ChildOf(sub_root));

        // Rudder as child (triangular prism thickness)
        let rudder_mesh = make_rudder_prism_mesh(1.0, 1.4, 0.12);
        let rudder_mesh = meshes.add(rudder_mesh);
        let rudder_material = materials.add(StandardMaterial {
            base_color: Color::from(Srgba::new(0.9, 0.1, 0.1, 1.0)),
            cull_mode: None,
            ..Default::default()
        });
        let rudder_local = Transform::from_translation(Vec3::new(-1.6, 0.0, 0.0));
        let rudder = commands
            .spawn((
                Mesh3d(rudder_mesh),
                MeshMaterial3d(rudder_material),
                rudder_local,
                GlobalTransform::default(),
                Rudder,
                Name::new("Rudder"),
            ))
            .id();
        commands.entity(rudder).insert(ChildOf(sub_root));

        let _ = tunnel_entity; // ensure it exists (unused var otherwise)
    }
}

fn draw_flow_gizmos(
    vis: Option<Res<DebugVis>>,
    mut gizmos: Gizmos,
    q: Query<(&GlobalTransform, &FlowField, &TunnelBounds), With<Tunnel>>,
    time: Res<Time>,
) {
    let Some(vis) = vis else { return; };
    if !vis.flow_arrows { return; }

    for (transform, field, bounds) in &q {
        // For now, assume axis-aligned tunnel (no rotation or non-uniform scale).
        let center = transform.translation();
        let half = bounds.size * 0.5;

        let nx = 6; // samples along length
        let ny = 2; // samples along height
        let nz = 2; // samples along width

        for ix in 0..nx {
            for iy in 0..ny {
                for iz in 0..nz {
                    let fx = (ix as f32 + 0.5) / nx as f32;
                    let fy = (iy as f32 + 0.5) / ny as f32;
                    let fz = (iz as f32 + 0.5) / nz as f32;

                    // Local position within the cuboid, centered on `center`.
                    let local = Vec3::new(
                        -half.x + bounds.size.x * fx,
                        -half.y + bounds.size.y * fy,
                        -half.z + bounds.size.z * fz,
                    );
                    let pos = center + local;

                    let (flow, variance) = field.sample(pos, time.elapsed_secs());
                    let dir = flow;
                    if dir.length_squared() > 1e-6 {
                        let len = 0.8 + variance; // visualize variance as arrow length contribution
                        gizmos.arrow(pos, pos + dir.normalize() * len, Color::srgb(0.2, 0.7, 1.0));
                    }
                }
            }
        }
    }
}

fn simulate_submarine(
    time: Res<Time>,
    mut q_sub: Query<(&mut Transform, &mut Velocity, &SubPhysics, Option<&ServerCorrection>, &mut AngularVelocity), With<Submarine>>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
    mut telemetry: ResMut<SubTelemetry>,
    paused: Res<SimPause>,
    mut timing: ResMut<ClientPhysicsTiming>,
) {
    let frame_dt = time.delta_secs();
    if frame_dt <= 0.0 { return; }
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
    if steps == 0 { return; }

    // Build a transient LevelSpec identical to what's spawned (use the builtin for now)
    let level = greybox_level();

    let inputs = if let Some(c) = controls { SubInputs { thrust: c.value, yaw: c.yaw, pump_fwd: c.pump_fwd, pump_aft: c.pump_aft } } else { SubInputs::default() };

    for (mut transform, mut vel, spec, correction, mut ang_vel_comp) in &mut q_sub {
        // Persist ballast fill across frames using last telemetry (single local player)
        // Default to 50% fill; override from telemetry if available (mass_eff > 0 implies prior step)
        let mut prev_fill = vec![0.5; spec.0.ballast_tanks.len()];
        if spec.0.ballast_tanks.len() >= 2 {
            let last = &telemetry.0;
            if last.mass_eff > 0.0 {
                prev_fill[0] = last.fill_fwd.clamp(0.0, 1.0);
                prev_fill[1] = last.fill_aft.clamp(0.0, 1.0);
            }
        }
        let mut state = SubState {
            position: Vec3f { x: transform.translation.x, y: transform.translation.y, z: transform.translation.z },
            velocity: Vec3f { x: vel.x, y: vel.y, z: vel.z },
            orientation: {
                use bevy::prelude::EulerRot;
                let (_rx, yaw, _rz) = transform.rotation.to_euler(EulerRot::YXZ);
                Quatf::from_yaw(yaw)
            },
            ang_vel: Vec3f::new(ang_vel_comp.x, ang_vel_comp.y, ang_vel_comp.z),
            ballast_fill: prev_fill,
        };
        // Fixed-step loop; advance time parameter for flow sampling consistently
        let t0 = time.elapsed_secs() - (timing.acc + steps as f32 * step_dt);
        for i in 0..steps {
            let mut dbg = SubStepDebug::default();
            let t_sub = t0 + (i + 1) as f32 * step_dt;
            step_submarine_dbg(&level, &spec.0, inputs, &mut state, step_dt, t_sub, Some(&mut dbg));
            telemetry.0 = dbg; // store last step's diagnostics
        }
        transform.translation = Vec3::new(state.position.x, state.position.y, state.position.z);
        // If a server correction smoothing is active, avoid fighting it on rotation.
        // Otherwise, apply full simulated orientation (yaw + pitch).
        if correction.is_none() {
            transform.rotation = quatf_to_bevy(state.orientation);
        }
        **vel = Vec3::new(state.velocity.x, state.velocity.y, state.velocity.z);
        **ang_vel_comp = Vec3::new(state.ang_vel.x, state.ang_vel.y, state.ang_vel.z);
    }
}

fn apply_server_corrections(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Velocity, &mut ServerCorrection), With<Submarine>>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 { return; }

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

fn update_follow_camera(
    time: Res<Time>,
    q_sub: Query<&Transform, With<Submarine>>,
    mut q_cam: Query<(&mut Transform, &FollowCam, &mut FollowCamState), Without<Submarine>>,
) {
    let Ok(sub_t) = q_sub.single() else { return; };
    let sub_pos = sub_t.translation;
    // Forward direction from orientation (body +X)
    let orient_dir = (sub_t.rotation * Vec3::X).normalize_or_zero();

    for (mut cam_t, cam, mut state) in &mut q_cam {
        let dir = if orient_dir.length_squared() > 1e-6 { orient_dir } else { state.last_dir };
        state.last_dir = dir;

        // Place camera behind the submarine relative to its orientation
        let desired_pos = sub_pos - dir * cam.distance + Vec3::Y * cam.height;

        let stiffness = cam.stiffness.max(0.0);
        let dt = time.delta_secs();
        let lerp = 1.0 - (-stiffness * dt).exp();
        cam_t.translation = cam_t.translation.lerp(desired_pos, lerp);
        cam_t.look_at(sub_pos, Vec3::Y);
    }
}

fn animate_rudder(
    _time: Res<Time>,
    q_sub: Query<(&Transform, &Velocity), With<Submarine>>,
    mut q_rudder: Query<&mut Transform, (With<Rudder>, Without<Submarine>)>,
    controls: Option<Res<crate::hud_controls::ThrustInput>>,
) {
    let Ok((_sub_t, _sub_v)) = q_sub.single() else { return; };
    let Some(mut rudder_t) = q_rudder.iter_mut().next() else { return; };
    // Visual convention: positive input = right rudder. Mesh is built so that
    // positive rotation around +Y deflects the trailing edge starboard.
    let yaw = controls.as_ref().map(|c| c.yaw).unwrap_or(0.0).clamp(-1.0, 1.0);
    let max_angle = 0.6_f32; // radians (~34 degrees)
    let angle = yaw * max_angle;
    rudder_t.rotation = Quat::from_rotation_y(angle);
}

fn make_rudder_prism_mesh(length: f32, height: f32, thickness: f32) -> Mesh {
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
        uvs.push([0.0, 0.0]); uvs.push([1.0, 0.0]); uvs.push([0.5, 1.0]);
        indices.extend_from_slice(&[base, base + 1, base + 2]);
    };

    // Front face (z = +zf), normal +Z
    add_tri(a + Vec3::Z * zf, b + Vec3::Z * zf, c + Vec3::Z * zf, Vec3::Z);
    // Back face (z = -zf), normal -Z (note reversed winding)
    add_tri(b + Vec3::Z * zb, a + Vec3::Z * zb, c + Vec3::Z * zb, -Vec3::Z);

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
