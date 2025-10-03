use bevy::color::{LinearRgba, Srgba};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSamplerDescriptor};
use bevy::math::primitives::{Cuboid, Sphere, Plane3d};
use bevy::pbr::{MeshMaterial3d, NotShadowCaster, StandardMaterial};
use bevy::prelude::*;
use bevy::math::{Affine2, Vec2};
use bevy::render::render_resource::TextureUsages;

use crate::debug_vis::DebugVis;
use levels::{builtins::greybox_level, FlowFieldSpec, LevelSpec, Vec3f};
use levels::subspecs::small_skiff_spec;

use super::setup::spawn_box;
use super::submarine::{make_rudder_prism_mesh, AngularVelocity, Rudder, SubPhysics, Submarine, Velocity};
use super::camera::{GameCamera, CamMode, FollowCam, FollowCamState, FreeFlyState};

#[derive(Component)]
pub struct StationRoom;

#[derive(Component)]
pub struct Tunnel;

#[derive(Component)]
pub struct Chamber;

#[derive(Component)]
#[allow(dead_code)]
pub struct BlinkingLight {
    /// total period in seconds (e.g. 1.0 = 1 Hz)
    period: f32,
    /// fraction of the period that the light is ON (0..1), e.g. 0.2 = 20% duty cycle
    on_fraction: f32,
    /// intensity when ON
    on_intensity: f32,
    /// intensity when OFF (usually 0.0)
    off_intensity: f32,
}
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
}

impl FlowField {
    pub fn uniform(flow: Vec3, variance: f32) -> Self {
        Self::Uniform { flow, variance }
    }

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

pub fn spawn_greybox(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>
) {
    // Convert helpers
    fn v(v: Vec3f) -> Vec3 {
        Vec3::new(v.x, v.y, v.z)
    }

    // Reusable colors (StandardMaterial base_color: Color, emissive: LinearRgba)
    let wall_color: Color = Color::from(Srgba::new(0.35, 0.38, 0.42, 1.0));
    let floor_color: Color = Color::from(Srgba::new(0.20, 0.22, 0.25, 1.0));
    let _tunnel_color: Color = Color::from(Srgba::new(0.25, 0.30, 0.35, 1.0));
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
    commands
        .entity(e_wall_negx)
        .insert(Name::new("Station Wall -X"));
    // +Z wall
    let e_wall_posz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, room_d * 0.5),
        wall_color,
    );
    commands
        .entity(e_wall_posz)
        .insert(Name::new("Station Wall +Z"));
    // -Z wall
    let e_wall_negz = spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(room_w, room_h, wall_thick),
        Vec3::new(0.0, room_h * 0.5 - wall_thick, -room_d * 0.5),
        wall_color,
    );
    commands
        .entity(e_wall_negz)
        .insert(Name::new("Station Wall -Z"));

    // Docking pad in the station
    {
        let dsz = v(level.room.dock_size);
        let mesh = meshes.add(Mesh::from(Cuboid::new(dsz.x, dsz.y, dsz.z)));
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: dock_emissive,
            ..Default::default()
        });
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
                Visibility::default(),
                // Flow field from spec
                match level.tunnel.flow {
                    FlowFieldSpec::Uniform { flow, variance } => {
                        FlowField::uniform(v(flow), variance)
                    }
                },
                Name::new("Tunnel"),
            ))
            .id();

        // Use the provided rock albedo; disable depth_map for now to avoid sampler type mismatch from 16-bit PNG
        let tex_albedo: Handle<Image> = asset_server.load_with_settings("textures/rock_face_03_diff_4k.jpg", | settings: &mut ImageLoaderSettings| {
            settings.sampler = bevy::image::ImageSampler::Descriptor(ImageSamplerDescriptor {
                            address_mode_u: ImageAddressMode::Repeat,
                            address_mode_v: ImageAddressMode::Repeat,
                            address_mode_w: ImageAddressMode::Repeat,
                            ..default()
                        });
        });

        // Helper to build a material with custom UV tiling and optional flips
        let mut make_mat = |repeats: Vec2, flip_x: bool, flip_y: bool| {
            let mut uv = Affine2::from_scale(repeats);
            if flip_x { uv = StandardMaterial::FLIP_VERTICAL * uv; }
            if flip_y { uv = StandardMaterial::FLIP_HORIZONTAL * uv; }
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(tex_albedo.clone()),
                metallic: 0.1,
                perceptual_roughness: 0.95,
                // Ensure interior faces render correctly when viewed from inside the tunnel
                cull_mode: None,
                double_sided: true,
                uv_transform: uv,
                ..Default::default()
            })
        };

        // Repeats tuned per face to avoid stretching on long axes
        let rx = 8.0;
        let rz = 2.0;
        let ry = 2.0;
        let mat_floor = make_mat(Vec2::new(rx, rz), false, false);
        let mat_ceil = make_mat(Vec2::new(rx, rz), false, false); // flip to reduce obvious repetitions
        let mat_wall_pz = make_mat(Vec2::new(rx, ry), false, false);
        let mat_wall_nz = make_mat(Vec2::new(rx, ry), false, false); // mirrored to break symmetry seams
        let half = tunnel_size * 0.5;

        // Helper to spawn a single textured plane as a child (avoids cuboid UV issues)
        let mut spawn_plane = |size: Vec2, local: Vec3, rot: Quat, name: &str, mat: Handle<StandardMaterial>| {
            let mesh = meshes.add(Plane3d::default().mesh().size(size.x, size.y));
            let child = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_translation(local).with_rotation(rot),
                    GlobalTransform::default(),
                    Name::new(name.to_string()),
                ))
                .id();
            commands.entity(child).insert(ChildOf(parent));
        };

        // Floor (XZ plane, normal +Y)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.z),
            Vec3::new(0.0, -half.y, 0.0),
            Quat::IDENTITY,
            "Tunnel Floor",
            mat_floor.clone(),
        );
        // Ceiling (XZ plane, normal -Y)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.z),
            Vec3::new(0.0, half.y, 0.0),
            Quat::from_rotation_x(std::f32::consts::PI),
            "Tunnel Ceiling",
            mat_ceil.clone(),
        );
        // +Z wall (XY plane, normal -Z)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.y),
            Vec3::new(0.0, 0.0, half.z),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            "Tunnel Wall +Z",
            mat_wall_pz.clone(),
        );
        // -Z wall (XY plane, normal +Z)
        spawn_plane(
            Vec2::new(tunnel_size.x, tunnel_size.y),
            Vec3::new(0.0, 0.0, -half.z),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            "Tunnel Wall -Z",
            mat_wall_nz.clone(),
        );

        // Accent spotlights: four walls, spaced along tunnel length, pointing inward
        {
            let xs = [ -half.x * 0.45, 0.0, half.x * 0.45 ];
            // Range sized to cross-section diagonal
            let cross_diag = (tunnel_size.y.hypot(tunnel_size.z)).max(1.0);
            let range = cross_diag * 0.74;
            let intensity = 4200.0; // relatively small accent lights
            let color = Color::srgb(0.85, 0.95, 1.0);
            let inner = 0.15f32;
            let outer = 0.25f32;
            let inset = 0.02; // push lights slightly into the interior from the wall

            for (i, x) in xs.into_iter().enumerate() {
                // Compute a local target at the tunnel center directly inward from the light
                let tgt = Vec3::new(x, 0.0, 0.0);

                // +Z wall → aim inward (-Z)
                let pos_pz = Vec3::new(x, 0.0, half.z - inset);
                let t = Transform::from_translation(pos_pz).looking_at(tgt, Vec3::Y);
                commands
                    .spawn((
                        SpotLight {
                            color,
                            intensity,
                            range,
                            inner_angle: inner,
                            outer_angle: outer,
                            shadows_enabled: false,
                            ..Default::default()
                        },
                        t,
                        GlobalTransform::default(),
                        Name::new(format!("Tunnel Light +Z #{i}")),
                    ))
                    .insert(ChildOf(parent));

                // -Z wall
                let pos_nz = Vec3::new(x, 0.0, -half.z + inset);
                let t = Transform::from_translation(pos_nz).looking_at(tgt, Vec3::Y);
                commands
                    .spawn((
                        SpotLight {
                            color,
                            intensity,
                            range,
                            inner_angle: inner,
                            outer_angle: outer,
                            shadows_enabled: false,
                            ..Default::default()
                        },
                        t,
                        GlobalTransform::default(),
                        Name::new(format!("Tunnel Light -Z #{i}")),
                    ))
                    .insert(ChildOf(parent));

                // +Y ceiling (aim down)
                let pos_py = Vec3::new(x, half.y - inset, 0.0);
                let t = Transform::from_translation(pos_py).looking_at(tgt, Vec3::Y);
                commands
                    .spawn((
                        SpotLight {
                            color,
                            intensity,
                            range,
                            inner_angle: inner,
                            outer_angle: outer,
                            shadows_enabled: false,
                            ..Default::default()
                        },
                        t,
                        GlobalTransform::default(),
                        Name::new(format!("Tunnel Light +Y #{i}")),
                    ))
                    .insert(ChildOf(parent));

                // -Y floor (aim up)
                let pos_ny = Vec3::new(x, -half.y + inset, 0.0);
                let t = Transform::from_translation(pos_ny).looking_at(tgt, Vec3::Y);
                commands
                    .spawn((
                        SpotLight {
                            color,
                            intensity,
                            range,
                            inner_angle: inner,
                            outer_angle: outer,
                            shadows_enabled: false,
                            ..Default::default()
                        },
                        t,
                        GlobalTransform::default(),
                        Name::new(format!("Tunnel Light -Y #{i}")),
                    ))
                    .insert(ChildOf(parent));
            }
        }

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
        let material = materials.add(StandardMaterial {
            base_color: chamber_color,
            ..Default::default()
        });
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
                Visibility::default(),
                Submarine,
                Velocity::default(),
                AngularVelocity::default(),
                SubPhysics(small_skiff_spec()),
                crate::hud_instruments::HudInstrumentState::default(),
                // Initialize persistent physics state; fill is set in simulate on first tick
                super::submarine::SubStateComp(levels::SubState { position: levels::Vec3f::new(start.x, start.y, start.z), velocity: levels::Vec3f::ZERO, orientation: Quat::IDENTITY, ang_mom: levels::Vec3f::ZERO, ballast_fill: Vec::new() }),
                super::submarine::SubInputStateComp(levels::SubInputState::default()),
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
        let rudder_mesh = make_rudder_prism_mesh(1.0, 1.2, 0.12);
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

        // Forward floodlight as a child (spotlight)
        // Positioned slightly ahead of the hull nose; oriented along local +X.
        let light_pos = Vec3::new(0.01, 0.3, 0.0);
        let light_transform = Transform::from_translation(light_pos)
            .looking_at(light_pos + Vec3::X, Vec3::Y);
        commands.spawn((
            SpotLight {
                color: Color::srgb(1.0, 1.0, 1.0),
                intensity: 1_200_000_000.0, // brighter for longer throw
                range: 600.0,
                inner_angle: 0.04,
                outer_angle: 0.08,
                shadows_enabled: true,
                ..Default::default()
            },
            light_transform,
            Name::new("Sub Floodlight"),
        )).insert(ChildOf(sub_root));

        let tail_point_light_pos = Vec3::new(-1.1, 0.27, 0.0);
        let tail_root = commands.spawn((
            Transform::from_translation(tail_point_light_pos),
            Name::from("Sub Tail-light Root"),
            ChildOf(sub_root)
        )).id();
        let _tail_bulb = commands.spawn((
            Mesh3d(meshes.add(Mesh::from(Sphere { radius: 0.02 }))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.9,0.2, 0.25),
                emissive: LinearRgba::rgb(8.0, 0.0, 0.0),
                ..default()
            })),
            Name::from("Sub Tail-light bulb"),
            NotShadowCaster,
            ChildOf(tail_root)
        ));
        let _tail_light = commands.spawn((
            PointLight {
                color: Color::linear_rgb(1.0, 0.0, 0.0),
                intensity: 2000.0,  //gets overwritten anyway
                range: 24.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::IDENTITY,
            Name::from("Sub Tail-ligt Pointlight"),
            ChildOf(tail_root)
        ));

        // Unified game camera (single camera). Initialize near the bow; mode = FirstPerson
        let fp_world = start + Vec3::new(1.0, 0.0, 0.0);
        let fp_t = Transform::from_translation(fp_world).looking_at(fp_world + Vec3::X, Vec3::Y);
        commands.spawn((
            Camera3d { depth_texture_usages: (TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING).into(), ..Default::default() },
            Camera { hdr: true, is_active: true, ..Default::default() },
            bevy::core_pipeline::bloom::Bloom::OLD_SCHOOL,
            bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            bevy::pbr::DistanceFog { color: Color::srgb(0.04, 0.11, 0.12), falloff: bevy::pbr::FogFalloff::Exponential { density: 0.10 }, ..Default::default() },
            Msaa::Off,
            fp_t,
            GlobalTransform::default(),
            GameCamera,
            CamMode::FirstPerson,
            FollowCam { distance: 8.0, height: 2.0, stiffness: 8.0 },
            FollowCamState { last_dir: Vec3::NEG_X },
            FreeFlyState { yaw: 0.0, pitch: 0.0, speed: 8.0 },
            Name::new("Game Camera"),
        ));

        let _ = tunnel_entity; // ensure it exists (unused var otherwise)
    }
}

pub fn draw_flow_gizmos(
    vis: Option<Res<DebugVis>>,
    mut gizmos: Gizmos,
    q: Query<(&GlobalTransform, &FlowField, &TunnelBounds), With<Tunnel>>,
    time: Res<Time>,
) {
    let Some(vis) = vis else { return; };
    if !vis.flow_arrows {
        return;
    }

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
                        gizmos.arrow(
                            pos,
                            pos + dir.normalize() * len,
                            Color::srgb(0.2, 0.7, 1.0),
                        );
                    }
                }
            }
        }
    }
}

